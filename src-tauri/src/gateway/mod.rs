pub mod context;
pub mod error;
pub mod header_config;
pub mod log_cleanup;
pub mod log_cost;
pub mod middleware;
pub mod pipeline;
pub mod plugin;
pub mod plugins;

use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use sqlx::SqlitePool;
use tokio::task::JoinHandle;

use crate::models::NewRequestLog;
use crate::persistence::LogExtraTokenRepo;

pub use context::{GatewayContext, RequestContext, RouteManager};
pub use error::GatewayError;
pub use pipeline::{GatewayPipeline, StageError};
pub use plugin::GatewayPlugin;

pub struct GatewayServerHandle {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    join_handle: JoinHandle<()>,
}

impl GatewayServerHandle {
    pub async fn stop(mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }
        let _ = self.join_handle.await;
    }
}

/// 启动网关服务
pub async fn spawn_gateway_server(
    context: GatewayContext,
) -> Result<GatewayServerHandle, std::io::Error> {
    let settings = context.settings.read().await.clone();
    let addr = format!("{}:{}", settings.bind_host, settings.bind_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let app = build_router(context);

    let join_handle = tokio::spawn(async move {
        let server =
            axum::serve(listener, app.into_make_service()).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });

        if let Err(err) = server.await {
            tracing::error!(%err, "gateway server stopped with error");
        }
    });

    Ok(GatewayServerHandle {
        shutdown: Some(shutdown_tx),
        join_handle,
    })
}

fn build_router(context: GatewayContext) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .fallback(proxy_handler)
        .with_state(context)
}

async fn health_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "silk-gateway"
    }))
}

async fn proxy_handler(State(context): State<GatewayContext>, req: Request<Body>) -> Response {
    GatewayPipeline::new(context).execute(req).await
}

/// 启动后台日志写入任务
///
/// 从 channel 接收日志数据，批量写入 SQLite（主表 + 扩展表）。
/// 返回 JoinHandle，用于优雅关闭。
pub fn spawn_log_writer(
    pool: SqlitePool,
    mut receiver: tokio::sync::mpsc::Receiver<NewRequestLog>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut batch = Vec::new();
        let batch_size = 50;
        let flush_interval = std::time::Duration::from_secs(5);

        let mut interval = tokio::time::interval(flush_interval);

        loop {
            tokio::select! {
                // 接收日志
                maybe_log = receiver.recv() => {
                    match maybe_log {
                        Some(log) => {
                            batch.push(log);
                            if batch.len() >= batch_size {
                                flush_batch(&pool, &mut batch).await;
                            }
                        }
                        None => {
                            // channel 关闭，刷新剩余日志后退出
                            if !batch.is_empty() {
                                flush_batch(&pool, &mut batch).await;
                            }
                            break;
                        }
                    }
                }
                // 定时刷新
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        flush_batch(&pool, &mut batch).await;
                    }
                }
            }
        }
    })
}

/// 批量写入日志到 SQLite（主表 + 扩展表）
async fn flush_batch(pool: &SqlitePool, batch: &mut Vec<NewRequestLog>) {
    if batch.is_empty() {
        return;
    }

    let mut logs = batch.drain(..).collect::<Vec<_>>();

    // 在消费侧计算 cost，不阻塞请求热路径
    log_cost::compute_batch_costs(&mut logs, pool).await;

    // 分离扩展信息（cache_hit, tokens, cost 等迁出字段）
    let extras: Vec<crate::models::NewRequestLogExtraToken> = logs
        .iter()
        .map(|log| crate::models::NewRequestLogExtraToken {
            request_id: log.request_id.clone(),
            cache_hit: log.cache_hit,
            request_size_bytes: log.request_size_bytes,
            response_size_bytes: log.response_size_bytes,
            tokens_input: log.tokens_input,
            tokens_output: log.tokens_output,
            tokens_sent: log.tokens_sent,
            cost: log.cost,
        })
        .collect();

    // 写入主表
    let main_result = crate::persistence::LogRepo::insert_batch(pool, &logs).await;
    match &main_result {
        Ok(count) => {
            tracing::debug!(count, "批量写入日志成功");
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.code().as_deref() == Some("787") => {
            // FOREIGN KEY 约束失败：批量将 provider_id 和 route_id 置空后重试
            tracing::warn!("日志 FOREIGN KEY 约束失败，整批降级写入");
            let fallback_logs: Vec<_> = logs
                .into_iter()
                .map(|mut log| {
                    log.provider_id = None;
                    log.route_id = None;
                    log
                })
                .collect();
            if let Err(err) = crate::persistence::LogRepo::insert_batch(pool, &fallback_logs).await {
                tracing::warn!(%err, "降级写入日志仍然失败");
            }
        }
        Err(err) => {
            tracing::warn!(%err, "批量写入日志失败");
        }
    }

    // 只有主表写入成功后才写入扩展表
    if main_result.is_ok() {
        match LogExtraTokenRepo::insert_batch(pool, &extras).await {
            Ok(count) => {
                tracing::debug!(count, "批量写入扩展日志成功");
            }
            Err(e) => {
                tracing::error!(error = %e, "写入扩展日志失败");
            }
        }
    }
}
