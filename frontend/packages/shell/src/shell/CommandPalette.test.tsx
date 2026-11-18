import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const context = vi.hoisted(() => ({ owner: "account-a", denied: false, paletteOpen: true, run: vi.fn(), dispatch: vi.fn() }));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("../apps/useShellApps", () => ({ useShellApps: () => [] }));
vi.mock("../preferences/OsMode", () => ({ useOsModePolicy: () => ({ browseSystemFiles: false }) }));
vi.mock("../preferences/ShellPreferences", () => ({ useShellPreferences: () => ({ preferences: { ownerId: context.owner } }) }));
vi.mock("./commands", () => ({ useShellCommands: () => [
  { id: "allowed", title: "Allowed command", hint: "allowed", keywords: "allowed", run: context.run, disabled: context.denied },
  { id: "blocked", title: "Blocked command", hint: "blocked", keywords: "blocked", run: context.run, disabled: true }
] }));
vi.mock("./ShellContext", () => ({ useShell: () => ({ snapshot: { contributions: [] }, state: { commandPaletteOpen: context.paletteOpen }, dispatch: context.dispatch, open: vi.fn() }) }));

import { CommandPalette } from "./CommandPalette";

beforeEach(() => {
  localStorage.clear();
  context.owner = "account-a";
  context.paletteOpen = true;
  context.denied = false;
  context.run.mockReset();
  context.dispatch.mockReset();
});
afterEach(cleanup);

test("favorites persist and can be removed without executing a command", () => {
  const view = render(<CommandPalette />);
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "Allowed command" } });
  fireEvent.click(screen.getByRole("button", { name: "command.addFavorite" }));
  expect(context.run).not.toHaveBeenCalled();
  view.unmount();
  render(<CommandPalette />);
  fireEvent.click(screen.getByRole("button", { name: "command.favorites" }));
  expect(screen.getAllByRole("option")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "command.removeFavorite" }));
  expect(screen.queryByRole("option")).toBeNull();
});

test("Enter uses the selected target, records it and allows clearing history", () => {
  render(<CommandPalette />);
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "Allowed command" } });
  fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });
  expect(context.run).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "command.recent" }));
  expect(screen.getAllByRole("option")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "command.clearRecent" }));
  expect(screen.queryByRole("option")).toBeNull();
});

test("changing accounts never displays the previous account's favorites", () => {
  const view = render(<CommandPalette />);
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "Allowed command" } });
  fireEvent.click(screen.getByRole("button", { name: "command.addFavorite" }));
  context.owner = "account-b";
  view.rerender(<CommandPalette />);
  fireEvent.click(screen.getByRole("button", { name: "command.favorites" }));
  expect(screen.queryByRole("option")).toBeNull();
});

test("a previously saved command is hidden when it becomes unavailable", () => {
  localStorage.setItem("rumahl.search.v1:account-a", JSON.stringify({ favorites: ["allowed", "blocked", "removed"], recent: ["allowed"] }));
  const view = render(<CommandPalette />);
  context.denied = true;
  view.rerender(<CommandPalette />);
  fireEvent.click(screen.getByRole("button", { name: "command.favorites" }));
  expect(screen.queryByRole("option")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "command.recent" }));
  expect(screen.queryByRole("option")).toBeNull();
  expect(context.run).not.toHaveBeenCalled();
});

test("adding a favorite preserves keyboard selection and returns focus to search", () => {
  render(<CommandPalette />);
  const input = screen.getByRole("combobox");
  fireEvent.keyDown(input, { key: "ArrowDown" });
  const selected = input.getAttribute("aria-activedescendant");
  fireEvent.click(screen.getByRole("button", { name: "command.addFavorite" }));
  expect(input).toHaveAttribute("aria-activedescendant", selected);
  expect(document.activeElement).toBe(input);
});

test.each([true, false])("focus restoration on dismissal must not steal focus from an executed target (%s)", (execute) => {
  const source = document.createElement("button");
  document.body.append(source);
  source.focus();
  const focus = vi.spyOn(source, "focus");
  const view = render(<CommandPalette />);
  focus.mockClear();
  if (execute) {
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "Allowed command" } });
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });
  } else fireEvent.click(screen.getByRole("button", { name: "command.close" }));
  context.paletteOpen = false;
  view.rerender(<CommandPalette />);
  expect(focus).toHaveBeenCalledTimes(execute ? 0 : 1);
  source.remove();
});
