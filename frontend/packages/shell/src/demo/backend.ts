import type { ShellSnapshotV1 } from "@rumahl/contracts";
import type { ExtensionContribution } from "@rumahl/contracts";
import { parseShellSnapshot } from "@rumahl/contracts/parse";
import { SHELL_EVENT_VERSION } from "@rumahl/contracts/events";
import { SHELL_EVENTS_ENDPOINT, type EventSocket, type OpenEvents } from "../live-updates";
import { SHELL_SNAPSHOT_ENDPOINT, type ShellRequest } from "../snapshot-client";
import { parseWorkspace } from "../preferences/Workspace";
import type { Preferences, OsModeSettings } from "../preferences/client";
import { demoSnapshot } from "./snapshot";
import { DEMO_OWNER_ID, demoApps, loadDemoState, saveDemoState, type DemoFile, type DemoFileContent, type DemoState } from "./state";

const DEFAULT_THEME = "com.rumahl.default";
const DEFAULT_WIDGET_ORIGIN = "https://demo-widget.rumahl.com";
const LEASE = "0".repeat(64);
const WIDGETS: Readonly<Record<string, string>> = {
  "com.example.home.dashboard": "home",
  "com.example.weather.current": "weather",
  "com.example.energy.today": "energy"
};

const WIDGET_CONTRIBUTIONS: readonly ExtensionContribution[] = [
  { kind: "widget", id: "com.example.home.dashboard", title: "Home", slot: "dashboard_widgets", entrypoint: "dashboard", size: "large" },
  { kind: "widget", id: "com.example.weather.current", title: "Weather", slot: "dashboard_widgets", entrypoint: "weather", size: "medium" },
  { kind: "widget", id: "com.example.energy.today", title: "Energy", slot: "dashboard_widgets", entrypoint: "energy", size: "small" }
];

function json(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" }
  });
}

function noContent(): Response {
  return new Response(null, { status: 204, headers: { "Cache-Control": "no-store" } });
}

function origin(value: string): string {
  return value.replace(/\/+$/, "");
}

function contentBytes(file: DemoFile): Uint8Array<ArrayBuffer> | null {
  if (!file.content) return null;
  if (file.content.kind === "text") return new TextEncoder().encode(file.content.value);
  try {
    const binary = atob(file.content.value);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
    return bytes;
  } catch {
    return null;
  }
}

function contentType(name: string): string {
  const extension = name.split(".").pop()?.toLowerCase() ?? "";
  return ({
    md: "text/markdown; charset=utf-8",
    txt: "text/plain; charset=utf-8",
    json: "application/json; charset=utf-8",
    sh: "text/x-shellscript; charset=utf-8",
    svg: "image/svg+xml",
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    webp: "image/webp",
    gif: "image/gif"
  } as Record<string, string>)[extension] ?? "application/octet-stream";
}

function validName(name: string): boolean {
  return name.length > 0 && name.length <= 255 && name !== "." && name !== ".." && !/[/\\\0]/.test(name);
}

export class DemoBackend {
  private snapshot: ShellSnapshotV1;
  private state: DemoState;
  private readonly sockets = new Set<DemoEventSocket>();
  private readonly widgetOrigin: string;

  constructor(options: { widgetOrigin?: string } = {}) {
    this.widgetOrigin = origin(options.widgetOrigin ?? DEFAULT_WIDGET_ORIGIN);
    this.state = loadDemoState();
    this.snapshot = parseShellSnapshot({
      ...demoSnapshot,
      revision: "demo-only-revision-001",
      apps: demoApps.map(({ id, title, launchable }) => ({ id, title, launchable })),
      systemStatus: { ...demoSnapshot.systemStatus, installedAppCount: demoApps.length + 3 },
      contributions: WIDGET_CONTRIBUTIONS
    });
  }

