use axum::body::Body;
use axum::http::Request;
use axum::response::Response;

use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::middleware::{
    authenticate, dispatch_upstream, extract, finalize, persist_log, rate_limit, resolve_route,
    select_channel, transform_request, transform_response,
};

pub struct StageError {
    pub context: RequestContext,
    pub error: GatewayError,
}

impl StageError {
    pub fn new(context: RequestContext, error: GatewayError) -> Self {
        Self { context, error }
    }
}

#[derive(Clone)]
pub struct GatewayPipeline {
    runtime: GatewayContext,
}

impl GatewayPipeline {
    pub fn new(runtime: GatewayContext) -> Self {
        Self { runtime }
    }

    pub async fn execute(&self, req: Request<Body>) -> Response {
        let (parts, body) = req.into_parts();
        let base_ctx = extract::initialize(parts);

        match self.run_main(base_ctx, body).await {
            Ok(mut ctx) => {
                let _ = persist_log::run(&self.runtime.log_sender, &mut ctx).await;
                finalize::success(ctx)
            }
            Err(mut stage_error) => {
                let status = stage_error.error.status_code();
                stage_error.context.mark_error(
                    stage_error.error.to_string(),
                    stage_error.error.error_code().to_string(),
                    status,
                );
                let _ = persist_log::run(&self.runtime.log_sender, &mut stage_error.context).await;
                finalize::failure(stage_error.error)
            }
        }
    }

    /// 执行主处理管道，含三级失败回退：
    ///   1. dispatch_upstream 自身重试（已有）
    ///   2. 重试耗尽 → 换 Key（select_channel 排除已失败 Key）
    ///   3. 所有 Key 失败 → 换渠道（try_next_channel 选下一个 provider）
    async fn run_main(
        &self,
        ctx: RequestContext,
        body: Body,
    ) -> Result<RequestContext, StageError> {
        let ctx = extract::read_body(ctx, body).await?;
        let ctx = authenticate::run(ctx, &self.runtime).await?;
        let ctx = rate_limit::run(ctx, &self.runtime).await?;
        let ctx = resolve_route::run(&self.runtime, ctx).await?;

        // 如果路由阶段已构建响应（如 /v1/models），跳过后续流程
        if ctx.response.is_some() {
            return Ok(ctx);
        }

        // 失败回退循环
        self.run_with_failover(ctx).await
    }

    /// 带三级回退的核心处理逻辑
    async fn run_with_failover(
        &self,
        mut ctx: RequestContext,
    ) -> Result<RequestContext, StageError> {
        // 外层循环：换渠道（Level 3）
        loop {
            // 清理上次渠道的 Key 失败记录
            ctx.failed_keys.clear();

            // 中层循环：换 Key（Level 2）
            loop {
                ctx = select_channel::run(ctx).await?;
                ctx = transform_request::run(ctx).await?;

                // dispatch_upstream 内部包含 Level 1（重试）
                match dispatch_upstream::run(&self.runtime, ctx).await {
                    Ok(new_ctx) => {
                        // 成功！继续到响应转换
                        let ctx = transform_response::run(new_ctx).await?;
                        return Ok(ctx);
                    }
                    Err(stage_err) => {
                        // 记录失败的 Key
                        ctx = stage_err.context;
                        if let Some(ref key) = ctx.selected_api_key {
                            ctx.failed_keys.push(key.clone());
                        }
                        ctx.selected_api_key = None; // 避免复用

                        // 检查是否还有可用的 Key
                        let has_available_keys = ctx
                            .provider
                            .as_ref()
                            .map(|provider| {
                                provider.keys_vec().iter().any(|e| {
                                    e.enabled && !e.value.is_empty() && !ctx.failed_keys.contains(&e.value)
                                })
                            })
                            .unwrap_or(false);

                        if has_available_keys {
                            // Level 2：换 Key，继续中层循环
                            continue;
                        }

                        // 所有 Key 都试过了，记录 provider 失败
                        if let Some(ref p) = ctx.provider {
                            if !ctx.failed_providers.contains(&p.id) {
                                ctx.failed_providers.push(p.id.clone());
                            }
                        }
                        break; // 跳出中层循环，尝试换渠道
                    }
                }
            }

            // Level 3：换渠道（从 channels_available 选下一个）
            if !ctx.channels_available.is_empty() {
                // clone 用于失败回退场景，正常路径无额外开销
                if let Some(new_ctx) =
                    resolve_route::try_next_channel(&self.runtime, ctx.clone()).await
                {
                    ctx = new_ctx;
                    continue; // 外层循环，用新渠道重新尝试
                }
            }

            // 所有渠道 + 所有 Key 都失败
            return Err(StageError::new(
                ctx,
                GatewayError::Internal("所有渠道和 Key 均已失败".to_string()),
            ));
        }
    }
}
