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

function loadWorker(name: string) {
  const buildDir = resolve(repoRoot, name, "build");
  return {
    js: readFileSync(resolve(buildDir, "index.js"), "utf-8"),
    wasm: readFileSync(resolve(buildDir, "index_bg.wasm")),
  };
}

const echo = loadWorker("echo-worker");
const gateway = loadWorker("gateway-worker");
const echoDo = loadWorker("echo-do-worker");

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
      // that to the echo-worker registered below.
      serviceBindings: { ECHO: "echo-worker" },
      // DO namespace binding: the gateway calls `env.durable_object("ECHO_DO")`
      // to get a stub, which it casts to a Fetcher for FetcherTransport.
      durableObjects: {
        ECHO_DO: { className: "EchoDO", scriptName: "echo-do-worker" },
      },
    },
    {
      name: "echo-worker",
      modules: [
        { type: "ESModule", path: "index.js", contents: echo.js },
        { type: "CompiledWasm", path: "index_bg.wasm", contents: echo.wasm },
      ],
      compatibilityDate: "2026-04-22",
    },
    {
      name: "echo-do-worker",
      modules: [
        { type: "ESModule", path: "index.js", contents: echoDo.js },
        { type: "CompiledWasm", path: "index_bg.wasm", contents: echoDo.wasm },
      ],
      compatibilityDate: "2026-04-22",
      // Local DO binding so the outer fetch handler can route into the DO.
      durableObjects: {
        ECHO_DO: "EchoDO",
      },
    },
  ],
});

// `mf.ready` resolves once the gateway (the first worker) is bound to a
// localhost port. `dispatchFetch` always targets that first worker.
export const mfUrl = (await mf.ready).toString().replace(/\/$/, "");
