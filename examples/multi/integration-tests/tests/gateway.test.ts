import { describe, it, expect } from "vitest";
import { echoDoClient, gatewayClient } from "./helpers";

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

describe("GatewayService.collectEchoes (client streaming via service binding)", () => {
  it("collects multiple messages via upstream client stream", async () => {
    const client = gatewayClient();
    const res = await client.collectEchoes({
      messages: ["alpha", "bravo", "charlie"],
    });
    expect(res.combined).toBe("alpha, bravo, charlie");
    expect(res.upstream).toBe("echo-worker");
  });

  it("handles a single message", async () => {
    const client = gatewayClient();
    const res = await client.collectEchoes({ messages: ["solo"] });
    expect(res.combined).toBe("solo");
    expect(res.upstream).toBe("echo-worker");
  });

  it("handles empty message list", async () => {
    const client = gatewayClient();
    const res = await client.collectEchoes({ messages: [] });
    expect(res.combined).toBe("");
    expect(res.upstream).toBe("echo-worker");
  });

  it("works over the JSON codec", async () => {
    const client = gatewayClient({ useBinaryFormat: false });
    const res = await client.collectEchoes({
      messages: ["one", "two"],
    });
    expect(res.combined).toBe("one, two");
    expect(res.upstream).toBe("echo-worker");
  });
});

describe("GatewayService.greetViaDO (e2e via Durable Object stub)", () => {
  it("greets a named user — proves the call hops through EchoDO", async () => {
    const client = gatewayClient();
    const res = await client.greetViaDO({ name: "Ada" });
    expect(res.greeting).toBe("Hello, Ada!");
    // `upstream` is "echo-do" (set by the DO), not "echo-worker".
    expect(res.upstream).toBe("echo-do");
  });

  it("falls back to 'world' when the name is empty", async () => {
    const client = gatewayClient();
    const res = await client.greetViaDO({ name: "" });
    expect(res.greeting).toBe("Hello, world!");
    expect(res.upstream).toBe("echo-do");
  });

  it("preserves multibyte unicode through the DO stub", async () => {
    const client = gatewayClient();
    const res = await client.greetViaDO({ name: "世界" });
    expect(res.greeting).toBe("Hello, 世界!");
    expect(res.upstream).toBe("echo-do");
  });

  it("works over the JSON codec", async () => {
    const client = gatewayClient({ useBinaryFormat: false });
    const res = await client.greetViaDO({ name: "Grace" });
    expect(res.greeting).toBe("Hello, Grace!");
    expect(res.upstream).toBe("echo-do");
  });
});

describe("EchoDO direct (e2e via DO outer fetch handler)", () => {
  it("echoes via the DO's outer fetch → stub → DO fetch chain", async () => {
    const client = echoDoClient();
    const res = await client.echo({ message: "ping" });
    expect(res.echoed).toBe("ping");
    expect(res.servedBy).toBe("echo-do");
  });

  it("works over JSON codec", async () => {
    const client = echoDoClient({ useBinaryFormat: false });
    const res = await client.echo({ message: "json-ping" });
    expect(res.echoed).toBe("json-ping");
    expect(res.servedBy).toBe("echo-do");
  });
});
