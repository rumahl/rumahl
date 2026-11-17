import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterAll, beforeAll, beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  advanced: false,
  apps: [] as { id: string; title: string; path: string; launchable: boolean; system: boolean; kind: "app" }[],
  topbarHost: null as HTMLElement | null,
  live: { request: vi.fn(async (url: string, init?: RequestInit): Promise<{ ok: boolean; headers?: Headers; json: () => Promise<unknown> }> => { void url; void init; return { ok: true, headers: new Headers(), json: async () => [] }; }) },
  open: vi.fn(),
  fetchHostList: vi.fn(async () => [] as { name: string; directory: boolean; size: number; modified: number }[]),
}));

vi.mock("../../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("../../shell/ShellContext", () => ({ useShell: () => ({ live: state.live, open: state.open }) }));
vi.mock("../../preferences/OsMode", () => ({ useOsModePolicy: () => ({ advancedSettings: state.advanced, browseSystemFiles: state.advanced, terminal: false, ssh: false, webConsole: false }) }));
vi.mock("../../preferences/storage", () => ({ browserProfile: () => ({ id: "01990000-0000-7000-8000-000000000001", persistence: "memory" }) }));
vi.mock("../../preferences/Workspace", () => ({ useWorkspace: () => ({ effective: { folders: [] } }) }));
vi.mock("../useShellApps", () => ({ useShellApps: () => state.apps }));
vi.mock("../fsClient", () => ({ fetchHostList: state.fetchHostList, hostContentUrl: () => "/content" }));
vi.mock("../../components/ProtectedWindow", () => ({ useAppTopbarHost: () => state.topbarHost }));

import { FilesApp } from "./FilesApp";

beforeAll(() => {
  if (typeof HTMLDialogElement.prototype.showModal !== "function") {
    Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
      configurable: true,
      value(this: HTMLDialogElement) { this.open = true; }
    });
  }
  if (typeof HTMLDialogElement.prototype.close !== "function") {
    Object.defineProperty(HTMLDialogElement.prototype, "close", {
      configurable: true,
      value(this: HTMLDialogElement) { this.open = false; }
    });
  }
});
afterAll(() => {
  Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal");
  Reflect.deleteProperty(HTMLDialogElement.prototype, "close");
});

beforeEach(() => {
  state.advanced = false;
  state.apps = [];
  state.topbarHost = null;
  state.fetchHostList.mockClear();
  state.live.request.mockReset();
  state.open.mockReset();
  state.live.request.mockImplementation(async () => ({ ok: true, json: async () => [] }));
  document.body.querySelectorAll("[data-test-titlebar]").forEach((element) => element.remove());
});

function titlebarAt(width: number): HTMLElement {
  const titlebar = document.createElement("div");
  titlebar.dataset.testTitlebar = "";
  const host = document.createElement("div");
  titlebar.append(host);
  document.body.append(titlebar);
  titlebar.getBoundingClientRect = () => ({
    width, height: 46, top: 0, left: 0, right: width, bottom: 46, x: 0, y: 0, toJSON: () => ({})
  }) as DOMRect;
  state.topbarHost = host;
  return titlebar;
}

const listing = (files: { id: string; name: string; directory: boolean; size: number }[]) =>
  state.live.request.mockImplementation(async (url: string, init?: RequestInit) => {
    if (init?.method === "DELETE") {
      const id = new URL(String(url), "https://localhost").searchParams.get("id");
      const index = files.findIndex((file) => file.id === id);
      if (index >= 0) files.splice(index, 1);
      return { ok: true, json: async () => ({}) };
    }
    if (String(url).startsWith("/api/v1/files?")) {
      const params = new URL(String(url), "https://localhost").searchParams;
      const query = (params.get("q") ?? "").toLocaleLowerCase();
      const filtered = query ? files.filter((file) => file.name.toLocaleLowerCase().includes(query)) : files;
      const limit = Number(params.get("limit") ?? filtered.length);
      const offset = Number(params.get("offset") ?? 0);
      return {
        ok: true,
        headers: new Headers({ "x-total-count": String(filtered.length) }),
        json: async () => filtered.slice(offset, offset + limit)
      };
    }
    return { ok: true, headers: new Headers(), json: async () => [] };
  });

