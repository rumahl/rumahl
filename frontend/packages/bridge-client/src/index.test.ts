import { afterEach, describe, expect, test, vi } from "vitest";
import { RUMAHL_BRIDGE } from "@rumahl/contracts/bridge";
import { connectRumahlBridge, RumahlBridgeError, type RumahlBridge } from "./index";

function fakeParent(): { target: Window; posted: Record<string, unknown>[] } {
  const posted: Record<string, unknown>[] = [];
  const target = { postMessage: (message: Record<string, unknown>) => { posted.push(message); } } as unknown as Window;
  return { target, posted };
}
function deliver(target: Window, data: unknown): void {
  window.dispatchEvent(new MessageEvent("message", { data, source: target as unknown as MessageEventSource }));
}
const welcome = (appId = "com.example.notes") => ({ bridge: RUMAHL_BRIDGE, kind: "welcome", version: 1, appId, methods: [] as string[] });
const response = (id: string, result: unknown) => ({ bridge: RUMAHL_BRIDGE, kind: "response", id, ok: true, result });

let active: RumahlBridge | null = null;
afterEach(() => { active?.dispose(); active = null; });

describe("connectRumahlBridge", () => {
  test("performs the handshake and becomes available", async () => {
    const { target, posted } = fakeParent();
    const bridge = active = connectRumahlBridge({ target, timeoutMs: 100 });
    expect(bridge.available).toBe(true);
    expect(posted.some((message) => message.kind === "hello")).toBe(true);
    deliver(target, welcome("com.example.notes"));
    await expect(bridge.ready()).resolves.toMatchObject({ appId: "com.example.notes" });
    expect(bridge.appId).toBe("com.example.notes");
  });

  test("sends typed requests and resolves responses", async () => {
    const { target, posted } = fakeParent();
    const bridge = active = connectRumahlBridge({ target, timeoutMs: 100 });
    const promise = bridge.info();
    const request = posted.find((message) => message.method === "os.info")!;
    deliver(target, response(request.id as string, { app: { id: "com.example.notes", title: "Notes", version: "1" }, account: { displayName: "Kaim" }, mode: "desktop" }));
    await expect(promise).resolves.toMatchObject({ mode: "desktop" });
  });

  test("rejects when the shell reports an error", async () => {
    const { target, posted } = fakeParent();
    const bridge = active = connectRumahlBridge({ target, timeoutMs: 100 });
    const promise = bridge.notify({ message: "hi" });
    const request = posted.find((message) => message.method === "os.notification")!;
    deliver(target, { bridge: RUMAHL_BRIDGE, kind: "response", id: request.id, ok: false, error: { code: "unknown-method", message: "nope" } });
    await expect(promise).rejects.toBeInstanceOf(RumahlBridgeError);
  });

  test("invokes a capability and carries the resource", async () => {
    const { target, posted } = fakeParent();
    const bridge = active = connectRumahlBridge({ target, timeoutMs: 100 });
    const promise = bridge.invokeCapability("rumahl.files.preview", { namespace: "rumahl.files", kind: "file", key: "a" });
    const request = posted.find((message) => message.method === "os.capabilities.invoke")!;
    expect(request.params).toMatchObject({ capability: "rumahl.files.preview", resource: { namespace: "rumahl.files", kind: "file", key: "a" } });
    deliver(target, response(request.id as string, { capability: "rumahl.files.preview", outcome: "invoked" }));
    await expect(promise).resolves.toEqual({ capability: "rumahl.files.preview", outcome: "invoked" });
  });

  test("times out unanswered requests", async () => {
    const { target } = fakeParent();
    const bridge = active = connectRumahlBridge({ target, timeoutMs: 20 });
    await expect(bridge.theme()).rejects.toMatchObject({ code: "timeout" });
  });

  test("dispatches OS events to subscribers", async () => {
    const { target } = fakeParent();
    const bridge = active = connectRumahlBridge({ target, timeoutMs: 100 });
    const listener = vi.fn();
    const off = bridge.on("os.theme.changed", listener);
    deliver(target, { bridge: RUMAHL_BRIDGE, kind: "event", topic: "os.theme.changed", payload: { scheme: "dark" } });
    expect(listener).toHaveBeenCalledWith({ scheme: "dark" });
    off();
    deliver(target, { bridge: RUMAHL_BRIDGE, kind: "event", topic: "os.theme.changed", payload: { scheme: "light" } });
    expect(listener).toHaveBeenCalledTimes(1);
  });

  test("ignores other sources and reports unavailability outside the shell", async () => {
    const { target } = fakeParent();
    const bridge = active = connectRumahlBridge({ target, timeoutMs: 100 });
    const other = { postMessage: vi.fn() } as unknown as Window;
    deliver(other, welcome());
    await Promise.resolve();
    expect(bridge.appId).toBeNull();
    const standalone = connectRumahlBridge({ target: window });
    expect(standalone.available).toBe(false);
    await expect(standalone.info()).rejects.toMatchObject({ code: "not-embedded" });
    standalone.dispose();
  });
});
