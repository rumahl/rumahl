import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  advanced: false,
  live: { request: vi.fn(async (url: string) => { void url; return { ok: true, json: async () => [] }; }) },
  fetchHostList: vi.fn(async () => []),
}));

vi.mock("../../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("../../shell/ShellContext", () => ({ useShell: () => ({ live: state.live }) }));
vi.mock("../../preferences/OsMode", () => ({ useOsModePolicy: () => ({ advancedSettings: state.advanced, browseSystemFiles: state.advanced, terminal: false, ssh: false, webConsole: false }) }));
vi.mock("../../preferences/storage", () => ({ browserProfile: () => ({ id: "01990000-0000-7000-8000-000000000001", persistence: "memory" }) }));
vi.mock("../fsClient", () => ({ fetchHostList: state.fetchHostList, hostContentUrl: () => "/content" }));

import { FilesApp } from "./FilesApp";

beforeEach(() => {
  state.advanced = false;
  state.fetchHostList.mockClear();
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
  fireEvent.click(await screen.findByRole("menuitem", { name: "files.newFile" }));
  const input = (await screen.findByPlaceholderText("files.newFile")) as HTMLInputElement;
  fireEvent.change(input, { target: { value: "notes.txt" } });
  fireEvent.submit(input.closest("form")!);
  await waitFor(() => {
    const call = state.live.request.mock.calls.find(([url]) => String(url).includes("directory=false"));
    expect(call).toBeTruthy();
    expect(String(call?.[0])).toContain("name=notes.txt");
  });
});