test("browses personal files and hides host roots below advanced mode", async () => {
  render(<FilesApp />);
  const sidebar = screen.getByRole("complementary");
  expect(within(sidebar).getByRole("button", { name: "files.root" })).toBeInTheDocument();
  expect(within(sidebar).queryByRole("button", { name: "files.applications" })).toBeNull();
  expect(screen.getByRole("heading", { level: 1, name: "files.root" })).toBeInTheDocument();
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

test.each([1006, 1050, 1060])("keeps the titlebar command visible at %ipx", async (width) => {
  titlebarAt(width);
  render(<FilesApp />);
  await waitFor(() => expect(document.querySelector(".files-topbar__command")).toBeInTheDocument());
  expect(document.querySelector(".files-toolbar")).toBeNull();
});

test("falls back to the toolbar exactly when the titlebar command is hidden", async () => {
  titlebarAt(980);
  render(<FilesApp />);
  await waitFor(() => expect(document.querySelector(".files-toolbar")).toBeInTheDocument());
  expect(document.querySelector(".files-topbar__command")).toBeNull();
});

test("creates a new file and immediately renames it inline", async () => {
  let created = false;
  state.live.request.mockImplementation(async (url: string, init?: RequestInit) => {
    if (String(url).startsWith("/api/v1/files?")) {
      if (init?.method === "POST") { created = true; return { ok: true, json: async () => ({}) }; }
      return { ok: true, json: async () => created ? [{ id: "new-file", name: "files.newFile.txt", directory: false, size: 0 }] : [] };
    }
    return { ok: true, json: async () => [] };
  });
  render(<FilesApp />);
  await screen.findByText("files.empty");
  const surface = document.querySelector(".files-surface") as HTMLElement;
  fireEvent.contextMenu(surface, { clientX: 12, clientY: 12 });
  const menu = await screen.findByRole("menu");
  expect(menu).toHaveStyle({ left: "12px", top: "12px" });
  expect(within(menu).getByRole("menuitem", { name: "files.newFolder" })).toHaveFocus();
  fireEvent.click(within(menu).getByRole("menuitem", { name: "files.newFile" }));
  await waitFor(() => expect(state.live.request.mock.calls.some(([url, init]) => String(url).includes("directory=false") && (init as RequestInit | undefined)?.method === "POST")).toBe(true));
  const input = (await screen.findByRole("textbox", { name: "files.rename" })) as HTMLInputElement;
  expect(input).toHaveValue("files.newFile.txt");
  fireEvent.change(input, { target: { value: "notes.txt" } });
  fireEvent.blur(input);
  await waitFor(() => expect(state.live.request.mock.calls.some(([url, init]) => String(url).includes("name=notes.txt") && (init as RequestInit | undefined)?.method === "PUT")).toBe(true));
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
      return { ok: true, json: async () => [{ id: "file-2", name: "manual.pdf", directory: false, size: 12 }] };
    }
    return { ok: true, json: async () => [] };
  });
  render(<FilesApp />);
  const file = await screen.findByText("manual.pdf");
  fireEvent.doubleClick(file);
  expect(await screen.findByText("files.programMissing")).toBeInTheDocument();
});

