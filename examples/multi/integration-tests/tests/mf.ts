/**
 * Miniflare bootstrap wiring `gateway-worker` → `echo-worker` via a service
 * binding named `ECHO`. Tests dispatch fetches at the gateway; its handler
 * internally calls the echo worker over the binding using the Rust
 * `FetcherTransport`.
 */

import { Miniflare } from "miniflare";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..", "..");

function loadWorker(name: string) {
  const buildDir = resolve(repoRoot, name, "build");
  return {
    js: readFileSync(resolve(buildDir, "index.js"), "utf-8"),
    wasm: readFileSync(resolve(buildDir, "index_bg.wasm")),
  };
}

const echo = loadWorker("echo-worker");
const gateway = loadWorker("gateway-worker");

export const mf = new Miniflare({
  workers: [
    {
      name: "gateway-worker",
      modules: [
        { type: "ESModule", path: "index.js", contents: gateway.js },
        { type: "CompiledWasm", path: "index_bg.wasm", contents: gateway.wasm },
      ],
      compatibilityDate: "2026-04-22",
      // The Rust gateway looks up `env.service("ECHO")`. Miniflare wires
      // that to the second worker registered in this array.
      serviceBindings: { ECHO: "echo-worker" },
    },
    {
      name: "echo-worker",
      modules: [
        { type: "ESModule", path: "index.js", contents: echo.js },
        { type: "CompiledWasm", path: "index_bg.wasm", contents: echo.wasm },
      ],
      compatibilityDate: "2026-04-22",
    },
  ],
});

// `mf.ready` resolves once the gateway (the first worker) is bound to a
// localhost port. `dispatchFetch` always targets that first worker.
export const mfUrl = (await mf.ready).toString().replace(/\/$/, "");
