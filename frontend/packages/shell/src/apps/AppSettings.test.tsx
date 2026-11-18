import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  advanced: false,
  runtime: "web" as "web" | "container",
  image: null as null | { runtime: "web" | "container"; image: null | string; entries: { name: string; value: string }[] },
  grants: {
    requests: [{ permission: "rumahl.files.read", scope: "user-selected", required: false, reason: "Open files you pick" }],
    grants: [{ id: "01990000-0000-7000-8000-0000000000aa", permission: "rumahl.files.read", scope: "explicit", resources: ["rumahl.files:file:1"], validity: "session", expiresAt: null }]
  },
  revoked: [] as string[],
  manifest: [
    { key: "server.url", title: "Server URL", description: "Backend endpoint", type: "text", required: true, options: [] },
  ],
}));

vi.mock("./AppCatalog", () => ({
  useAppCatalog: () => ({
    status: "ready",
    apps: [{ id: "com.rumahl.notes", installationId: "01990000-0000-7000-8000-000000000001", title: "Notes", version: "1.0.0", launchable: true, runtime: state.runtime }],
    preview: [],
  }),
}));
vi.mock("./client", () => ({
  fetchAppSettings: vi.fn(async () => ({
    id: "com.rumahl.notes",
    installationId: "01990000-0000-7000-8000-000000000001",
    manifest: state.manifest,
    extended: null,
  })),
  fetchAppData: vi.fn(async () => ({
    kind: "directory",
    path: "",
    entries: [{ name: "notes.db", directory: false, size: 12 }],
  })),
  fetchAppImageEnvironment: vi.fn(async () => state.image ?? {
    runtime: state.runtime,
    image: null,
    entries: [],
  }),
  fetchAppGrantState: vi.fn(async () => ({ ...state.grants, id: "com.rumahl.notes", installationId: "01990000-0000-7000-8000-000000000001" })),
  revokeAppGrant: vi.fn(async (_request: unknown, _app: unknown, grantId: string) => { state.revoked.push(grantId); }),
}));
vi.mock("../shell/ShellContext", () => ({ useShell: () => ({ live: { request: () => {} } }) }));
vi.mock("../preferences/OsMode", () => ({
  useOsModePolicy: () => ({ advancedSettings: state.advanced, browseSystemFiles: state.advanced, terminal: false, ssh: false, webConsole: false }),
}));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));

import { InstalledAppSettings } from "./AppSettings";

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/app/com.rumahl.notes/settings"]}>
      <Routes>
        <Route path="/app/:appId/*" element={<InstalledAppSettings />} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  state.advanced = false;
  state.runtime = "web";
  state.image = null;
  state.revoked = [];
});

test("renders the typed manifest settings with title, description and type in every mode", async () => {
  renderPage();
  expect(await screen.findByText("Server URL")).toBeInTheDocument();
  expect(screen.getByText("Backend endpoint")).toBeInTheDocument();
  expect(screen.getByText("appSettings.type.text")).toBeInTheDocument();
  expect(screen.getByText("appSettings.required")).toBeInTheDocument();
  // The extended section is hidden below advanced mode.
  expect(screen.queryByText("appSettings.extendedTitle")).toBeNull();
});

test("shows the extended configuration and app data sections once in advanced mode", async () => {
  state.advanced = true;
  renderPage();
  expect(await screen.findByText("appSettings.extendedTitle")).toBeInTheDocument();
  expect(await screen.findByText("notes.db · 12")).toBeInTheDocument();
  // Web apps have no container image section to render.
  expect(screen.queryByText("appSettings.imageTitle")).toBeNull();
});

test("a container app lists its recorded image environment in advanced mode", async () => {
  state.advanced = true;
  state.runtime = "container";
  state.image = {
    runtime: "container",
    image: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    entries: [{ name: "PATH", value: "/usr/bin:/bin" }],
  };
  renderPage();
  expect(await screen.findByText("appSettings.imageTitle")).toBeInTheDocument();
  expect(
    await screen.findByText("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
  ).toBeInTheDocument();
  expect(await screen.findByText("PATH")).toBeInTheDocument();
  expect(screen.getByText("/usr/bin:/bin")).toBeInTheDocument();
});

test("the permissions section lists requests and revokes issued grants", async () => {
  renderPage();
  expect(await screen.findByText("appPermissions.title")).toBeInTheDocument();
  // The permission shows up twice on purpose: once as requested, once granted.
  const mentions = await screen.findAllByText("rumahl.files.read", { exact: false });
  expect(mentions.length).toBe(2);
  expect(screen.getByText("Open files you pick")).toBeInTheDocument();
  const revoke = screen.getByRole("button", { name: "appPermissions.revoke" });
  fireEvent.click(revoke);
  await waitFor(() => expect(state.revoked).toEqual(["01990000-0000-7000-8000-0000000000aa"]));
});

test("apps without grants say so instead of guessing", async () => {
  const grants = state.grants;
  state.grants = { requests: [], grants: [] };
  const view = renderPage();
  expect(await screen.findByText("appPermissions.requestedNone")).toBeInTheDocument();
  expect(screen.getByText("appPermissions.grantsNone")).toBeInTheDocument();
  view.unmount();
  state.grants = grants;
});