test("opens a text file in the native text editor", async () => {
  state.live.request.mockImplementation(async (url: string) => {
    if (String(url).startsWith("/api/v1/files?")) {
      return { ok: true, json: async () => [{ id: "file-3", name: "notes.txt", directory: false, size: 12 }] };
    }
    return { ok: true, json: async () => [] };
  });
  render(<FilesApp />);
  const file = await screen.findByText("notes.txt");
  fireEvent.doubleClick(file);
  await waitFor(() => expect(state.open).toHaveBeenCalledWith("/app/editor?name=notes.txt&id=file-3"));
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

test("paginates a large folder and keeps the rendered page bounded", async () => {
  const files = Array.from({ length: 205 }, (_, index) => ({ id: `file-${index}`, name: `file-${index}.txt`, directory: false, size: 1 }));
  listing(files);
  render(<FilesApp />);
  await waitFor(() => expect(document.querySelectorAll(".files-grid .files-tile")).toHaveLength(200));
  expect(screen.getByRole("button", { name: "files.nextPage" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "files.nextPage" }));
  await waitFor(() => expect(document.querySelectorAll(".files-grid .files-tile")).toHaveLength(5));
  expect(screen.getByRole("button", { name: "files.nextPage" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "files.previousPage" }));
  await waitFor(() => expect(document.querySelectorAll(".files-grid .files-tile")).toHaveLength(200));
});

test("exposes the grid as a labeled list of selectable tiles", async () => {
  listing([{ id: "one", name: "mystery.xyz", directory: false, size: 12 }]);
  render(<FilesApp />);
  const grid = await screen.findByRole("list", { name: "files.title, files.grid" });
  const tile = within(grid).getByRole("button", { name: "mystery.xyz, XYZ" });
  expect(tile).toHaveAttribute("tabindex", "0");
  expect(tile).toHaveAttribute("aria-pressed", "false");
  fireEvent.keyDown(tile, { key: "Enter" });
  expect(await screen.findByText("files.noAssociation")).toBeInTheDocument();
});

test("moves focus with arrows and deletes the whole multi-selection after confirmation", async () => {
  const files = [
    { id: "one", name: "one.xyz", directory: false, size: 1 },
    { id: "two", name: "two.xyz", directory: false, size: 1 }
  ];
  listing(files);
  render(<FilesApp />);
  const options = await waitFor(() => {
    const tiles = [...document.querySelectorAll<HTMLElement>(".files-grid .files-tile")];
    expect(tiles).toHaveLength(2);
    return tiles;
  });
  expect(options[0]).toHaveAttribute("tabindex", "0");
  expect(options[1]).toHaveAttribute("tabindex", "-1");

  fireEvent.keyDown(options[0]!, { key: "ArrowDown" });
  await waitFor(() => expect(options[1]).toHaveAttribute("aria-pressed", "true"));
  expect(options[1]).toHaveFocus();

  fireEvent.keyDown(options[1]!, { key: "Backspace" });
  expect(screen.queryByRole("alertdialog")).toBeNull();

  fireEvent.click(options[0]!, { ctrlKey: true });
  await waitFor(() => expect(options[0]).toHaveAttribute("aria-pressed", "true"));
  expect(await screen.findByText("files.selectedCount")).toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent(/files\.items\s*·\s*files\.selectedCount/);

  fireEvent.keyDown(options[1]!, { key: "Delete" });
  const dialog = await screen.findByRole("alertdialog");
  expect(dialog).toHaveAccessibleName("files.delete");
  fireEvent.click(within(dialog).getByRole("button", { name: "files.delete" }));

  await waitFor(() => {
    const deletes = state.live.request.mock.calls.filter(([url, init]) => String(url).includes("id=") && (init as RequestInit | undefined)?.method === "DELETE");
    expect(deletes).toHaveLength(2);
  });
  await waitFor(() => expect(screen.queryByText("files.selectedCount")).toBeNull());
});

test("does not open the selected file when Enter is pressed on a toolbar button", async () => {
  listing([{ id: "one", name: "mystery.xyz", directory: false, size: 12 }]);
  render(<FilesApp />);
  const tile = await screen.findByRole("button", { name: "mystery.xyz, XYZ" });
  fireEvent.click(tile);
  const newFolder = screen.getByRole("button", { name: "files.newFolder" });
  newFolder.focus();
  fireEvent.keyDown(newFolder, { key: "Enter" });
  expect(screen.queryByText("files.noAssociation")).toBeNull();
  expect(tile).toHaveAttribute("aria-pressed", "true");
});

test("opens the per-item overflow menu and restores focus on Escape", async () => {
  listing([{ id: "one", name: "mystery.xyz", directory: false, size: 12 }]);
  render(<FilesApp />);
  const more = await screen.findByRole("button", { name: "files.actions: mystery.xyz" });
  more.focus();
  fireEvent.click(more);
  const menu = await screen.findByRole("menu");
  expect(within(menu).getByRole("menuitem", { name: "files.open" })).toHaveFocus();
  fireEvent.keyDown(menu, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  expect(more).toHaveFocus();
});

test("traps drawer focus and restores it to the locations toggle", async () => {
  render(<FilesApp />);
  const toggle = screen.getByRole("button", { name: "files.locations" });
  toggle.focus();
  fireEvent.click(toggle);
  const drawer = await screen.findByRole("dialog", { name: "files.locations" });
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(toggle).toHaveAttribute("aria-controls", "files-locations-drawer");
  expect(document.querySelector(".files-app__main")).toHaveAttribute("inert");
  await waitFor(() => expect(within(drawer).getByRole("button", { name: "files.dismiss" })).toHaveFocus());
  fireEvent.keyDown(drawer, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(toggle).toHaveFocus();
});

test("announces loading instead of zero items and marks the surface busy", async () => {
  let release: (value: unknown) => void = () => {};
  const pending = new Promise<unknown>((resolve) => { release = resolve; });
  state.live.request.mockImplementation(async (url: string) => {
    if (String(url).startsWith("/api/v1/files?")) return { ok: true, json: () => pending };
    return { ok: true, json: async () => [] };
  });
  render(<FilesApp />);
  const status = screen.getByRole("status");
  expect(status).toHaveTextContent("files.loading");
  expect(status).not.toHaveTextContent("files.items");
  const surface = document.querySelector(".files-surface");
  expect(surface).toHaveAttribute("aria-busy", "true");

  await act(async () => { release([{ id: "one", name: "one.txt", directory: false, size: 1 }]); });
  await waitFor(() => expect(status).toHaveTextContent("files.items"));
  expect(surface).toHaveAttribute("aria-busy", "false");
});

test("exposes explicit grid and list view radios", async () => {
  listing([{ id: "one", name: "mystery.xyz", directory: false, size: 12 }]);
  render(<FilesApp />);
  const grid = screen.getByRole("radio", { name: "files.gridView" });
  const list = screen.getByRole("radio", { name: "files.listView" });
  expect(grid).toBeChecked();

  fireEvent.click(list);
  expect(list).toBeChecked();
  expect(await screen.findByRole("table", { name: "files.title, files.list" })).toBeInTheDocument();

  fireEvent.click(grid);
  expect(grid).toBeChecked();
  expect(await screen.findByRole("list", { name: "files.title, files.grid" })).toBeInTheDocument();
});

test("shows the modified column only for host listings", async () => {
  state.advanced = true;
  state.fetchHostList.mockResolvedValue([{ name: "demo.bin", directory: false, size: 4, modified: 1_700_000_000 }]);
  render(<FilesApp />);
  await screen.findByText("files.empty");
  fireEvent.click(screen.getByRole("radio", { name: "files.listView" }));
  expect(screen.queryByRole("columnheader", { name: "files.modified" })).toBeNull();

  const sidebar = screen.getByRole("complementary");
  fireEvent.click(within(sidebar).getByRole("button", { name: "files.applications" }));
  expect(await screen.findByText("demo.bin")).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "files.modified" })).toBeInTheDocument();
});

test("reports a non-empty folder separately from a name conflict", async () => {
  const files = [{ id: "folder-1", name: "Projects", directory: true, size: 0 }];
  state.live.request.mockImplementation(async (url: string, init?: RequestInit) => {
    if (String(url).startsWith("/api/v1/files?")) {
      if (init?.method === "DELETE") return { ok: false, status: 409, json: async () => ({}) };
      return { ok: true, json: async () => files };
    }
    return { ok: true, json: async () => [] };
  });
  render(<FilesApp />);
  const folder = await screen.findByRole("button", { name: "Projects, files.folder" });
  fireEvent.click(folder);
  fireEvent.keyDown(folder, { key: "Delete" });
  const dialog = await screen.findByRole("alertdialog");
  fireEvent.click(within(dialog).getByRole("button", { name: "files.delete" }));
  expect(await screen.findByText("files.folderNotEmpty")).toBeInTheDocument();
});

test("marks the current breadcrumb and keeps the root crumb intact", async () => {
  listing([{ id: "folder-1", name: "Projects", directory: true, size: 0 }]);
  render(<FilesApp />);
  const nav = screen.getByRole("navigation", { name: "files.path" });
  const root = within(nav).getByRole("button", { name: "files.root" });
  expect(root).toHaveClass("files-crumbs__root");
  expect(root).toHaveAttribute("aria-current", "page");

  fireEvent.doubleClick(await screen.findByText("Projects"));
  await waitFor(() => expect(within(nav).getByRole("button", { name: "Projects" })).toHaveAttribute("aria-current", "page"));
  expect(root).not.toHaveAttribute("aria-current");
});

test("clears the search from the button and with Escape", async () => {
  listing([
    { id: "one", name: "notes.txt", directory: false, size: 1 },
    { id: "two", name: "photo.png", directory: false, size: 2 }
  ]);
  render(<FilesApp />);
  const input = await screen.findByRole("searchbox", { name: "files.search" });
  expect(screen.getByText("photo.png")).toBeInTheDocument();

  fireEvent.change(input, { target: { value: "notes" } });
  await waitFor(() => expect(screen.queryByText("photo.png")).toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "files.clearSearch" }));
  expect(input).toHaveValue("");
  expect(await screen.findByText("photo.png")).toBeInTheDocument();

  fireEvent.change(input, { target: { value: "notes" } });
  fireEvent.keyDown(input, { key: "Escape" });
  expect(input).toHaveValue("");
  expect(await screen.findByText("photo.png")).toBeInTheDocument();
});