  readonly request: ShellRequest = async (input, init) => {
    const url = new URL(typeof input === "string" ? input : input.toString(), location.href);
    const method = init?.method ?? "GET";
    const path = url.pathname;

    if (path === SHELL_SNAPSHOT_ENDPOINT && method === "GET") return json(this.snapshot);
    if (path === "/api/v1/shell/apps" && method === "GET") return json({ catalogVersion: 1, apps: demoApps });
    if (path === "/api/v1/shell/streams" && method === "GET") return json({ sessions: [] });
    if (path === "/api/v1/shell/report" && method === "POST") return noContent();
    if (path === "/api/v1/shell/preferences") return this.preferences(init, method);
    if (path === "/api/v1/shell/os-mode") return this.osMode(init, method);
    if (path === "/api/v1/shell/workspace") return this.workspace(init, method);
    if (path === "/api/v1/files") return this.files(url, init, method);
    if (path === "/api/v1/files/content" && method === "GET") return this.fileContent(url);
    if (path === "/api/v1/shell/fs" && method === "GET") return this.hostFiles(url);
    if (path === "/api/v1/shell/fs/content" && method === "GET") return this.hostContent(url);
    if (path === "/api/v1/shell/capabilities/invoke" && method === "POST") {
      const body = await this.body(init);
      const capability = typeof body.capability === "string" ? body.capability : "";
      return json({ capabilityVersion: 1, capability, outcome: "denied" });
    }

    const widget = path.match(/^\/api\/v1\/shell\/widgets\/([^/]+)\/frame$/);
    if (widget?.[1] && method === "GET") {
      const contributionId = decodeURIComponent(widget[1]);
      const frame = WIDGETS[contributionId];
      if (!frame) return json({ error: "widget not found" }, 404);
      return json({ frameVersion: 1, contributionId, frameUrl: `${this.widgetOrigin}/widgets/${frame}/` });
    }

    const launch = path.match(/^\/api\/v1\/shell\/apps\/([^/]+)\/launch$/);
    if (launch?.[1] && method === "POST") {
      const app = demoApps.find((item) => item.id === decodeURIComponent(launch[1]!));
      if (!app || !app.launchable) return json({ error: "app not found" }, 404);
      const body = await this.body(init);
      if (typeof body.installationId === "string" && body.installationId !== app.installationId)
        return json({ error: "installation mismatch" }, 409);
      return json({
        launchVersion: 1,
        id: app.id,
        installationId: app.installationId,
        lease: LEASE,
        frameUrl: `${this.widgetOrigin}/apps/${encodeURIComponent(app.id)}/`,
        renewAfterSeconds: 30
      });
    }

    const settings = path.match(/^\/api\/v1\/shell\/apps\/([^/]+)\/settings$/);
    if (settings?.[1] && method === "GET") {
      const app = demoApps.find((item) => item.id === decodeURIComponent(settings[1]!));
      if (!app) return json({ error: "app not found" }, 404);
      return json({
        id: app.id,
        installationId: app.installationId,
        manifest: [
          { key: "notifications", title: "Notifications", description: "Show notifications from this app.", type: "boolean", required: false, options: [] },
          { key: "startOnLogin", title: "Start on sign-in", type: "boolean", required: false, options: [] },
          { key: "appearance", title: "Appearance", type: "select", required: true, options: [
            { value: "system", label: "Match the OS" },
            { value: "light", label: "Light" },
            { value: "dark", label: "Dark" }
          ] }
        ],
        extended: null
      });
    }

    const data = path.match(/^\/api\/v1\/shell\/apps\/([^/]+)\/data$/);
    if (data?.[1] && method === "GET") {
      const app = demoApps.find((item) => item.id === decodeURIComponent(data[1]!));
      if (!app) return json({ error: "app not found" }, 404);
      const requested = url.searchParams.get("path") ?? "";
      if (!requested) return json({ dataVersion: 1, kind: "directory", path: "", entries: [
        { name: "config.json", directory: false, size: 48 },
        { name: "README.md", directory: false, size: 120 }
      ] });
      if (requested === "config.json") return json({ dataVersion: 1, kind: "file", path: "config.json", size: 48, contentType: "application/json", text: '{"theme":"system","notifications":true}' });
      if (requested === "README.md") return json({ dataVersion: 1, kind: "file", path: "README.md", size: 120, contentType: "text/markdown", text: `# ${app.title}\n\nThis private demo data directory belongs to ${app.title}.\n` });
      return json({ error: "not found" }, 404);
    }

    const runtime = path.match(/^\/api\/v1\/shell\/apps\/([^/]+)\/runtime$/);
    if (runtime?.[1] && method === "POST") return noContent();

    return json({ error: "demo route not found" }, 404);
  };

  readonly openEvents: OpenEvents = (url) => {
    if (new URL(url).pathname !== SHELL_EVENTS_ENDPOINT) throw new Error("demo event route not found");
    const socket = new DemoEventSocket(() => this.sockets.delete(socket));
    this.sockets.add(socket);
    queueMicrotask(() => socket.open());
    return socket;
  };

  publishSnapshot(value: unknown): void {
    this.snapshot = parseShellSnapshot(value);
    const event = JSON.stringify({ eventVersion: SHELL_EVENT_VERSION, kind: "snapshot_changed", revision: this.snapshot.revision });
    for (const socket of this.sockets) socket.send(event);
  }

  private async body(init: RequestInit | undefined): Promise<Record<string, unknown>> {
    try {
      const value = typeof init?.body === "string" ? JSON.parse(init.body) : null;
      return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
    } catch {
      return {};
    }
  }

