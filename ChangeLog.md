# connectrpc-workers Change Log

## Version 0.2.0 - 2026-05-06

- Bump `connectrpc` dependency from 0.3 to 0.4 and `buffa` from 0.3 to 0.5.
- Client streaming now works end-to-end on Workers. The multi-worker example
  exercises this: the gateway's `CollectEchoes` RPC client-streams messages to
  the echo worker's `Collect` RPC via a service binding, verified by
  vitest + miniflare integration tests.

## Version 0.1.0 - 2026-04-26

Initial release.

- `FetcherTransport`: a `connectrpc::client::ClientTransport` over a Workers
  service binding (`worker::Fetcher`). Use it for inter-service calls within
  the same Cloudflare zone. The runtime short-circuits the request, so it
  doesn't go through DNS, TLS, or the public internet.
- `FetchTransport`: same trait, backed by the global `worker::Fetch` for
  arbitrary `http://` / `https://` URLs. It rewrites the request URI's
  scheme and authority to the configured base, so generated clients don't
  need to know the real upstream URL.
- Both transports satisfy `ClientTransport`'s `Send + Sync + 'static`
  bounds via `worker::send::SendFuture` and `worker::send::SendWrapper`,
  which is sound on Workers because the isolate is single-threaded.
- Tested against `connectrpc 0.3` and `worker 0.8`.
- End-to-end example (two Rust workers talking ConnectRPC over a service
  binding, exercised by vitest + miniflare):
  <https://github.com/connyay/workers-connectrpc-multi>
