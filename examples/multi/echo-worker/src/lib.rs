//! Backend worker that serves `EchoService` over ConnectRPC.

#![allow(refining_impl_trait)]

use std::sync::Arc;

use connectrpc::{
    ConnectRpcBody, ConnectRpcService, RequestContext, Response, Router as RpcRouter,
    ServiceRequest, ServiceResult, ServiceStream, StreamMessage,
};
use futures::StreamExt;
use tower::Service;
use worker::{Context, Env, HttpRequest, event};

use multi_proto::echo::v1::{EchoRequest, EchoResponse, EchoService, EchoServiceExt};

const SERVED_BY: &str = "echo-worker";

struct EchoImpl;

impl EchoService for EchoImpl {
    async fn echo(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, EchoRequest>,
    ) -> ServiceResult<EchoResponse> {
        Response::ok(EchoResponse {
            echoed: request.message.to_string(),
            served_by: SERVED_BY.into(),
            ..Default::default()
        })
    }

    async fn collect(
        &self,
        _ctx: RequestContext,
        mut requests: ServiceStream<StreamMessage<EchoRequest>>,
    ) -> ServiceResult<EchoResponse> {
        let mut parts = Vec::new();
        while let Some(req) = requests.next().await {
            let req = req?;
            parts.push(req.message().to_string());
        }
        Response::ok(EchoResponse {
            echoed: parts.join(", "),
            served_by: SERVED_BY.into(),
            ..Default::default()
        })
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