  private async preferences(init: RequestInit | undefined, method: string): Promise<Response> {
    if (method === "GET") return json(this.preferencesValue());
    if (method !== "PUT") return json({ error: "method not allowed" }, 405);
    return this.body(init).then((body) => {
      const revision = body.revision;
      const scope = body.scope === "device" ? "device" : body.scope === "user" ? "user" : null;
      const key = body.key;
      if (revision !== this.state.preferences.revision || scope === null || (key !== "shell.mode" && key !== "shell.theme"))
        return json({ error: "conflict" }, 409);
      const value = body.value;
      if (key === "shell.mode") {
        if (value !== null && value !== "desktop" && value !== "launcher" && value !== "mobile") return json({ error: "invalid value" }, 400);
        if (scope === "user") this.state.preferences.user.shellMode = (value as "desktop" | "launcher" | "mobile" | null) ?? "desktop";
        else this.state.preferences.device.shellMode = value as "desktop" | "launcher" | "mobile" | null;
      } else {
        if (value !== null && (typeof value !== "string" || value.length > 128)) return json({ error: "invalid value" }, 400);
        if (scope === "user") this.state.preferences.user.shellTheme = (value as string | null) ?? DEFAULT_THEME;
        else this.state.preferences.device.shellTheme = value as string | null;
      }
      this.state.preferences.revision += 1;
      saveDemoState(this.state);
      return json(this.preferencesValue());
    });
  }

  private preferencesValue(): Preferences {
    const { revision, user, device } = this.state.preferences;
    return {
      settingsVersion: 1, ownerId: DEMO_OWNER_ID, revision, user, device,
      effective: { shellMode: device.shellMode ?? user.shellMode, shellTheme: device.shellTheme ?? user.shellTheme }
    };
  }

  private async osMode(init: RequestInit | undefined, method: string): Promise<Response> {
    if (method === "GET") return json(this.osModeValue());
    if (method !== "PUT") return json({ error: "method not allowed" }, 405);
    return this.body(init).then((body) => {
      const revision = body.revision;
      const scope = body.scope === "device" ? "device" : body.scope === "user" ? "user" : null;
      const mode = body.mode;
      if (revision !== this.state.osMode.revision || scope === null ||
          (mode !== null && mode !== "guided" && mode !== "advanced" && mode !== "developer"))
        return json({ error: "conflict" }, 409);
      if (scope === "user") this.state.osMode.user.osMode = mode === null ? "guided" : mode;
      else this.state.osMode.device.osMode = mode;
      this.state.osMode.revision += 1;
      saveDemoState(this.state);
      return json(this.osModeValue());
    });
  }

  private osModeValue(): OsModeSettings {
    const { revision, user, device } = this.state.osMode;
    return { settingsVersion: 1, ownerId: DEMO_OWNER_ID, revision, user, device, effective: { osMode: device.osMode ?? user.osMode } };
  }

  private async workspace(init: RequestInit | undefined, method: string): Promise<Response> {
    if (method === "GET") return json(this.workspaceValue());
    if (method !== "PUT") return json({ error: "method not allowed" }, 405);
    return this.body(init).then((body) => {
      if (body.revision !== this.state.workspace.revision) return json({ error: "conflict" }, 409);
      const scope = body.scope === "device" ? "device" : body.scope === "user" ? "user" : null;
      if (scope === null) return json({ error: "invalid scope" }, 400);
      let value = body.value;
      if (value !== null) {
        try { value = parseWorkspace(value); } catch { return json({ error: "invalid workspace" }, 400); }
      }
      this.state.workspace[scope] = value as DemoState["workspace"]["device"];
      this.state.workspace.revision += 1;
      saveDemoState(this.state);
      return json(this.workspaceValue());
    });
  }

  private workspaceValue() {
    return { ownerId: DEMO_OWNER_ID, revision: this.state.workspace.revision, user: this.state.workspace.user, device: this.state.workspace.device };
  }

