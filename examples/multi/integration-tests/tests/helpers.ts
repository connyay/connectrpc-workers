import { createClient, type Client, type Transport } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import type { DescService } from "@bufbuild/protobuf";

import { mf, mfUrl } from "./mf";

import { GatewayService } from "../gen/multi/gateway/v1/gateway_pb.js";
import { EchoService } from "../gen/multi/echo/v1/echo_pb.js";

export interface TransportOptions {
  /** false → JSON codec, true (default) → binary protobuf. */
  useBinaryFormat?: boolean;
}

function transportFor(
  opts: TransportOptions = {},
  fetchFn?: typeof globalThis.fetch,
): Transport {
  return createConnectTransport({
    baseUrl: mfUrl,
    useBinaryFormat: opts.useBinaryFormat ?? true,
    fetch:
      fetchFn ??
      (((input, init) =>
        mf.dispatchFetch(input as string, init)) as typeof globalThis.fetch),
  });
}

function clientFor<S extends DescService>(
  service: S,
  opts?: TransportOptions,
  fetchFn?: typeof globalThis.fetch,
): Client<S> {
  return createClient(service, transportFor(opts, fetchFn));
}

/** Hits `gateway-worker` directly. The gateway's handler internally calls
 *  `echo-worker` via the `ECHO` service binding. */
export const gatewayClient = (opts?: TransportOptions) =>
  clientFor(GatewayService, opts);

/** Hits `echo-do-worker` directly. The outer fetch handler proxies into the
 *  `EchoDO` Durable Object which runs EchoService over ConnectRPC. */
const echoDoWorker = await mf.getWorker("echo-do-worker");
export const echoDoClient = (opts?: TransportOptions) =>
  clientFor(
    EchoService,
    opts,
    ((input, init) =>
      echoDoWorker.fetch(input as string, init)) as typeof globalThis.fetch,
  );
