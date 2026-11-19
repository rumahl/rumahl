import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import type * as WorkspaceModule from "./Workspace";
const context = vi.hoisted(() => ({ save: vi.fn(), busy: false }));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("../components/SettingsHeading", () => ({ SettingsHeading: ({ title }: { title: string }) => <h1>{title}</h1> }));
vi.mock("./Workspace", async () => {
  const actual = await vi.importActual<typeof WorkspaceModule>("./Workspace");
  return { ...actual, useWorkspace: () => ({
    ready: true, busy: context.busy, error: null, userWorkspace: null, save: context.save,
    effective: { ...actual.emptyWorkspace, folders: [{ id: "keep", name: "Keep", apps: [] }], fullscreenChrome: { hideTopbar: false, hideDock: false, dockClearance: true } }
  }) };
});
import { FullscreenChromeSettings } from "./FullscreenChromeSettings";
beforeEach(() => { context.busy = false; context.save.mockReset(); context.save.mockResolvedValue(true); });

test("chrome reset preserves the visible arrangement and the independent spacing option", () => {
  render(<FullscreenChromeSettings />);
  fireEvent.click(screen.getByRole("button", { name: "workspace.resetChrome" }));
  expect(context.save).toHaveBeenCalledWith("user", expect.objectContaining({
    folders: [{ id: "keep", name: "Keep", apps: [] }],
    fullscreenChrome: { hideTopbar: true, hideDock: true, dockClearance: true }
  }));
});

test("spacing reset changes spacing only", () => {
  render(<FullscreenChromeSettings />);
  fireEvent.click(screen.getByRole("button", { name: "workspace.resetSpace" }));
  expect(context.save).toHaveBeenCalledWith("user", expect.objectContaining({ fullscreenChrome: { hideTopbar: false, hideDock: false, dockClearance: false } }));
});

test("resets are disabled while a save is pending", () => {
  context.busy = true;
  render(<FullscreenChromeSettings />);
  expect(screen.getByRole("button", { name: "workspace.resetChrome" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "workspace.resetSpace" })).toBeDisabled();
});
