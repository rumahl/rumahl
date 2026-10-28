import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  advanced: false,
  apps: [] as { id: string; title: string; path: string; launchable: boolean; system: boolean; kind: "app" }[],
  live: { request: vi.fn(async (url: string): Promise<{ ok: boolean; json: () => Promise<unknown> }> => { void url; return { ok: true, json: async () => [] }; }) },
  fetchHostList: vi.fn(async () => [] as { name: string; directory: boolean; size: number; modified: number }[]),
}));

vi.mock("../../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("../../shell/ShellContext", () => ({ useShell: () => ({ live: state.live, open: vi.fn() }) }));
vi.mock("../../preferences/OsMode", () => ({ useOsModePolicy: () => ({ advancedSettings: state.advanced, browseSystemFiles: state.advanced, terminal: false, ssh: false, webConsole: false }) }));
vi.mock("../../preferences/storage", () => ({ browserProfile: () => ({ id: "01990000-0000-7000-8000-000000000001", persistence: "memory" }) }));
vi.mock("../../preferences/Workspace", () => ({ useWorkspace: () => ({ effective: { folders: [] } }) }));
vi.mock("../useShellApps", () => ({ useShellApps: () => state.apps }));
vi.mock("../fsClient", () => ({ fetchHostList: state.fetchHostList, hostContentUrl: () => "/content" }));

import { FilesApp } from "./FilesApp";

beforeEach(() => {
  state.advanced = false;
  state.apps = [];
  state.fetchHostList.mockClear();
  state.live.request.mockReset();
  state.live.request.mockImplementation(async () => ({ ok: true, json: async () => [] }));
});

test("browses personal files and hides host roots below advanced mode", async () => {
  render(<FilesApp />);
  const sidebar = screen.getByRole("complementary");
  expect(within(sidebar).getByRole("button", { name: "files.root" })).toBeInTheDocument();
  expect(within(sidebar).queryByRole("button", { name: "files.applications" })).toBeNull();
  expect(await screen.findByText("files.empty")).toBeInTheDocument();
});

test("browses host roots in advanced mode", async () => {
  state.advanced = true;
  render(<FilesApp />);
  const sidebar = screen.getByRole("complementary");
  fireEvent.click(within(sidebar).getByRole("button", { name: "files.applications" }));
  await waitFor(() => expect(state.fetchHostList).toHaveBeenCalled());
  expect(await screen.findByText("files.empty")).toBeInTheDocument();
});

test("opens a context menu on empty space and creates an empty file", async () => {
  render(<FilesApp />);
  await screen.findByText("files.empty");
  const surface = document.querySelector(".files-surface") as HTMLElement;
  fireEvent.contextMenu(surface, { clientX: 12, clientY: 12 });
  const menu = await screen.findByRole("menu");
  expect(menu).toHaveStyle({ left: "12px", top: "12px" });
  fireEvent.click(within(menu).getByRole("menuitem", { name: "files.newFile" }));
  const input = (await screen.findByPlaceholderText("files.newFile")) as HTMLInputElement;
  fireEvent.change(input, { target: { value: "notes.txt" } });
  fireEvent.submit(input.closest("form")!);
  await waitFor(() => {
    const call = state.live.request.mock.calls.find(([url]) => String(url).includes("directory=false"));
    expect(call).toBeTruthy();
    expect(String(call?.[0])).toContain("name=notes.txt");
  });
});

test("does not download a file without a registered program", async () => {
  state.live.request.mockImplementation(async (url: string) => {
    if (String(url).startsWith("/api/v1/files?")) {
      return { ok: true, json: async () => [{ id: "file-1", name: "mystery.xyz", directory: false, size: 12 }] };
    }
    return { ok: true, json: async () => [] };
  });
  render(<FilesApp />);
  const file = await screen.findByText("mystery.xyz");
  fireEvent.doubleClick(file);
  expect(await screen.findByText("files.noAssociation")).toBeInTheDocument();
});

test("does not download a file whose program is not installed", async () => {
  state.live.request.mockImplementation(async (url: string) => {
    if (String(url).startsWith("/api/v1/files?")) {
      return { ok: true, json: async () => [{ id: "file-2", name: "notes.txt", directory: false, size: 12 }] };
    }
    return { ok: true, json: async () => [] };
  });
  render(<FilesApp />);
  const file = await screen.findByText("notes.txt");
  fireEvent.doubleClick(file);
  expect(await screen.findByText("files.programMissing")).toBeInTheDocument();
});

test("shows application packages with their app identity", async () => {
  state.advanced = true;
  state.apps = [{ id: "com.example.notes", title: "Notes", path: "/apps/com.example.notes", launchable: true, system: false, kind: "app" }];
  state.fetchHostList.mockResolvedValue([{ name: "com.example.notes", directory: true, size: 0, modified: 0 }]);
  render(<FilesApp />);
  const sidebar = screen.getByRole("complementary");
  fireEvent.click(within(sidebar).getByRole("button", { name: "files.applications" }));
  expect(await screen.findByText("Notes")).toBeInTheDocument();
  expect(document.querySelector('.files-tile[data-kind="app"]')).toBeInTheDocument();
});
