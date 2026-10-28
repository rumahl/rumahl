import type { InstalledApp } from "../apps/client";
import type { OsModeSettings, Preferences } from "../preferences/client";
import type { Workspace } from "../preferences/Workspace";
import { defaultLook } from "../preferences/appearance-presets";

/** Stable demo account id (UUID-shaped, never used for real authentication). */
export const DEMO_OWNER_ID = "01990000-0000-7000-8000-000000000001";
const STORAGE_KEY = "rumahl.demo.state.v10";

export type DemoShellMode = "desktop" | "launcher" | "mobile";
export type DemoOsMode = "guided" | "advanced" | "developer";

export interface DemoFileContent {
  kind: "text" | "base64";
  value: string;
}

export interface DemoFile {
  id: string;
  parent: string;
  name: string;
  directory: boolean;
  size: number;
  contentType?: string;
  content?: DemoFileContent;
}

export interface DemoState {
  version: 1;
  preferences: Pick<Preferences, "revision" | "user" | "device">;
  workspace: { revision: number; user: Workspace | null; device: Workspace | null };
  osMode: Pick<OsModeSettings, "revision" | "user" | "device">;
  files: DemoFile[];
  nextFileId: number;
}

/** Sample apps shown by the demo. Their frames are served by demo-widget.rumahl.com. */
export const demoApps: readonly InstalledApp[] = [
  { id: "com.rumahl.demo.notes", installationId: "01990000-0000-7000-8000-000000000101", title: "Notes", version: "1.4.0", launchable: true, runtime: "web", lifecycle: "on-demand" },
  { id: "com.rumahl.demo.weather", installationId: "01990000-0000-7000-8000-000000000102", title: "Weather", version: "2.1.0", launchable: true, runtime: "web", lifecycle: "on-demand" },
  { id: "com.rumahl.demo.photos", installationId: "01990000-0000-7000-8000-000000000103", title: "Photos", version: "1.2.0", launchable: true, runtime: "web", lifecycle: "on-demand" },
  { id: "com.rumahl.demo.music", installationId: "01990000-0000-7000-8000-000000000104", title: "Music", version: "3.0.1", launchable: true, runtime: "web", lifecycle: "on-demand" },
  { id: "com.rumahl.demo.home", installationId: "01990000-0000-7000-8000-000000000105", title: "Home", version: "1.0.0", launchable: true, runtime: "web", lifecycle: "on-demand" }
];

const welcome = `# Welcome to rumahl OS

This is a fully interactive demo. Nothing leaves your browser:

- rearrange desktop icons and apps
- create folders, files and notes
- switch themes and shell modes
- open the sample apps and widgets

The demo resets when you clear its site data.
`;

const notes = `# Demo notes

- [x] Try the launcher
- [x] Open the file manager
- [ ] Change the accent colour
- [ ] Pin a widget to the desktop
`;

const architecture = `rumahl OS runs local services behind a small shell.
The demo replaces the HTTP gateway with an in-browser fixture backend,
so the same React runtime and contracts are exercised without a server.
`;

const releaseNotes = `# Release notes

## Demo build
- sample apps for notes, weather, photos, music and home
- persistent preferences, workspace and files
- interactive home widgets
`;

const shopping = `Milk
Bread
Coffee
Fresh herbs
`;

const script = `#!/bin/sh
# Demo backup script
set -eu
echo "Snapshotting demo data..."
`;

function textFile(id: string, parent: string, name: string, value: string, contentType = "text/plain"): DemoFile {
  return { id, parent, name, directory: false, size: new TextEncoder().encode(value).length, contentType, content: { kind: "text", value } };
}

function directory(id: string, parent: string, name: string): DemoFile {
  return { id, parent, name, directory: true, size: 0 };
}

function seedWorkspace(): Workspace {
  return {
    version: 1,
    windows: [],
    folders: [
      { id: "work", name: "Work", apps: ["com.rumahl.demo.notes"] },
      { id: "media", name: "Media", apps: ["com.rumahl.demo.photos", "com.rumahl.demo.music"] }
    ],
    desktop: { order: [], hidden: [], widgets: true },
    launcherView: null,
    appearance: { ...defaultLook, tokens: { ...defaultLook.tokens } }
  };
}

function seedFiles(): DemoFile[] {
  return [
    directory("desktop", "root", "Desktop"),
    directory("documents", "root", "Documents"),
    directory("downloads", "root", "Downloads"),
    directory("pictures", "root", "Pictures"),
    directory("music", "root", "Music"),
    textFile("welcome", "root", "Welcome.md", welcome, "text/markdown"),
    textFile("desktop-welcome", "desktop", "Getting started.txt", "Double-click an icon, drag it around, or open the Files app to explore.\n"),
    textFile("desktop-notes", "desktop", "Quick notes.md", notes, "text/markdown"),
    textFile("document-notes", "documents", "Demo notes.md", notes, "text/markdown"),
    textFile("document-architecture", "documents", "Architecture.txt", architecture),
    textFile("document-shopping", "documents", "Shopping list.txt", shopping),
    textFile("document-script", "documents", "backup.sh", script, "text/x-shellscript"),
    directory("projects", "documents", "Projects"),
    textFile("project-readme", "projects", "README.md", "# Projects\n\nSample files for the demo.\n", "text/markdown"),
    textFile("download-release", "downloads", "release-notes.md", releaseNotes, "text/markdown"),
    textFile("picture-aurora", "pictures", "aurora.svg", `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 400"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#072a2f"/><stop offset="1" stop-color="#28694c"/></linearGradient></defs><rect width="640" height="400" fill="url(#g)"/><path d="M0 260 Q160 120 320 230 T640 180" fill="none" stroke="#7ef0c4" stroke-width="22" opacity=".55"/><path d="M0 320 Q200 210 380 300 T640 250" fill="none" stroke="#b7ffe9" stroke-width="10" opacity=".4"/></svg>`, "image/svg+xml"),
    textFile("picture-forest", "pictures", "forest.svg", `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 400"><rect width="640" height="400" fill="#dcefe0"/><path d="M0 330 L120 180 L240 330 Z" fill="#28694c"/><path d="M170 330 L320 130 L470 330 Z" fill="#1f5840"/><path d="M380 330 L520 200 L640 330 Z" fill="#3f8a68"/><rect y="330" width="640" height="70" fill="#9ac7a7"/></svg>`, "image/svg+xml"),
    textFile("music-readme", "music", "Playlist.txt", "1. Morning light\n2. Glass house\n3. Local first\n", "text/plain")
  ];
}

export function seedDemoState(): DemoState {
  return {
    version: 1,
    preferences: {
      revision: 1,
      user: { shellMode: "desktop", shellTheme: "com.rumahl.default" },
      device: { shellMode: null, shellTheme: null }
    },
    workspace: { revision: 1, user: null, device: seedWorkspace() },
    osMode: {
      revision: 1,
      user: { osMode: "advanced" },
      device: { osMode: null }
    },
    files: seedFiles(),
    nextFileId: 1
  };
}

export function loadDemoState(): DemoState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<DemoState>;
      if (parsed.version === 1 && parsed.preferences && parsed.workspace && parsed.osMode && Array.isArray(parsed.files) && Number.isSafeInteger(parsed.nextFileId))
        return parsed as DemoState;
    }
  } catch { /* private mode or corrupt state: start fresh */ }
  return seedDemoState();
}

export function saveDemoState(state: DemoState): void {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(state)); } catch { /* demo remains usable without durable state */ }
}
