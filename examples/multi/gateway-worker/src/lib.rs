//! Front-door worker. Serves `GatewayService`; handlers internally call
//! `EchoService.Echo` on upstream workers using `FetcherTransport`.
//!
//! * `Greet` — calls `echo-worker` over a **service binding**.
//! * `GreetViaDO` — calls `EchoDO` (a Durable Object in `echo-do-worker`)
//!   via a DO stub cast to a `Fetcher` with [`FetcherTransport::from_stub`].
//!
//! This is the e2e proof of inter-service ConnectRPC over Cloudflare Workers
//! fetch, including Durable Objects.

#![allow(refining_impl_trait)]

use std::sync::Arc;

use connectrpc::client::ClientConfig;
use connectrpc::{
    ConnectError, ConnectRpcBody, ConnectRpcService, RequestContext, Response, Router as RpcRouter,
    ServiceResult,
};
use tower::Service;
use worker::{Context, Env, HttpRequest, event};

use buffa::view::OwnedView;
use connectrpc_workers::FetcherTransport;
use multi_proto::echo::v1::{EchoRequest, EchoServiceClient};
use multi_proto::gateway::v1::{
    CollectRequestView, CollectResponse, GatewayService, GatewayServiceExt, GreetRequestView,
    GreetResponse,
};

/// Service-binding name in `wrangler.toml` for the upstream echo worker.
const ECHO_BINDING: &str = "ECHO";

/// DO namespace binding name for the upstream EchoDO Durable Object.
const ECHO_DO_BINDING: &str = "ECHO_DO";

/// Sentinel base URI for the echo client. The authority is irrelevant for
/// service-binding fetches — the runtime routes via the binding, not DNS —
/// but ConnectRPC needs a syntactically-valid base URI for path construction.
const ECHO_BASE_URI: &str = "http://echo/";

struct GatewayImpl {
    echo: EchoServiceClient<FetcherTransport>,
    env: Env,
}

impl GatewayImpl {
    fn new(env: Env) -> worker::Result<Self> {
        let transport = FetcherTransport::new(env.service(ECHO_BINDING)?);
        let config = ClientConfig::new(ECHO_BASE_URI.parse().unwrap());

        Ok(Self {
            echo: EchoServiceClient::new(transport, config),
            env,
        })
    }

    fn echo_do_client(&self) -> Result<EchoServiceClient<FetcherTransport>, ConnectError> {
        let namespace = self
            .env
            .durable_object(ECHO_DO_BINDING)
            .map_err(|e| ConnectError::unavailable(format!("ECHO_DO binding: {e}")))?;
        let stub = namespace
            .get_by_name("singleton")
            .map_err(|e| ConnectError::unavailable(format!("ECHO_DO stub: {e}")))?;
        Ok(EchoServiceClient::new(
            FetcherTransport::from_stub(stub),
            ClientConfig::new(ECHO_BASE_URI.parse().unwrap()),
        ))
    }
}

impl GatewayService for GatewayImpl {
    async fn greet(
        &self,
        _ctx: RequestContext,
        request: OwnedView<GreetRequestView<'static>>,
    ) -> ServiceResult<GreetResponse> {
        let name = if request.name.is_empty() {
            "world"
        } else {
            request.name
        };

        let upstream = self
            .echo
            .echo(EchoRequest {
                message: format!("Hello, {name}!"),
                ..Default::default()
            })
            .await
            .map_err(|e| ConnectError::unavailable(format!("upstream echo call failed: {e}")))?;

        let response = upstream.into_owned();
        Response::ok(GreetResponse {
            greeting: response.echoed,
            upstream: response.served_by,
            ..Default::default()
        })
    }

    async fn greet_via_do(
        &self,
        _ctx: RequestContext,
        request: OwnedView<GreetRequestView<'static>>,
    ) -> ServiceResult<GreetResponse> {
        let name = if request.name.is_empty() {
            "world"
        } else {
            request.name
        };

        let echo_do = self.echo_do_client()?;
        let upstream = echo_do
            .echo(EchoRequest {
                message: format!("Hello, {name}!"),
                ..Default::default()
            })
            .await
            .map_err(|e| ConnectError::unavailable(format!("upstream echo-do call failed: {e}")))?;

        let response = upstream.into_owned();
        Response::ok(GreetResponse {
            greeting: response.echoed,
            upstream: response.served_by,
            ..Default::default()
        })
    }

    async fn collect_echoes(
        &self,
        _ctx: RequestContext,
        request: OwnedView<CollectRequestView<'static>>,
    ) -> ServiceResult<CollectResponse> {
        let messages: Vec<EchoRequest> = request
            .messages
            .iter()
            .map(|m| EchoRequest {
                message: m.to_string(),
                ..Default::default()
            })
            .collect();

        let upstream =
            self.echo.collect(messages).await.map_err(|e| {
                ConnectError::unavailable(format!("upstream collect call failed: {e}"))
            })?;

        let response = upstream.into_owned();
        Response::ok(CollectResponse {
            combined: response.echoed,
            upstream: response.served_by,
            ..Default::default()
        })
    }
}

#[event(fetch, respond_with_errors)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> worker::Result<http::Response<ConnectRpcBody>> {
    let gateway = GatewayImpl::new(env)?;
    let router = Arc::new(gateway).register(RpcRouter::new());
    let mut svc = ConnectRpcService::new(router);
    Ok(svc.call(req).await.unwrap())
}
