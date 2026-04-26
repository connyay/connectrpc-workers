import { describe, it, expect } from "vitest";
import { gatewayClient } from "./helpers";

describe("GatewayService.greet (e2e via service binding)", () => {
  it("greets a named user — proves the call hops through echo-worker", async () => {
    const client = gatewayClient();
    const res = await client.greet({ name: "Ada" });
    expect(res.greeting).toBe("Hello, Ada!");
    // `upstream` is set by echo-worker only — receiving it proves the
    // FetcherTransport really round-tripped through the binding.
    expect(res.upstream).toBe("echo-worker");
  });

  it("falls back to 'world' when the name is empty", async () => {
    const client = gatewayClient();
    const res = await client.greet({ name: "" });
    expect(res.greeting).toBe("Hello, world!");
    expect(res.upstream).toBe("echo-worker");
  });

  it("preserves multibyte unicode names round-tripping through the binding", async () => {
    const client = gatewayClient();
    const res = await client.greet({ name: "世界" });
    expect(res.greeting).toBe("Hello, 世界!");
    expect(res.upstream).toBe("echo-worker");
  });

  it("works over the JSON codec too (Connect-protocol over fetch)", async () => {
    const client = gatewayClient({ useBinaryFormat: false });
    const res = await client.greet({ name: "Grace" });
    expect(res.greeting).toBe("Hello, Grace!");
    expect(res.upstream).toBe("echo-worker");
  });
});
