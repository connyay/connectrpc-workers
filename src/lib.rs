//! ConnectRPC [`ClientTransport`] implementations backed by the Cloudflare
//! Workers fetch APIs.
//!
//! Two transports are provided:
//!
//! * [`FetcherTransport`] wraps a [`worker::Fetcher`] (a `[[services]]`
//!   binding). Use it for inter-service calls within the same Cloudflare
//!   zone. The runtime short-circuits these requests, so there's no DNS
//!   lookup, no TLS handshake, and no trip out to the public internet.
//! * [`FetchTransport`] wraps the global [`worker::Fetch`] for arbitrary
//!   `http://` / `https://` URLs.
//!
//! # Sendness
//!
//! `ClientTransport` requires `Send + Sync + 'static` on the type and a
//! `Send + 'static` future. Workers' fetch is `!Send` (everything in
//! JS-land is `!Send`). We use [`worker::send::SendFuture`] /
//! [`worker::send::SendWrapper`] to satisfy the bound. workers-rs ships
//! these specifically because the Workers isolate is single-threaded, so
//! nothing is ever actually moved across threads.
//!
//! # Protocol
//!
//! Use [`connectrpc::Protocol::Connect`] (or `GrpcWeb`). Workers fetch
//! subrequests don't expose raw HTTP/2, so gRPC's trailer requirement
//! won't survive. Connect over HTTP/1.1 and GrpcWeb (trailers in body)
//! both work.
//!
//! # Example
//!
//! ```ignore
//! use connectrpc::client::ClientConfig;
//! use connectrpc::Protocol;
//! use connectrpc_workers::FetcherTransport;
//!
//! // Inside a #[event(fetch)] handler:
//! let echo = env.service("ECHO")?;
//! let transport = FetcherTransport::new(echo);
//! let config = ClientConfig::new("http://echo/".parse()?).protocol(Protocol::Connect);
//! let client = EchoServiceClient::new(transport, config);
//! let resp = client.echo(EchoRequest { message: "hi".into() }).await?;
//! ```

use connectrpc::client::{BoxFuture, ClientBody, ClientTransport};
use http::uri::{Authority, PathAndQuery, Scheme};
use http::{Request, Response, Uri};
use worker::send::{SendFuture, SendWrapper};
use worker::{Body, Fetch, Fetcher};

/// `ClientTransport` backed by a Workers service binding.
///
/// Construct with [`Self::new`]. Cloning is cheap because the underlying
/// `Fetcher` is just a `JsValue` handle.
#[derive(Clone)]
pub struct FetcherTransport {
    fetcher: SendWrapper<Fetcher>,
}

impl FetcherTransport {
    pub fn new(fetcher: Fetcher) -> Self {
        Self {
            fetcher: SendWrapper::new(fetcher),
        }
    }
}

impl std::fmt::Debug for FetcherTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetcherTransport").finish_non_exhaustive()
    }
}

impl ClientTransport for FetcherTransport {
    type ResponseBody = Body;
    type Error = worker::Error;

    fn send(
        &self,
        request: Request<ClientBody>,
    ) -> BoxFuture<'static, Result<Response<Self::ResponseBody>, Self::Error>> {
        // Clone the JsValue handle so the future owns it.
        let fetcher = (*self.fetcher).clone();
        Box::pin(SendFuture::new(async move {
            fetcher.fetch_request(request).await
        }))
    }
}

/// `ClientTransport` backed by the global Workers `fetch` (arbitrary URL).
///
/// Useful for hitting an external ConnectRPC server. For same-zone calls
/// prefer [`FetcherTransport`], since service bindings skip DNS and TLS
/// and don't count against egress.
///
/// The transport rewrites the request URI's scheme and authority to point
/// at `base`, so generated clients can keep using arbitrary `Uri`s in
/// their `ClientConfig` without caring where the service actually lives.
#[derive(Clone, Debug)]
pub struct FetchTransport {
    scheme: Scheme,
    authority: Authority,
}

impl FetchTransport {
    pub fn new(base: Uri) -> Result<Self, worker::Error> {
        let parts = base.into_parts();
        let scheme = parts.scheme.ok_or_else(|| {
            worker::Error::RustError("FetchTransport base URI is missing a scheme".into())
        })?;
        let authority = parts.authority.ok_or_else(|| {
            worker::Error::RustError("FetchTransport base URI is missing an authority".into())
        })?;
        Ok(Self { scheme, authority })
    }
}

