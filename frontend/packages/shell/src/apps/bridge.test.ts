import { describe, expect, test, vi } from "vitest";
import { BRIDGE_CAPABILITIES, BRIDGE_CAPABILITY_LIST, parseBridgeMessage, RUMAHL_BRIDGE } from "@rumahl/contracts/bridge";
import { handleBridgeRequest, handleBridgeRequestAsync, type AppBridgeHandlers } from "./bridge";

function handlers(overrides: Partial<AppBridgeHandlers> = {}): AppBridgeHandlers {
  return {
    appId: "com.example.notes", appTitle: "Notes", version: "1.2.3", accountName: "Kaim", mode: "desktop",
    capabilities: BRIDGE_CAPABILITY_LIST,
    theme: () => ({ scheme: "light", accent: "#28694c", reducedMotion: false, palette: { background: "#ffffff", surface: "#f1f5f9", text: "#0f172a", textMuted: "#64748b", border: "#cbd5e1", accent: "#28694c", onAccent: "#ffffff" } }),
    notify: vi.fn(), windowControl: vi.fn(),
    invokeCapability: vi.fn(async (capability: string) => ({ capability, outcome: "invoked" as const })),
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
  test("forwards rich app notifications with images, metadata and buttons", () => {
    const notify = vi.fn();
    const response = handleBridgeRequest(request("10", "os.notification", {
      title: "Saved",
      message: "Your changes were written.",
      description: "All files are safe.",
      variant: "success",
      image: "https://example.test/photo.png",
      meta: "now",
      progress: 0.5,
      duration: 5000,
      buttons: [{ id: "undo", label: "Undo" }, { id: "later", label: "Later" }]
    }), handlers({ notify }));
    expect(response.ok).toBe(true);
    expect(notify).toHaveBeenCalledWith({
      title: "Saved",
      message: "Your changes were written.",
      description: "All files are safe.",
      variant: "success",
      image: "https://example.test/photo.png",
      meta: "now",
      progress: 0.5,
      duration: 5000,
      buttons: [{ id: "undo", label: "Undo" }, { id: "later", label: "Later" }]
    });

    for (const params of [
      { message: "x", image: "javascript:alert(1)" },
      { message: "x", progress: 2 },
      { message: "x", buttons: [{ id: "a", label: "A" }, { id: "b", label: "B" }, { id: "c", label: "C" }] },
      { message: "x", buttons: [{ id: "../escape", label: "A" }] }
    ]) {
      expect(handleBridgeRequest(request("11", "os.notification", params), handlers({ notify })))
        .toMatchObject({ ok: false, error: { code: "invalid-params" } });
    }
  });

  test("emergency notifications require the dedicated capability", () => {
    const notify = vi.fn();
    expect(handleBridgeRequest(request("12", "os.notification", { message: "Evacuate", emergency: true }), handlers({ notify })))
      .toMatchObject({ ok: false, error: { code: "denied" } });
    const caps = [...BRIDGE_CAPABILITY_LIST, BRIDGE_CAPABILITIES.emergency];
    expect(handleBridgeRequest(request("13", "os.notification", { message: "Evacuate", emergency: true }), handlers({ notify, capabilities: caps })).ok).toBe(true);
    expect(notify).toHaveBeenCalledWith({ message: "Evacuate", emergency: true });
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
  test("pets.state and pets.interact require the pet capability", () => {
    const petState = vi.fn(() => ({ id: "legacy-pet", name: "Momo", quiet: false, render: "2d" as const, temperament: "calm" as const, mood: "cozy" as const, vitality: 80, hunger: 20, bond: 70, sleeping: false }));
    const petInteract = vi.fn((action: string) => action === "dance");
    const pets = [...BRIDGE_CAPABILITY_LIST, BRIDGE_CAPABILITIES.pets];
    expect(handleBridgeRequest(request("p1", "pets.state"), handlers({ capabilities: pets, petState }))).toMatchObject({
      ok: true, result: { active: { name: "Momo" } }
    });
    expect(handleBridgeRequest(request("p2", "pets.interact", { action: "dance" }), handlers({ capabilities: pets, petInteract })).ok).toBe(true);
    expect(petInteract).toHaveBeenCalledWith("dance");
    expect(handleBridgeRequest(request("p3", "pets.interact", { action: "launch-missiles" }), handlers({ capabilities: pets, petInteract })))
      .toMatchObject({ ok: false, error: { code: "invalid-params" } });
    expect(handleBridgeRequest(request("p4", "pets.state"), handlers({ capabilities: [] })))
      .toMatchObject({ ok: false, error: { code: "denied" } });
    expect(handleBridgeRequest(request("p5", "pets.interact", { action: "dance" }), handlers({ capabilities: [BRIDGE_CAPABILITIES.info], petInteract })))
      .toMatchObject({ ok: false, error: { code: "denied" } });
    expect(handleBridgeRequest(request("p5b", "pets.state"), handlers({ capabilities: BRIDGE_CAPABILITY_LIST })))
      .toMatchObject({ ok: false, error: { code: "denied" } });
  });

  test("registers and unregisters bounded pet menu actions", () => {
    const unregister = vi.fn();
    const petRegisterAction = vi.fn(() => unregister);
    const petUnregisterAction = vi.fn(() => true);
    const registered = handleBridgeRequest(
      request("p6", "pets.action.register", { id: "com.example.radio.dance", label: "  Radio dance  ", icon: "music" }),
      handlers({ capabilities: [...BRIDGE_CAPABILITY_LIST, BRIDGE_CAPABILITIES.pets], petRegisterAction, petUnregisterAction })
    );
    expect(registered).toMatchObject({ ok: true, result: { registered: true, id: "com.example.radio.dance" } });
    expect(petRegisterAction).toHaveBeenCalledWith({ id: "com.example.radio.dance", label: "Radio dance", icon: "music" });
    expect(handleBridgeRequest(request("p7", "pets.action.unregister", { id: "com.example.radio.dance" }), handlers({ capabilities: [...BRIDGE_CAPABILITY_LIST, BRIDGE_CAPABILITIES.pets], petUnregisterAction })).ok).toBe(true);
    expect(petUnregisterAction).toHaveBeenCalledWith("com.example.radio.dance");

    for (const params of [
      { id: "not-namespaced", label: "Nope" },
      { id: "com.example.radio.dance", label: "" },
      { id: "com.example.radio.dance", label: "Nope", icon: "url(javascript:alert(1))" }
    ]) {
      expect(handleBridgeRequest(request("p8", "pets.action.register", params), handlers({ capabilities: [...BRIDGE_CAPABILITY_LIST, BRIDGE_CAPABILITIES.pets], petRegisterAction })))
        .toMatchObject({ ok: false, error: { code: "invalid-params" } });
    }
  });


  test("registers, publishes and clears OS weather warnings", async () => {
    const warningRegister = vi.fn(async () => ({ source: { id: "com.example.notes", name: "Notes", registeredAt: 1 } }));
    const warningPublish = vi.fn(async (warnings) => ({ accepted: warnings.length }));
    const warningClear = vi.fn(async () => ({ cleared: 1 }));
    const warningCapabilities = [...BRIDGE_CAPABILITY_LIST, BRIDGE_CAPABILITIES.warnings];
    const expires = Date.now() + 60_000;

    const registered = await handleBridgeRequestAsync(
      request("w1", "os.warnings.register"),
      handlers({ capabilities: warningCapabilities, warningRegister })
    );
    expect(registered).toMatchObject({ ok: true, result: { source: { id: "com.example.notes" } } });

    const warning = {
      id: "storm-1",
      level: "warning",
      headline: "Severe storm",
      description: "Gusts up to 120 km/h.",
      region: "Berlin",
      expires
    };
    const published = await handleBridgeRequestAsync(
      request("w2", "os.warnings.publish", { warnings: [warning] }),
      handlers({ capabilities: warningCapabilities, warningPublish })
    );
    expect(published).toMatchObject({ ok: true, result: { accepted: 1 } });
    expect(warningPublish).toHaveBeenCalledWith([warning]);

    const testPublished = await handleBridgeRequestAsync(
      request("w3", "os.warnings.test", { warnings: [{ ...warning, id: "test-1" }] }),
      handlers({ capabilities: warningCapabilities, warningPublish })
    );
    expect(testPublished.ok).toBe(true);
    expect(warningPublish).toHaveBeenLastCalledWith([{ ...warning, id: "test-1", test: true }]);

    const cleared = await handleBridgeRequestAsync(
      request("w4", "os.warnings.clear", { ids: ["storm-1"] }),
      handlers({ capabilities: warningCapabilities, warningClear })
    );
    expect(cleared).toMatchObject({ ok: true, result: { cleared: 1 } });
    expect(warningClear).toHaveBeenCalledWith(["storm-1"]);

    expect(await handleBridgeRequestAsync(request("w5", "os.warnings.register"), handlers({ warningRegister })))
      .toMatchObject({ ok: false, error: { code: "denied" } });
    expect(await handleBridgeRequestAsync(request("w6", "os.warnings.publish", { warnings: [warning] }), handlers({ warningPublish })))
      .toMatchObject({ ok: false, error: { code: "denied" } });
    for (const params of [
      { warnings: [] },
      { warnings: Array.from({ length: 9 }, (_, index) => ({ ...warning, id: `w-${index}` })) },
      { warnings: [{ ...warning, id: "../escape" }] },
      { warnings: [{ ...warning, level: "catastrophe" }] },
      { warnings: [{ ...warning, secret: "x" }] },
      { warnings: [{ ...warning, expires: 0 }] }
    ]) {
      expect(await handleBridgeRequestAsync(request("w7", "os.warnings.publish", params), handlers({ capabilities: warningCapabilities, warningPublish })))
        .toMatchObject({ ok: false, error: { code: "invalid-params" } });
    }
  });

  test("weather warnings do not use the app emergency boundary", async () => {
    const warningPublish = vi.fn(async (warnings) => ({ accepted: warnings.length }));
    const warning = {
      id: "evacuate",
      level: "emergency",
      headline: "Evacuate now",
      expires: Date.now() + 60_000
    };
    const warningCapabilities = [...BRIDGE_CAPABILITY_LIST, BRIDGE_CAPABILITIES.warnings];
    expect(await handleBridgeRequestAsync(
      request("w8", "os.warnings.publish", { warnings: [warning] }),
      handlers({ capabilities: warningCapabilities, warningPublish })
    )).toMatchObject({ ok: true, result: { accepted: 1 } });
    expect(warningPublish).toHaveBeenCalledWith([warning]);
  });

  test("invokes a capability on the user's behalf", async () => {
    const invokeCapability = vi.fn(async (capability: string, resource?: { namespace: string; kind: string; key: string }) => ({ capability, outcome: "invoked" as const, resource }));
    const response = await handleBridgeRequestAsync(
      request("10", "os.capabilities.invoke", { capability: "rumahl.files.preview", resource: { namespace: "rumahl.files", kind: "file", key: "a" } }),
      handlers({ invokeCapability })
    );
    expect(response).toMatchObject({ ok: true, result: { capability: "rumahl.files.preview", outcome: "invoked" } });
    expect(invokeCapability).toHaveBeenCalledWith("rumahl.files.preview", { namespace: "rumahl.files", kind: "file", key: "a" });
  });
  test("capability invocation fails closed", async () => {
    expect(await handleBridgeRequestAsync(request("11", "os.capabilities.invoke", { capability: "rumahl.files.preview" }), handlers({ capabilities: [] })))
      .toMatchObject({ ok: false, error: { code: "denied" } });
    expect(await handleBridgeRequestAsync(request("12", "os.capabilities.invoke", {}), handlers()))
      .toMatchObject({ ok: false, error: { code: "invalid-params" } });
    const invokeCapability = vi.fn(async () => { throw new Error("boom"); });
    expect(await handleBridgeRequestAsync(request("13", "os.capabilities.invoke", { capability: "rumahl.files.preview" }), handlers({ invokeCapability })))
      .toMatchObject({ ok: false, error: { code: "failed" } });
  });
});
