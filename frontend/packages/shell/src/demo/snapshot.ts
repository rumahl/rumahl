import type { ShellSnapshotV1 } from "@rumahl/contracts";

export const demoSnapshot = {
  snapshotVersion: 1,
  uiContractVersion: 1,
  extensionApiVersion: 1,
  shellBuildId: "shell-build-demo-only",
  revision: "demo-only-revision-001",
  user: {
    displayName: "Kaim",
    locale: "en-US"
  },
  theme: {
    stylesheetUrl:
      "/shell/themes/sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css",
    windowChrome: "standard"
  },
  systemStatus: {
    protection: "active",
    installedAppCount: 12,
    observedAtUnixMs: 1_790_106_120_000,
    lastActivityAtUnixMs: 1_790_105_880_000
  },
  contributions: [
    {
      kind: "command",
      id: "com.rumahl.apps.install",
      title: "Install app",
      capability: "com.rumahl.apps.install"
    },
    {
      kind: "search_provider",
      id: "com.example.notes.search",
      title: "Search notes",
      capability: "com.example.notes.search"
    },
    {
      kind: "widget",
      id: "com.example.home.dashboard",
      title: "Home",
      slot: "dashboard_widgets",
      entrypoint: "dashboard"
    }
  ]
} as const satisfies ShellSnapshotV1;
