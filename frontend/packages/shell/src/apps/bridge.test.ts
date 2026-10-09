import { describe, expect, test, vi } from "vitest";
import { BRIDGE_CAPABILITY_LIST, parseBridgeMessage, RUMAHL_BRIDGE } from "@rumahl/contracts/bridge";
import { handleBridgeRequest, type AppBridgeHandlers } from "./bridge";

function handlers(overrides: Partial<AppBridgeHandlers> = {}): AppBridgeHandlers {
  return {
    appId: "com.example.notes", appTitle: "Notes", version: "1.2.3", accountName: "Kaim", mode: "desktop",
    capabilities: BRIDGE_CAPABILITY_LIST,
    theme: () => ({ scheme: "light", accent: "#28694c", reducedMotion: false }),
    notify: vi.fn(), windowControl: vi.fn(),
    ...overrides
  };
}
const request = (id: string, method: string, params?: unknown) => ({ bridge: RUMAHL_BRIDGE, kind: "request" as const, id, method, ...(params === undefined ? {} : { params }) });

describe("bridge contract", () => {
  test("parses valid messages and rejects junk", () => {
    expect(parseBridgeMessage({ bridge: RUMAHL_BRIDGE, kind: "hello" })).toEqual({ bridge: RUMAHL_BRIDGE, kind: "hello" });
    expect(parseBridgeMessage({ bridge: RUMAHL_BRIDGE, kind: "request", id: "1", method: "os.info" })).toMatchObject({ kind: "request", id: "1", method: "os.info" });
    expect(parseBridgeMessage(null)).toBeNull();
    expect(parseBridgeMessage({ kind: "hello" })).toBeNull();
    expect(parseBridgeMessage({ bridge: RUMAHL_BRIDGE, kind: "request", id: "", method: "os.info" })).toBeNull();
    expect(parseBridgeMessage({ bridge: RUMAHL_BRIDGE, kind: "request", id: "1", method: "x".repeat(100) })).toBeNull();
    expect(parseBridgeMessage({ bridge: RUMAHL_BRIDGE, kind: "nope" })).toBeNull();
  });
});

describe("bridge request handling", () => {
  test("os.info reports the app, account and mode", () => {
    expect(handleBridgeRequest(request("1", "os.info"), handlers())).toMatchObject({
      kind: "response", id: "1", ok: true,
      result: { app: { id: "com.example.notes", title: "Notes", version: "1.2.3" }, account: { displayName: "Kaim" }, mode: "desktop" }
    });
  });
  test("os.theme.get returns the theme", () => {
    expect(handleBridgeRequest(request("2", "os.theme.get"), handlers())).toMatchObject({ ok: true, result: { scheme: "light" } });
  });
  test("os.notification forwards a validated notification", () => {
    const notify = vi.fn();
    const response = handleBridgeRequest(request("3", "os.notification", { message: "Done", variant: "success" }), handlers({ notify }));
    expect(response.ok).toBe(true);
    expect(notify).toHaveBeenCalledWith({ message: "Done", variant: "success" });
  });
  test("os.notification rejects empty params", () => {
    const notify = vi.fn();
    expect(handleBridgeRequest(request("4", "os.notification", {}), handlers({ notify }))).toMatchObject({ ok: false, error: { code: "invalid-params" } });
    expect(notify).not.toHaveBeenCalled();
  });
  test("window controls dispatch the action", () => {
    const cases = [["os.window.close", "close"], ["os.window.minimize", "minimize"], ["os.window.focus", "focus"]] as const;
    for (const [method, action] of cases) {
      const windowControl = vi.fn();
      expect(handleBridgeRequest(request("5", method), handlers({ windowControl })).ok).toBe(true);
      expect(windowControl).toHaveBeenCalledWith(action);
    }
  });
  test("unknown methods fail closed", () => {
    expect(handleBridgeRequest(request("6", "os.evil"), handlers())).toMatchObject({ ok: false, error: { code: "unknown-method" } });
  });
  test("refuses methods whose capability the app did not declare", () => {
    expect(handleBridgeRequest(request("7", "os.notification", { message: "hi" }), handlers({ capabilities: [] }))).toMatchObject({ ok: false, error: { code: "denied" } });
    expect(handleBridgeRequest(request("8", "os.window.close"), handlers({ capabilities: [] }))).toMatchObject({ ok: false, error: { code: "denied" } });
    expect(handleBridgeRequest(request("9", "os.info"), handlers({ capabilities: ["com.rumahl.os.info"] })).ok).toBe(true);
  });
});
