import { render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import type { PropsWithChildren } from "react";
import type * as WorkspaceModule from "../preferences/Workspace";

const context = vi.hoisted(() => ({ path: "/app/files", focused: "app:files", mode: "desktop" }));
vi.mock("@rumahl/ui", () => ({ useTheme: () => ({ theme: {}, tokens: {}, variants: { windowChrome: "standard" } }) }));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("react-router", () => ({ useLocation: () => ({ pathname: context.path, search: "", hash: "" }) }));
vi.mock("../routing/routes", () => ({ describeRoute: (path: string) => ({ id: path.startsWith("/app/files") ? "app:files" : "settings" }), ShellRoutes: () => null }));
vi.mock("../routing/RouteBoundary", () => ({ RouteBoundary: ({ children }: PropsWithChildren) => children }));
vi.mock("../components/ProtectedWindow", () => ({ ProtectedWindow: ({ id, customTopbar, children }: PropsWithChildren<{ id: string; customTopbar: boolean }>) =>
  <section data-testid={id} data-custom={String(customTopbar)}>{children}</section> }));
vi.mock("../apps/registry", () => ({ findFirstPartyApp: (id: string) => id === "files" ? { customTopbar: true } : undefined }));
vi.mock("../preferences/Workspace", async () => ({
  ...await vi.importActual<typeof WorkspaceModule>("../preferences/Workspace"),
  useWorkspace: () => ({ effective: { fullscreenChrome: { hideDock: true } }, userWorkspace: null })
}));
vi.mock("./ShellContext", () => ({ useShell: () => ({
  state: { focusedWindowId: context.focused, windows: [
    { id: "app:files", title: "Files", subtitle: "", location: "/app/files" },
    { id: "settings", title: "Settings", subtitle: "", location: "/settings" }
  ] }, dispatch: () => {}, open: () => {}, mode: context.mode
}) }));

import { DesktopWindows } from "./DesktopWindows";
import { readFileSync } from "node:fs";
import { join } from "node:path";

beforeEach(() => { context.path = "/app/files"; context.focused = "app:files"; context.mode = "desktop"; });

test("each window keeps its own chrome when another app receives focus", () => {
  const view = render(<DesktopWindows animations={false} />);
  expect(screen.getByTestId("app:files")).toHaveAttribute("data-custom", "true");
  expect(screen.getByTestId("settings")).toHaveAttribute("data-custom", "false");
  context.path = "/settings";
  context.focused = "settings";
  view.rerender(<DesktopWindows animations={false} />);
  expect(screen.getByTestId("app:files")).toHaveAttribute("data-custom", "true");
  expect(screen.getByTestId("settings")).toHaveAttribute("data-custom", "false");
  context.path = "/app/files";
  context.focused = "app:files";
  view.rerender(<DesktopWindows animations={false} />);
  expect(screen.getByTestId("app:files")).toHaveAttribute("data-custom", "true");
  expect(screen.getByTestId("settings")).toHaveAttribute("data-custom", "false");
});

test("the focused window owns the highest stacking order", () => {
  const view = render(<DesktopWindows animations={false} />);
  const depth = (id: string) => Number((document.querySelector(`[data-testid="${id}"]`)?.closest(".window-position") as HTMLElement | null)?.style.zIndex ?? 0);
  expect(depth("app:files")).toBeGreaterThan(depth("settings"));
  context.focused = "settings";
  view.rerender(<DesktopWindows animations={false} />);
  expect(depth("settings")).toBeGreaterThan(depth("app:files"));
});

test("the launcher hides every window that is not the active route", () => {
  context.mode = "launcher";
  const view = render(<DesktopWindows animations={false} />);
  const wrapper = (id: string) => document.querySelector(`[data-testid="${id}"]`)?.closest(".window-position");
  expect(wrapper("settings")).toHaveClass("is-hidden");
  expect(wrapper("app:files")).not.toHaveClass("is-hidden");
  view.unmount();
});

test("hidden windows are styled through the motion wrapper", () => {
  // The window sits inside the motion element, so a rule that targets a direct
  // `.shell-window` child never matches and hidden windows stay clickable.
  const stylesheet = readFileSync(join(import.meta.dirname, "reference/reference.css"), "utf8");
  const index = stylesheet.indexOf(".window-position.is-hidden");
  expect(index).toBeGreaterThan(-1);
  expect(stylesheet.slice(index, index + 160)).toContain("> .window-motion > .shell-window");
});
