import { afterEach, describe, expect, test, vi } from "vitest";
import { RUMAHL_BRIDGE } from "@rumahl/contracts/bridge";
import { attachProviderChannel, type ProviderChannel } from "./provider-channel";
import { invokeProvider, registerProviderChannel } from "./providers";

function fakeFrame(): { iframe: HTMLIFrameElement; posted: Record<string, unknown>[]; source: Window } {
  const posted: Record<string, unknown>[] = [];
  const source = { postMessage: (message: Record<string, unknown>) => posted.push(message) } as unknown as Window;
  const iframe = { contentWindow: source } as unknown as HTMLIFrameElement;
  return { iframe, posted, source };
}
function deliver(source: Window, data: unknown): void {
  window.dispatchEvent(new MessageEvent("message", { data, source: source as unknown as MessageEventSource }));
}
async function waitRequest(posted: Record<string, unknown>[]): Promise<Record<string, unknown>> {
  await vi.waitFor(() => expect(posted.some((message) => message.kind === "request")).toBe(true));
  return posted.find((message) => message.kind === "request")!;
}

let active: ProviderChannel | null = null;
afterEach(() => { active?.dispose(); active = null; });

describe("provider channel", () => {
  test("delivers a capability invocation and resolves the provider result", async () => {
    const { iframe, posted, source } = fakeFrame();
    const channel = active = attachProviderChannel(iframe, { timeoutMs: 200 });
    const promise = channel.invoke("com.example.notes.search", { namespace: "rumahl.files", kind: "file", key: "a" });
    const request = await waitRequest(posted);
    expect(request).toMatchObject({ bridge: RUMAHL_BRIDGE, method: "provider.invoke", params: { capability: "com.example.notes.search" } });
    deliver(source, { bridge: RUMAHL_BRIDGE, kind: "response", id: request.id, ok: true, result: { hits: 2 } });
    await expect(promise).resolves.toEqual({ hits: 2 });
  });

  test("rejects on a provider error and when the provider does not answer", async () => {
    const { iframe, posted, source } = fakeFrame();
    const channel = active = attachProviderChannel(iframe, { timeoutMs: 200 });
    const rejected = channel.invoke("com.example.notes.search");
    const request = await waitRequest(posted);
    deliver(source, { bridge: RUMAHL_BRIDGE, kind: "response", id: request.id, ok: false, error: { code: "no-provider", message: "not provided" } });
    await expect(rejected).rejects.toThrow("not provided");
    await expect(channel.invoke("com.example.notes.search")).rejects.toThrow("did not answer");
  });

  test("waits for the handshake before delivering", async () => {
    const { iframe, posted, source } = fakeFrame();
    let handshake!: () => void;
    const ready = new Promise<void>((resolve) => { handshake = resolve; });
    const channel = active = attachProviderChannel(iframe, { ready, timeoutMs: 200 });
    const promise = channel.invoke("com.example.notes.search");
    await Promise.resolve();
    expect(posted).toHaveLength(0);
    handshake();
    const request = await waitRequest(posted);
    deliver(source, { bridge: RUMAHL_BRIDGE, kind: "response", id: request.id, ok: true, result: "ok" });
    await expect(promise).resolves.toBe("ok");
  });

  test("rejects when the provider never becomes ready", async () => {
    const { iframe } = fakeFrame();
    const channel = active = attachProviderChannel(iframe, { ready: new Promise<void>(() => undefined), timeoutMs: 20 });
    await expect(channel.invoke("com.example.notes.search")).rejects.toThrow("not ready");
  });
});

describe("provider registry", () => {
  test("routes invocations to an open provider and rejects unknown ones", async () => {
    const channel: ProviderChannel = { invoke: vi.fn(async () => "ok"), dispose: vi.fn() };
    const unregister = registerProviderChannel("com.example.notes", channel);
    await expect(invokeProvider("com.example.notes", "cap", undefined)).resolves.toBe("ok");
    expect(channel.invoke).toHaveBeenCalledWith("cap", undefined);
    unregister();
    await expect(invokeProvider("com.example.notes", "cap")).rejects.toThrow("not running");
  });
});