impl ClientTransport for FetchTransport {
    type ResponseBody = Body;
    type Error = worker::Error;

    fn send(
        &self,
        request: Request<ClientBody>,
    ) -> BoxFuture<'static, Result<Response<Self::ResponseBody>, Self::Error>> {
        let scheme = self.scheme.clone();
        let authority = self.authority.clone();
        Box::pin(SendFuture::new(async move {
            let request = rewrite_uri(request, scheme, authority)?;
            let req = worker::Request::try_from(request)?;
            Fetch::Request(req).send().await.and_then(|resp| {
                let resp: http::Response<Body> = resp.try_into()?;
                Ok(resp)
            })
        }))
    }
}

fn rewrite_uri<B>(
    mut req: http::Request<B>,
    scheme: Scheme,
    authority: Authority,
) -> Result<http::Request<B>, worker::Error> {
    let path_and_query = req
        .uri()
        .path_and_query()
        .cloned()
        .unwrap_or_else(|| PathAndQuery::from_static("/"));
    let new_uri = Uri::builder()
        .scheme(scheme)
        .authority(authority)
        .path_and_query(path_and_query)
        .build()
        .map_err(|e| worker::Error::RustError(format!("rewrite uri: {e}")))?;
    *req.uri_mut() = new_uri;
    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_transport_rejects_uri_without_scheme() {
        // Path-only URIs parse fine but don't have enough info to send a
        // real request; we surface that at construction time, not at the
        // first call.
        let uri: Uri = "/some/path".parse().unwrap();
        let err = FetchTransport::new(uri).unwrap_err();
        assert!(format!("{err}").contains("scheme"));
    }

    #[test]
    fn fetch_transport_rejects_uri_without_authority() {
        // `mailto:` parses with a scheme but no authority.
        let uri: Uri = "mailto:user@example.com".parse().unwrap();
        let err = FetchTransport::new(uri).unwrap_err();
        let msg = format!("{err}");
        // Either authority or scheme could trigger first depending on
        // parsing; both are "this URI can't address an HTTP server."
        assert!(
            msg.contains("authority") || msg.contains("scheme"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn fetch_transport_accepts_https_uri() {
        let uri: Uri = "https://example.com:8443/base".parse().unwrap();
        let t = FetchTransport::new(uri).expect("valid base URI");
        assert_eq!(t.scheme, Scheme::HTTPS);
        assert_eq!(t.authority.as_str(), "example.com:8443");
    }

    #[test]
    fn rewrite_uri_replaces_scheme_and_authority_keeps_path() {
        let req: http::Request<()> = http::Request::builder()
            .uri("http://placeholder.invalid/foo/bar?x=1")
            .body(())
            .unwrap();
        let scheme = Scheme::HTTPS;
        let authority: Authority = "real.example:8443".parse().unwrap();
        let rewritten = rewrite_uri(req, scheme, authority).unwrap();
        let uri = rewritten.uri();
        assert_eq!(uri.scheme_str(), Some("https"));
        assert_eq!(
            uri.authority().map(|a| a.as_str()),
            Some("real.example:8443")
        );
        assert_eq!(uri.path(), "/foo/bar");
        assert_eq!(uri.query(), Some("x=1"));
    }

    #[test]
    fn rewrite_uri_defaults_path_to_root_when_missing() {
        // `http::Uri` parsed from just `http://host` has no path-and-query.
        let req: http::Request<()> = http::Request::builder()
            .uri("http://placeholder")
            .body(())
            .unwrap();
        let scheme = Scheme::HTTP;
        let authority: Authority = "real.example".parse().unwrap();
        let rewritten = rewrite_uri(req, scheme, authority).unwrap();
        assert_eq!(rewritten.uri().path(), "/");
    }

    /// Compile-time check that both transports satisfy `ClientTransport`,
    /// including its `Send + Sync + 'static` bound. If this stops compiling,
    /// one of the `SendWrapper` / `SendFuture` shims is no longer pulling
    /// its weight.
    #[test]
    fn transports_implement_client_transport() {
        fn assert_transport<T: ClientTransport>() {}
        assert_transport::<FetcherTransport>();
        assert_transport::<FetchTransport>();
    }
}
