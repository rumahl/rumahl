import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  location: "/app/editor?id=notes&name=notes.txt",
  host: null as HTMLElement | null,
  live: { request: vi.fn() }, open: vi.fn()
}));
vi.mock("../../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("../../shell/ShellContext", () => ({ useShell: () => ({ live: state.live, open: state.open }) }));
vi.mock("../../routing/window-location", () => ({ useWindowLocation: () => state.location }));
vi.mock("../../preferences/storage", () => ({ browserProfile: () => ({ id: "test-device" }) }));
vi.mock("../../components/ProtectedWindow", () => ({ useAppTopbarHost: () => state.host }));
import { EditorApp } from "./EditorApp";

beforeAll(() => {
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", { configurable: true, value(this: HTMLDialogElement) { this.open = true; } });
  Object.defineProperty(HTMLDialogElement.prototype, "close", { configurable: true, value(this: HTMLDialogElement) { this.open = false; } });
});
beforeEach(() => {
  state.host?.remove(); state.host = null;
  state.location = "/app/editor?id=notes&name=notes.txt";
  state.live.request.mockReset().mockImplementation(async () => ({ ok: true, status: 200, arrayBuffer: async () => new TextEncoder().encode("Original text").buffer }));
});

test("plain text keeps its editable surface and truthful save feedback", async () => {
  render(<EditorApp />);
  const area = await screen.findByRole("textbox", { name: "editor.textArea" });
  expect(area).toHaveValue("Original text");
  fireEvent.change(area, { target: { value: "Draft" } });
  let finish!: (value: unknown) => void;
  state.live.request.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  fireEvent.click(screen.getByRole("button", { name: "editor.save" }));
  expect(screen.getByText("editor.saving")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "editor.save" })).toBeDisabled();
  await act(async () => finish({ ok: true }));
  expect(screen.getByText("editor.saved")).toBeInTheDocument();
  expect(state.live.request).toHaveBeenLastCalledWith(expect.any(String), expect.objectContaining({ method: "PUT", body: "Draft" }));
});

test("canceling reload preserves the unsaved draft", async () => {
  render(<EditorApp />);
  const area = await screen.findByRole("textbox");
  fireEvent.change(area, { target: { value: "Keep this draft" } });
  fireEvent.click(screen.getByRole("button", { name: "editor.reload" }));
  const dialog = await screen.findByRole("alertdialog");
  fireEvent.click(within(dialog).getByRole("button", { name: "editor.keepEditing" }));
  expect(area).toHaveValue("Keep this draft");
  expect(state.live.request).toHaveBeenCalledTimes(1);
});

test("Markdown uses shared format groups and a native hosted header", async () => {
  state.location = "/app/editor?id=notes&name=notes.md";
  state.live.request.mockImplementation(async () => ({ ok: true, status: 200, arrayBuffer: async () => new TextEncoder().encode("# Notes\n\nHello").buffer }));
  state.host = document.createElement("div"); document.body.append(state.host);
  const view = render(<EditorApp />);
  await screen.findByRole("heading", { name: "Notes" });
  expect(state.host.querySelector(".editor__bar--hosted")).not.toBeNull();
  expect(view.container.querySelector(".editor__bar")).toBeNull();
  expect(screen.getByRole("group", { name: "editor.format.textGroup" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "editor.mode.preview" }));
  await waitFor(() => expect(view.container.querySelector(".editor__sheet")).not.toBeNull());
});

test("host Markdown remains read-only, including every formatting action", async () => {
  state.location = "/app/editor?source=host&area=apps&path=notes.md&name=notes.md";
  render(<EditorApp />);
  const area = await screen.findByRole("textbox", { name: "editor.textArea" });
  expect(area).toHaveAttribute("aria-readonly", "true");
  for (const button of within(screen.getByRole("toolbar")).getAllByRole("button")) expect(button).toBeDisabled();
  expect(screen.getByRole("combobox")).toBeDisabled();
  expect(screen.getByRole("button", { name: "editor.save" })).toBeDisabled();
});
