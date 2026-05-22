//! Durable Object that serves `EchoService` over ConnectRPC.
//!
//! The DO's `fetch` handler converts the incoming `worker::Request` to an
//! `http::Request`, routes it through the ConnectRPC service, and converts
//! the response back.
//!
//! The outer `#[event(fetch)]` handler proxies every request into a
//! singleton DO instance, demonstrating `FetcherTransport::from_stub`.

#![allow(refining_impl_trait)]

use std::sync::Arc;

use connectrpc::{
    ConnectRpcBody, ConnectRpcService, RequestContext, Response, Router as RpcRouter,
    ServiceResult, ServiceStream,
};
use futures::StreamExt;
use tower::Service;
use worker::{Context, DurableObject, Env, Fetcher, HttpRequest, State, durable_object, event};

use buffa::view::OwnedView;
use multi_proto::echo::v1::{EchoRequestView, EchoResponse, EchoService, EchoServiceExt};

const SERVED_BY: &str = "echo-do";

struct EchoImpl;

impl EchoService for EchoImpl {
    async fn echo(
        &self,
        _ctx: RequestContext,
        request: OwnedView<EchoRequestView<'static>>,
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
        mut requests: ServiceStream<OwnedView<EchoRequestView<'static>>>,
    ) -> ServiceResult<EchoResponse> {
        let mut parts = Vec::new();
        while let Some(req) = requests.next().await {
            let req = req?;
            parts.push(req.message.to_string());
        }
        Response::ok(EchoResponse {
            echoed: parts.join(", "),
            served_by: SERVED_BY.into(),
            ..Default::default()
        })
    }
}

#[durable_object]
pub struct EchoDO {
    #[allow(dead_code)]
    state: State,
    #[allow(dead_code)]
    env: Env,
}

impl DurableObject for EchoDO {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, req: worker::Request) -> worker::Result<worker::Response> {
        let http_req: HttpRequest = req.try_into()?;
        let router = Arc::new(EchoImpl).register(RpcRouter::new());
        let mut svc = ConnectRpcService::new(router);
        let http_resp: http::Response<ConnectRpcBody> = svc.call(http_req).await.unwrap();
        http_resp.try_into()
    }
}

/// The outer fetch handler proxies every request into the `EchoDO` Durable
/// Object. This demonstrates the stub-to-Fetcher cast that makes
/// `FetcherTransport` work with Durable Objects.
#[event(fetch, respond_with_errors)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> worker::Result<http::Response<worker::Body>> {
    let namespace = env.durable_object("ECHO_DO")?;
    let stub = namespace.get_by_name("singleton")?;
    // DurableObjectStub extends Fetcher in the Workers runtime, so we can
    // cast it and use the same FetcherTransport the library provides for
    // service bindings. The cast is what `from_stub` does internally.
    let fetcher: Fetcher = stub.into_rpc();
    fetcher.fetch_request(req).await
}
