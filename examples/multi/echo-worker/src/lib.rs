//! Backend worker that serves `EchoService` over ConnectRPC.

use std::sync::Arc;

use connectrpc::{
    ConnectError, ConnectRpcBody, ConnectRpcService, Context as RpcContext, Router as RpcRouter,
};
use tower::Service;
use worker::{Context, Env, HttpRequest, event};

use buffa::view::OwnedView;
use multi_proto::echo::v1::{EchoRequestView, EchoResponse, EchoService, EchoServiceExt};

const SERVED_BY: &str = "echo-worker";

struct EchoImpl;

impl EchoService for EchoImpl {
    async fn echo(
        &self,
        ctx: RpcContext,
        request: OwnedView<EchoRequestView<'static>>,
    ) -> Result<(EchoResponse, RpcContext), ConnectError> {
        Ok((
            EchoResponse {
                echoed: request.message.to_string(),
                served_by: SERVED_BY.into(),
                ..Default::default()
            },
            ctx,
        ))
    }
}

#[event(fetch, respond_with_errors)]
async fn fetch(
    req: HttpRequest,
    _env: Env,
    _ctx: Context,
) -> worker::Result<http::Response<ConnectRpcBody>> {
    let router = Arc::new(EchoImpl).register(RpcRouter::new());
    let mut svc = ConnectRpcService::new(router);
    Ok(svc.call(req).await.unwrap())
}