  private files(url: URL, init: RequestInit | undefined, method: string): Response | Promise<Response> {
    if (method === "GET") {
      const parent = url.searchParams.get("parent") ?? "root";
      if (parent !== "root" && !this.state.files.some((file) => file.id === parent && file.directory)) return json({ error: "not found" }, 404);
      const entries = this.state.files.filter((file) => file.parent === parent)
        .sort((left, right) => Number(right.directory) - Number(left.directory) || left.name.localeCompare(right.name))
        .map(({ id, name, directory, size }) => ({ id, name, directory, size }));
      return json(entries);
    }
    if (method === "POST") {
      const parent = url.searchParams.get("parent") ?? "root";
      const name = url.searchParams.get("name") ?? "";
      const directory = url.searchParams.get("directory") === "true";
      if (!validName(name)) return json({ error: "invalid name" }, 400);
      if (this.state.files.some((file) => file.parent === parent && file.name === name)) return json({ error: "conflict" }, 409);
      return (async () => {
        const content = directory ? undefined : await this.uploadedContent(init);
        const size = content?.size ?? 0;
        const file: DemoFile = { id: `file-${this.state.nextFileId++}`, parent, name, directory, size, ...(content ? { contentType: contentType(name), content: content.value } : {}) };
        this.state.files.push(file);
        saveDemoState(this.state);
        return json({ id: file.id, name: file.name, directory: file.directory, size: file.size });
      })();
    }
    if (method === "PUT") {
      const id = url.searchParams.get("id");
      const file = this.state.files.find((entry) => entry.id === id);
      if (!file) return json({ error: "not found" }, 404);
      const name = url.searchParams.get("name") ?? file.name;
      const parent = url.searchParams.get("parent") ?? file.parent;
      if (!validName(name)) return json({ error: "invalid name" }, 400);
      if (this.state.files.some((entry) => entry.id !== id && entry.parent === parent && entry.name === name)) return json({ error: "conflict" }, 409);
      file.name = name;
      file.parent = parent;
      saveDemoState(this.state);
      return json({ id: file.id, name: file.name, directory: file.directory, size: file.size });
    }
    if (method === "DELETE") {
      const id = url.searchParams.get("id");
      if (!id || !this.state.files.some((entry) => entry.id === id)) return json({ error: "not found" }, 404);
      this.removeFile(id);
      saveDemoState(this.state);
      return noContent();
    }
    return json({ error: "method not allowed" }, 405);
  }

  private async uploadedContent(init: RequestInit | undefined): Promise<{ value: DemoFileContent; size: number } | undefined> {
    if (!init?.body) return undefined;
    try {
      const buffer = await new Response(init.body as BodyInit).arrayBuffer();
      const bytes = new Uint8Array(buffer);
      let binary = "";
      const chunk = 0x8000;
      for (let index = 0; index < bytes.length; index += chunk)
        binary += String.fromCharCode(...bytes.subarray(index, index + chunk));
      return { value: { kind: "base64", value: btoa(binary) }, size: bytes.length };
    } catch {
      return undefined;
    }
  }

  private removeFile(id: string): void {
    for (const child of this.state.files.filter((file) => file.parent === id)) this.removeFile(child.id);
    this.state.files = this.state.files.filter((file) => file.id !== id);
  }

  private fileContent(url: URL): Response {
    const file = this.state.files.find((entry) => entry.id === url.searchParams.get("id") && !entry.directory);
    if (!file) return json({ error: "not found" }, 404);
    const bytes = contentBytes(file);
    if (!bytes) return json({ error: "not found" }, 404);
    return new Response(bytes, {
      headers: {
        "Content-Type": file.contentType ?? contentType(file.name),
        "Content-Disposition": `attachment; filename="${file.name.replace(/["\\]/g, "_")}"`,
        "Cache-Control": "no-store"
      }
    });
  }

  private hostFiles(url: URL): Response {
    const area = url.searchParams.get("area") === "system" ? "system" : "apps";
    const requested = url.searchParams.get("path") ?? "";
    const entries = area === "apps"
      ? requested === "" ? demoApps.map((app) => ({ name: app.id.split(".").pop() ?? app.id, directory: true, size: 0, modified: 1_790_000_000 }))
        : [{ name: "manifest.json", directory: false, size: 128, modified: 1_790_000_000 }]
      : requested === "" ? [
          { name: "etc", directory: true, size: 0, modified: 1_790_000_000 },
          { name: "var", directory: true, size: 0, modified: 1_790_000_000 }
        ] : [{ name: "rumahl.conf", directory: false, size: 64, modified: 1_790_000_000 }];
    return json({ fsVersion: 1, area, path: requested, entries });
  }

  private hostContent(url: URL): Response {
    const path = url.searchParams.get("path") ?? "";
    const text = path.endsWith("manifest.json")
      ? '{"demo":true,"signed":false}'
      : path.endsWith("rumahl.conf") ? "mode=advanced\nsync=local\n" : "Demo host file\n";
    return new Response(text, { headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" } });
  }
}

class DemoEventSocket implements EventSocket {
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onopen: ((event: Event) => void) | null = null;
  private closed = false;

  constructor(private readonly remove: () => void) {}

  open(): void {
    if (!this.closed) this.onopen?.(new Event("open"));
  }

  send(data: string): void {
    if (!this.closed) this.onmessage?.(new MessageEvent("message", { data }));
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.remove();
    this.onclose?.(new CloseEvent("close"));
  }
}
