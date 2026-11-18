/**
 * Miniflare bootstrap wiring:
 *   - `gateway-worker` → `echo-worker` via a service binding (`ECHO`)
 *   - `gateway-worker` → `echo-do-worker` via a DO namespace binding (`ECHO_DO`)
 *   - `echo-do-worker` hosts the `EchoDO` Durable Object class
 *
 * Tests dispatch fetches at the gateway; its handlers internally call the
 * upstream echo services using the Rust `FetcherTransport`.
 */

import { Miniflare } from "miniflare";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..", "..");

/** Module manifest for a worker-build output directory. */
function loadWorker(name: string) {
  const buildDir = resolve(repoRoot, name, "build");
  return {
    mainModule: "index.js",
    modules: {
      "index.js": {
        type: "esm" as const,
        contents: readFileSync(resolve(buildDir, "index.js"), "utf-8"),
      },
      "index_bg.wasm": {
        type: "wasm" as const,
        contents: new Uint8Array(readFileSync(resolve(buildDir, "index_bg.wasm"))),
      },
    },
  };
}

const compatibilityDate = "2026-04-22";

export const mf = new Miniflare({
  workers: [
    {
      config: {
        name: "gateway-worker",
        compatibilityDate,
        manifest: loadWorker("gateway-worker"),
        env: {
          // The Rust gateway looks up `env.service("ECHO")`. Miniflare wires
          // that to the echo-worker registered below.
          ECHO: { type: "worker", worker: "echo-worker" },
          // DO namespace binding: the gateway calls `env.durable_object("ECHO_DO")`
          // to get a stub, which it casts to a Fetcher for FetcherTransport.
          ECHO_DO: {
            type: "durable-object",
            worker: "echo-do-worker",
            exportName: "EchoDO",
          },
        },
      },
    },
    {
      config: {
        name: "echo-worker",
        compatibilityDate,
        manifest: loadWorker("echo-worker"),
      },
    },
    {
      config: {
        name: "echo-do-worker",
        compatibilityDate,
        manifest: loadWorker("echo-do-worker"),
        // Matches the `new_classes` (KV-backed) migration in wrangler.toml.
        exports: {
          EchoDO: { type: "durable-object", storage: "legacy-kv" },
        },
        env: {
          // Local DO binding so the outer fetch handler can route into the DO.
          ECHO_DO: {
            type: "durable-object",
            worker: "echo-do-worker",
            exportName: "EchoDO",
          },
        },
      },
    },
  ],
});

// `mf.ready` resolves once the gateway (the first worker) is bound to a
// localhost port. `dispatchFetch` always targets that first worker.
export const mfUrl = (await mf.ready).toString().replace(/\/$/, "");
