import { describe, expect, it } from "vitest";
import type { ShellRequest } from "../snapshot-client";
import { controlAppRuntime, invokeCapability, parseAppData, parseAppSettings, parseCapabilityInvocation, parseCatalog, parseImport, parseLaunch } from "./client";
const app = { id: "com.rumahl.notes", installationId: "01990000-0000-7000-8000-000000000001", title: "Notes", version: "1.0.0", launchable: true };
const lease = "a".repeat(64);
const launch = { launchVersion: 1, id: app.id, installationId: app.installationId, lease, frameUrl: `https://${app.installationId}.apps.localhost:8443/launch/${lease}/web/index.html`, renewAfterSeconds: 30 };
const settings = {
  settingsVersion: 1,
  id: app.id,
  installationId: app.installationId,
  manifest: [
    { key: "server.url", title: "Server URL", description: "Backend endpoint", type: "text", required: true, options: [] },
    { key: "mode", title: "Mode", description: null, type: "select", required: false, options: [{ value: "fast", label: "Fast" }] },
  ],
  extended: null,
};
describe("installed app contracts", () => {
  it("projects only public metadata and rejects duplicate identities or incompatible schemas", () => {
    expect(parseCatalog({ catalogVersion: 1, apps: [{ ...app, internalPath: "/secret" }] })).toEqual([app]);
    for (const value of [{ catalogVersion: 2, apps: [] }, { catalogVersion: 1, apps: [app, app] }, { catalogVersion: 1, apps: [{ ...app, id: "../escape" }] }, { catalogVersion: 1, apps: [{ ...app, installationId: "legacy-install" }] }]) expect(() => parseCatalog(value)).toThrow();
  });
  it("accepts the platform's full 255-character app ID range", () => {
    const id = Array(4).fill("a".repeat(63)).join(".");
    expect(id.length).toBe(255);
    expect(parseCatalog({ catalogVersion: 1, apps: [{ ...app, id }] })[0]?.id).toBe(id);
    expect(() => parseCatalog({ catalogVersion: 1, apps: [{ ...app, id: `${id}a` }] })).toThrow();
  });
  it("binds a launch to the requested installation and a separate HTTPS app host", () => {
    expect(parseLaunch(launch, app, "https://localhost:8443").frameUrl).toBe(launch.frameUrl);
    for (const frameUrl of [launch.frameUrl.replace("https:", "http:"), launch.frameUrl.replace(`${app.installationId}.apps.localhost`, "localhost"), launch.frameUrl.replace(app.installationId, "01990000-0000-7000-8000-000000000002"), `${launch.frameUrl}?token=secret`, `${launch.frameUrl}#injected`, launch.frameUrl.replace(`/launch/${lease}/`, "/other/"), launch.frameUrl.replace("https://", "https://user:password@")]) expect(() => parseLaunch({ ...launch, frameUrl }, app, "https://localhost:8443")).toThrow();
    expect(() => parseLaunch({ ...launch, installationId: "other" }, app, "https://localhost:8443")).toThrow();
    expect(() => parseLaunch({ ...launch, renewAfterSeconds: 99999 }, app, "https://localhost:8443")).toThrow();
  });
  it("accepts declared capabilities and lifecycle, and rejects malformed ones", () => {
    const withMeta = { ...app, capabilities: ["com.rumahl.os.window", "com.rumahl.os.info"], lifecycle: "always-on", connectors: ["nextcloud"] };
    expect(parseCatalog({ catalogVersion: 1, apps: [withMeta] })[0]).toMatchObject({ capabilities: ["com.rumahl.os.window", "com.rumahl.os.info"], lifecycle: "always-on", connectors: ["nextcloud"] });
    expect(parseCatalog({ catalogVersion: 1, apps: [{ ...app }] })[0]?.capabilities).toBeUndefined();
    const cases = [
      { ...app, capabilities: "window" },
      { ...app, capabilities: [""] },
      { ...app, capabilities: ["x".repeat(300)] },
      { ...app, lifecycle: "sometimes" },
      { ...app, connectors: [1] },
      { ...app, connectors: ["x".repeat(65)] },
    ];
    for (const value of cases) expect(() => parseCatalog({ catalogVersion: 1, apps: [value] })).toThrow();
  });
  it("renders only typed manifest settings and rejects malformed declarations", () => {
    const parsed = parseAppSettings(settings);
    expect(parsed.manifest).toHaveLength(2);
    expect(parsed.manifest[0]).toMatchObject({ key: "server.url", type: "text", required: true, options: [] });
    expect(parsed.manifest[1]).toMatchObject({ type: "select", options: [{ value: "fast", label: "Fast" }] });
    expect(parsed.extended).toBeNull();

    const cases = [
      { ...settings, settingsVersion: 2 },
      { ...settings, extended: [{ key: "x", value: "y" }] },
      { ...settings, manifest: [{ ...settings.manifest[0], type: "date" }] },
      { ...settings, manifest: [{ ...settings.manifest[0], key: "Bad Key" }] },
      { ...settings, manifest: [{ ...settings.manifest[0], required: "yes" }] },
      { ...settings, manifest: [settings.manifest[0], settings.manifest[0]] },
      { ...settings, manifest: [{ ...settings.manifest[0], type: "select", options: [] }] },
      { ...settings, manifest: [{ ...settings.manifest[0], options: [{ value: "a", label: "A" }] }] },
    ];
    for (const value of cases) expect(() => parseAppSettings(value)).toThrow();
  });
  it("posts on-demand runtime start and stop", async () => {
    const calls: { url: string; body: unknown }[] = [];
    const request = (async (url: string, init?: RequestInit) => {
      calls.push({ url, body: init?.body ? JSON.parse(String(init.body)) : null });
      return new Response(JSON.stringify({ runtimeVersion: 1, id: app.id, installationId: app.installationId, state: "running" }), { status: 200, headers: { "content-type": "application/json" } });
    }) as unknown as ShellRequest;
    const signal = new AbortController().signal;
    await controlAppRuntime(request, app, "start", signal);
    await controlAppRuntime(request, app, "stop", signal);
    expect(calls.map((call) => call.url)).toEqual([
      `/api/v1/shell/apps/${app.id}/runtime`,
      `/api/v1/shell/apps/${app.id}/runtime`,
    ]);
    expect(calls[0]?.body).toMatchObject({ installationId: app.installationId, action: "start" });
    expect(calls[1]?.body).toMatchObject({ installationId: app.installationId, action: "stop" });
  });
  it("posts a capability invocation and parses the outcome", async () => {
    const calls: { url: string; body: unknown }[] = [];
    const request = (async (url: string, init?: RequestInit) => {
      calls.push({ url, body: init?.body ? JSON.parse(String(init.body)) : null });
      return new Response(JSON.stringify({ capabilityVersion: 1, outcome: "invoked", result: { answer: 42 } }), { status: 200, headers: { "content-type": "application/json" } });
    }) as unknown as ShellRequest;
    const signal = new AbortController().signal;
    const result = await invokeCapability(request, "rumahl.files.preview", { namespace: "rumahl.files", kind: "file", key: "a" }, signal);
    expect(result).toEqual({ capability: "rumahl.files.preview", outcome: "invoked", result: { answer: 42 } });
    expect(calls[0]?.url).toBe("/api/v1/shell/capabilities/invoke");
    expect(calls[0]?.body).toMatchObject({ capability: "rumahl.files.preview", resource: { namespace: "rumahl.files", kind: "file", key: "a" } });
    expect(() => parseCapabilityInvocation({ capabilityVersion: 1, outcome: "maybe" }, "x")).toThrow();
    expect(parseCapabilityInvocation({ capabilityVersion: 1, outcome: "invoked", browser: { appId: app.id, installationId: app.installationId } }, "cap"))
      .toEqual({ capability: "cap", outcome: "invoked", browser: { appId: app.id, installationId: app.installationId } });
    expect(() => parseCapabilityInvocation({ capabilityVersion: 1, outcome: "invoked", browser: { appId: "../escape", installationId: app.installationId } }, "cap")).toThrow();
    expect(() => parseCapabilityInvocation({ capabilityVersion: 1, outcome: "invoked", browser: { appId: app.id } }, "cap")).toThrow();
  });
  it("parses a successful import and rejects malformed responses", () => {
    const imported = { importVersion: 1, id: app.id, installationId: app.installationId, title: "Notes", version: "1.0.0" };
    expect(parseImport(imported)).toMatchObject({ id: app.id, installationId: app.installationId });
    for (const value of [{ ...imported, importVersion: 2 }, { ...imported, id: "not an id" }, { ...imported, installationId: "legacy" }, { ...imported, title: "" }, { ...imported, version: "x".repeat(200) }]) expect(() => parseImport(value)).toThrow();
  });
  it("validates app data listings and file previews", () => {    const directory = { dataVersion: 1, kind: "directory", path: "docs", entries: [{ name: "readme.md", directory: false, size: 8 }, { name: "sub", directory: true, size: 0 }] };
    expect(parseAppData(directory)).toEqual({ kind: "directory", path: "docs", entries: directory.entries });
    const file = { dataVersion: 1, kind: "file", path: "readme.md", size: 8, contentType: "text/plain; charset=utf-8", text: "# readme" };
    expect(parseAppData(file)).toEqual({ kind: "file", path: "readme.md", size: 8, contentType: "text/plain; charset=utf-8", text: "# readme" });
    expect(parseAppData({ ...file, text: null }).kind).toBe("file");

    const cases = [
      { ...directory, dataVersion: 2 },
      { ...directory, kind: "socket" },
      { ...directory, path: "../etc" },
      { ...directory, entries: [{ name: "../escape", directory: false, size: 1 }] },
      { ...directory, entries: [{ name: "x", directory: false, size: -1 }] },
      { ...file, size: 99 * 1024 * 1024 },
      { ...file, text: 42 },
    ];
    for (const value of cases) expect(() => parseAppData(value)).toThrow();
  });
});
