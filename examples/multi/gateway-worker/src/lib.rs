//! Front-door worker. Serves `GatewayService.Greet`; the handler internally
//! calls `EchoService.Echo` on `echo-worker` over a service binding using
//! the `connectrpc-workers::FetcherTransport`.
//!
//! This is the e2e proof of inter-service ConnectRPC over Cloudflare Workers
//! fetch.

use std::sync::Arc;

use connectrpc::Protocol;
use connectrpc::client::ClientConfig;
use connectrpc::{
    ConnectError, ConnectRpcBody, ConnectRpcService, Context as RpcContext, Router as RpcRouter,
};
use tower::Service;
use worker::{Context, Env, HttpRequest, event};

use buffa::view::OwnedView;
use connectrpc_workers::FetcherTransport;
use multi_proto::echo::v1::{EchoRequest, EchoServiceClient};
use multi_proto::gateway::v1::{
    GatewayService, GatewayServiceExt, GreetRequestView, GreetResponse,
};

/// Service-binding name in `wrangler.toml` for the upstream echo worker.
const ECHO_BINDING: &str = "ECHO";

/// Sentinel base URI for the echo client. The authority is irrelevant for
/// service-binding fetches — the runtime routes via the binding, not DNS —
/// but ConnectRPC needs a syntactically-valid base URI for path construction.
const ECHO_BASE_URI: &str = "http://echo/";

struct GatewayImpl {
    echo: EchoServiceClient<FetcherTransport>,
}

impl GatewayImpl {
    fn new(env: &Env) -> worker::Result<Self> {
        let transport = FetcherTransport::new(env.service(ECHO_BINDING)?);
        let config = ClientConfig::new(ECHO_BASE_URI.parse().unwrap()).protocol(Protocol::Connect);
        Ok(Self {
            echo: EchoServiceClient::new(transport, config),
        })
    }
}

impl GatewayService for GatewayImpl {
    async fn greet(
        &self,
        ctx: RpcContext,
        request: OwnedView<GreetRequestView<'static>>,
    ) -> Result<(GreetResponse, RpcContext), ConnectError> {
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
        Ok((
            GreetResponse {
                greeting: response.echoed,
                upstream: response.served_by,
                ..Default::default()
            },
            ctx,
        ))
    }
}

#[event(fetch, respond_with_errors)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> worker::Result<http::Response<ConnectRpcBody>> {
    let gateway = GatewayImpl::new(&env)?;
    let router = Arc::new(gateway).register(RpcRouter::new());
    let mut svc = ConnectRpcService::new(router);
    Ok(svc.call(req).await.unwrap())
}
