# multi: end-to-end ConnectRPC over Workers

Two Cloudflare Workers in Rust talking to each other over ConnectRPC,
wired up with the [`connectrpc-workers`](../../) crate at the root of
this repo.

## Layout

```
examples/multi/
├── proto-crate/         shared generated bindings (build.rs → connectrpc-build)
│   └── proto/multi/{echo,gateway}/v1/*.proto
├── echo-worker/         backend worker, serves EchoService
├── gateway-worker/      front worker, serves GatewayService;
│                        handler calls echo-worker via FetcherTransport
└── integration-tests/   vitest + miniflare, both workers + service binding
```

## How it's wired

`gateway-worker` depends on `connectrpc-workers` (the path dep at the
workspace root) and builds a `FetcherTransport` from a service binding
declared in its `wrangler.toml`:

```rust
use connectrpc::Protocol;
use connectrpc::client::ClientConfig;
use connectrpc_workers::FetcherTransport;
use multi_proto::echo::v1::EchoServiceClient;

let transport = FetcherTransport::new(env.service("ECHO")?);
let config = ClientConfig::new("http://echo/".parse()?).protocol(Protocol::Connect);
let echo = EchoServiceClient::new(transport, config);
let resp = echo.echo(EchoRequest { message: "hi".into(), ..Default::default() }).await?;
```

## Running the integration tests

```bash
cd examples/multi/integration-tests
npm install
npm test
```

`npm test` runs `pretest` first, which:

1. `buf generate`s the TS bindings into `gen/`
2. builds both workers via `worker-build --release` into `<worker>/build/`
3. then runs `vitest`

`tests/mf.ts` boots Miniflare with both compiled workers and a
`serviceBindings: { ECHO: "echo-worker" }` entry on the gateway. The
tests fire fetches at the gateway and assert on `upstream` to prove the
inter-service hop actually happened.

## Caveats

- Stick to `Protocol::Connect` (or `GrpcWeb`). Workers fetch subrequests
  don't expose raw HTTP/2, so gRPC's trailer requirement won't survive.
  Connect over HTTP/1.1 and GrpcWeb (trailers in body) both work.
- Each call counts as one subrequest (50 free / 1000 paid).
- Server-streaming and client-streaming should work. Bidi over a single
  fetch isn't exercised here, so verify it yourself before relying on it.
