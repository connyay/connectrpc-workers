import { createClient, type Client, type Transport } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import type { DescService } from "@bufbuild/protobuf";

import { mf, mfUrl } from "./mf";

import { GatewayService } from "../gen/multi/gateway/v1/gateway_pb.js";

export interface TransportOptions {
  /** false → JSON codec, true (default) → binary protobuf. */
  useBinaryFormat?: boolean;
}

function transportFor(opts: TransportOptions = {}): Transport {
  return createConnectTransport({
    baseUrl: mfUrl,
    useBinaryFormat: opts.useBinaryFormat ?? true,
    fetch: ((input, init) =>
      mf.dispatchFetch(input as string, init)) as typeof globalThis.fetch,
  });
}

function clientFor<S extends DescService>(
  service: S,
  opts?: TransportOptions,
): Client<S> {
  return createClient(service, transportFor(opts));
}

/** Hits `gateway-worker` directly. The gateway's handler internally calls
 *  `echo-worker` via the `ECHO` service binding. */
export const gatewayClient = (opts?: TransportOptions) =>
  clientFor(GatewayService, opts);
