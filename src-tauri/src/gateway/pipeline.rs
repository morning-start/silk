use axum::body::Body;
use axum::http::Request;
use axum::response::Response;

use crate::gateway::context::{GatewayContext, RequestContext};
use crate::gateway::error::GatewayError;
use crate::gateway::middleware::{
    dispatch_upstream, extract, finalize, normalize_protocol, persist_log, resolve_route,
    transform_request, transform_response,
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
        let result = self.run_main(base_ctx, body).await;

        match result {
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

    async fn run_main(&self, ctx: RequestContext, body: Body) -> Result<RequestContext, StageError> {
        let ctx = extract::read_body(ctx, body).await?;
        let ctx = resolve_route::run(&self.runtime, ctx).await?;
        let ctx = normalize_protocol::run(ctx).await?;
        let ctx = transform_request::run(ctx).await?;
        let ctx = dispatch_upstream::run(&self.runtime, ctx).await?;
        let ctx = transform_response::run(ctx).await?;
        Ok(ctx)
    }
}
