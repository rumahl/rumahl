import { describe, expect, it } from "vitest";
import { parseAppSettings, parseCatalog, parseLaunch } from "./client";
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
});
