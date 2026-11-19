import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  mode: "desktop" as "desktop" | "launcher" | "mobile",
  safe: false,
  disabled: [] as string[],
  contributions: [] as Record<string, unknown>[],
  apps: [] as { id: string; title: string; path: string; launchable: boolean; system: boolean; kind: "app" }[],
  catalogApps: [] as { id: string; installationId: string; title: string; version: string; launchable: boolean }[]
}));
const navigate = vi.hoisted(() => vi.fn());

vi.mock("react-router", () => ({ useNavigate: () => navigate }));
vi.mock("./ShellContext", () => ({
  useShell: () => ({ mode: state.mode, snapshot: { contributions: state.contributions } })
}));
vi.mock("../apps/useShellApps", () => ({ useShellApps: () => state.apps }));
vi.mock("../preferences/disabled-apps", () => ({ useDisabledApps: () => state.disabled }));
vi.mock("../preferences/safe-mode", () => ({ useSafeMode: () => state.safe }));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key, locale: "en" }) }));
vi.mock("../apps/AppCatalog", () => ({ useAppCatalog: () => ({ apps: state.catalogApps, preview: [], status: "ready" }) }));
vi.mock("./SurfaceFrame", () => ({
  SurfaceFrame: (props: { surfaceId: string }) => <div data-testid="surface-frame">{props.surfaceId}</div>
}));

import { InstalledSurfaces } from "./InstalledSurfaces";

const surface = {
  kind: "surface",
  id: "com.example.clock.desktop",
  app: "com.example.clock",
  title: "Clock",
  slot: "both",
  surface: "companion",
  size: "medium",
  placement: "bottom-right",
  tone: "accent",
  icon: "pulse"
};

beforeEach(() => {
  state.mode = "desktop";
  state.safe = false;
  state.disabled = [];
  state.contributions = [surface];
  state.apps = [{ id: "com.example.clock", title: "Clock", path: "/app/com.example.clock", launchable: true, system: false, kind: "app" }];
  state.catalogApps = [];
  navigate.mockClear();
});

test("renders a declarative surface and opens the installed app", () => {
  render(<InstalledSurfaces />);
  const button = screen.getByRole("button", { name: "surfaces.open" });
  expect(button).toHaveAttribute("title", "Clock");
  fireEvent.click(button);
  expect(navigate).toHaveBeenCalledWith("/app/com.example.clock");
});

test("hides surfaces in safe mode, when disabled, or outside the slot", () => {
  state.safe = true;
  const { unmount } = render(<InstalledSurfaces />);
  expect(screen.queryByRole("button", { name: "surfaces.open" })).toBeNull();
  unmount();

  state.safe = false;
  state.disabled = ["com.example.clock"];
  render(<InstalledSurfaces />);
  expect(screen.queryByRole("button", { name: "surfaces.open" })).toBeNull();
});

test("respects desktop/launcher slots and only renders installed apps", () => {
  for (const next of [{ slot: "desktop" }, { slot: "launcher" }]) {
    state.contributions = [{ ...surface, ...next }];
    state.mode = next.slot === "desktop" ? "desktop" : "launcher";
    const { unmount } = render(<InstalledSurfaces />);
    expect(screen.getByRole("button", { name: "surfaces.open" })).toBeInTheDocument();
    unmount();
    state.mode = next.slot === "desktop" ? "launcher" : "desktop";
    render(<InstalledSurfaces />);
    expect(screen.queryByRole("button", { name: "surfaces.open" })).toBeNull();
  }

  state.contributions = [surface];
  state.mode = "desktop";
  state.apps = [];
  render(<InstalledSurfaces />);
  expect(screen.queryByRole("button", { name: "surfaces.open" })).toBeNull();
});

test("uses an isolated surface frame when the app declares an entrypoint", () => {
  state.contributions = [{ ...surface, entrypoint: "overlay" }];
  state.catalogApps = [{
    id: "com.example.clock",
    installationId: "018f0e4a-7c3b-7d2e-8a91-1234567890ab",
    title: "Clock",
    version: "1.0.0",
    launchable: true
  }];
  render(<InstalledSurfaces />);
  expect(screen.getByTestId("surface-frame")).toHaveTextContent("com.example.clock.desktop");
  expect(screen.queryByRole("button", { name: "surfaces.open" })).toBeNull();
});

test("falls back to the declarative card until a frame target is available", () => {
  state.contributions = [{ ...surface, entrypoint: "overlay" }];
  state.catalogApps = [];
  render(<InstalledSurfaces />);
  expect(screen.queryByTestId("surface-frame")).toBeNull();
  expect(screen.getByRole("button", { name: "surfaces.open" })).toBeInTheDocument();
});
