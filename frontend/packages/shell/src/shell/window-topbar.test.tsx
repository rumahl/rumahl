import { render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import type { PropsWithChildren } from "react";
import type * as WorkspaceModule from "../preferences/Workspace";

const context = vi.hoisted(() => ({ path: "/app/files", focused: "app:files" }));
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
  ] }, dispatch: () => {}, open: () => {}, mode: "desktop"
}) }));

import { DesktopWindows } from "./DesktopWindows";

beforeEach(() => { context.path = "/app/files"; context.focused = "app:files"; });

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
