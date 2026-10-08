import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  advanced: false,
  manifest: [
    { key: "server.url", title: "Server URL", description: "Backend endpoint", type: "text", required: true, options: [] },
  ],
}));

vi.mock("./AppCatalog", () => ({
  useAppCatalog: () => ({
    status: "ready",
    apps: [{ id: "com.rumahl.notes", installationId: "01990000-0000-7000-8000-000000000001", title: "Notes", version: "1.0.0", launchable: true }],
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

test("shows the extended configuration section once in advanced mode", async () => {
  state.advanced = true;
  renderPage();
  expect(await screen.findByText("appSettings.extendedTitle")).toBeInTheDocument();
});
