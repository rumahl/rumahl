import { beforeEach, describe, expect, test } from "vitest";
import { parseLaunch } from "../apps/client";
import { DemoBackend } from "./backend";
import { demoApps } from "./state";

const widgetOrigin = "https://demo-widget.rumahl.com";
const json = (response: Response) => response.json() as Promise<Record<string, unknown>>;

beforeEach(() => localStorage.clear());

describe("demo backend", () => {
  test("serves a shell snapshot and the sample app catalog", async () => {
    const backend = new DemoBackend({ widgetOrigin });
    const snapshot = await json(await backend.request("/api/v1/shell/snapshot"));
    expect(snapshot.snapshotVersion).toBe(1);
    expect(snapshot.apps).toHaveLength(demoApps.length);
    const catalog = await json(await backend.request("/api/v1/shell/apps"));
    expect(catalog.catalogVersion).toBe(1);
    expect((catalog.apps as unknown[]).length).toBe(demoApps.length);
  });

  test("exposes widget sizes in the snapshot", async () => {
    const backend = new DemoBackend({ widgetOrigin });
    const snapshot = await json(await backend.request("/api/v1/shell/snapshot"));
    const widgets = snapshot.contributions as { id: string; size?: string }[];
    expect(widgets.find((widget) => widget.id === "com.example.home.dashboard")?.size).toBe("large");
    expect(widgets.find((widget) => widget.id === "com.example.weather.current")?.size).toBe("medium");
    expect(widgets.find((widget) => widget.id === "com.example.energy.today")?.size).toBe("small");
  });

  test("persists preferences and rejects stale revisions", async () => {
    const backend = new DemoBackend({ widgetOrigin });
    const initial = await json(await backend.request("/api/v1/shell/preferences?device=test"));
    expect(initial.effective).toMatchObject({ shellMode: "desktop" });
    const updated = await json(await backend.request("/api/v1/shell/preferences?device=test", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ settingsVersion: 1, revision: initial.revision, scope: "device", key: "shell.mode", value: "launcher" })
    }));
    expect(updated.effective).toMatchObject({ shellMode: "launcher" });
    const stale = await backend.request("/api/v1/shell/preferences?device=test", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ settingsVersion: 1, revision: initial.revision, scope: "device", key: "shell.mode", value: "desktop" })
    });
    expect(stale.status).toBe(409);
  });

  test("lists, creates and downloads demo files", async () => {
    const backend = new DemoBackend({ widgetOrigin });
    const root = await json(await backend.request("/api/v1/files?parent=root")) as unknown as { name: string; id: string }[];
    expect(root.map((entry) => entry.name)).toContain("Welcome.md");
    const created = await json(await backend.request("/api/v1/files?parent=root&name=notes.txt&directory=false", { method: "POST", body: "hello" }));
    expect(created).toMatchObject({ name: "notes.txt", size: 5 });
    const content = await backend.request(`/api/v1/files/content?id=${created.id as string}`);
    expect(await content.text()).toBe("hello");
    const renamed = await json(await backend.request(`/api/v1/files?id=${created.id as string}&name=renamed.txt`, { method: "PUT" }));
    expect(renamed).toMatchObject({ name: "renamed.txt" });
  });

  test("keeps created files and preferences in browser storage across reloads", async () => {
    const first = new DemoBackend({ widgetOrigin });
    await first.request("/api/v1/files?parent=root&name=persisted.txt&directory=false", { method: "POST", body: "cached" });
    const preferences = await json(await first.request("/api/v1/shell/preferences?device=test"));
    await first.request("/api/v1/shell/preferences?device=test", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ settingsVersion: 1, revision: preferences.revision, scope: "device", key: "shell.mode", value: "launcher" })
    });

    const second = new DemoBackend({ widgetOrigin });
    const root = await json(await second.request("/api/v1/files?parent=root")) as unknown as { name: string }[];
    expect(root.map((entry) => entry.name)).toContain("persisted.txt");
    const restored = await json(await second.request("/api/v1/shell/preferences?device=test"));
    expect(restored.effective).toMatchObject({ shellMode: "launcher" });
  });

  test("stores the workspace and accepts demo app launches on the widget origin", async () => {
    const backend = new DemoBackend({ widgetOrigin });
    const workspace = await json(await backend.request("/api/v1/shell/workspace?device=test"));
    expect(workspace.device).toMatchObject({ version: 1 });
    const launch = await json(await backend.request(`/api/v1/shell/apps/${encodeURIComponent(demoApps[0]!.id)}/launch`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ installationId: demoApps[0]!.installationId })
    }));
    const parsed = parseLaunch(launch, demoApps[0]!, "https://demo.rumahl.com", { frameOrigin: widgetOrigin });
    expect(parsed.frameUrl).toBe(`${widgetOrigin}/apps/${encodeURIComponent(demoApps[0]!.id)}/`);
    expect(() => parseLaunch(launch, demoApps[0]!, "https://demo.rumahl.com")).toThrow();
    const sameOrigin = "https://demo.rumahl.com";
    const sameOriginLaunch = { ...launch, frameUrl: `${sameOrigin}/apps/${encodeURIComponent(demoApps[0]!.id)}/` };
    expect(parseLaunch(sameOriginLaunch, demoApps[0]!, sameOrigin, { frameOrigin: sameOrigin }).frameUrl)
      .toBe(`${sameOrigin}/apps/${encodeURIComponent(demoApps[0]!.id)}/`);
    const widget = await json(await backend.request("/api/v1/shell/widgets/com.example.home.dashboard/frame"));
    expect(widget.frameUrl).toBe(`${widgetOrigin}/widgets/home/`);
  });
});
