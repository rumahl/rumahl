import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
  policy: { viewPlatformConfig: false },
  sensors: vi.fn(),
  config: vi.fn()
}));
vi.mock("@rumahl/ui", () => ({ useTheme: () => ({ theme: { name: "Iris" } }) }));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key, locale: "en" }) }));
vi.mock("../apps/recovery-client", () => ({ fetchSensors: (...args: unknown[]) => state.sensors(...args) }));
vi.mock("../preferences/client", () => ({ requestPlatformConfig: (...args: unknown[]) => state.config(...args) }));
vi.mock("../preferences/storage", () => ({ browserProfile: () => ({ id: "test-device" }) }));
vi.mock("../preferences/OsMode", () => ({ useOsModePolicy: () => state.policy }));
vi.mock("../system-labels", () => ({ buildLabel: () => "test build", osName: () => "rumahl OS" }));
vi.mock("../shell/ShellContext", () => ({
  useShell: () => ({
    snapshot: {
      shellBuildId: "test",
      revision: 1,
      user: { locale: "en" },
      systemStatus: { protection: "active", installedAppCount: 0, observedAtUnixMs: 0 }
    },
    mode: "desktop",
    live: { request: vi.fn(), demo: false }
  })
}));

import { SystemInfo } from "./SystemInfo";

beforeEach(() => {
  state.policy = { viewPlatformConfig: false };
  state.sensors.mockReset().mockResolvedValue(null);
  state.config.mockReset().mockResolvedValue({ entries: [] });
});

test("the platform configuration stays hidden below advanced mode", async () => {
  render(<SystemInfo />);
  expect(await screen.findByText("settings.system.info")).toBeInTheDocument();
  expect(screen.queryByText("settings.system.config")).toBeNull();
  expect(state.config).not.toHaveBeenCalled();
});

test("advanced mode lists the deployment configuration", async () => {
  state.policy = { viewPlatformConfig: true };
  state.config.mockResolvedValue({
    entries: [
      { name: "RUMAHL_PUBLIC_ORIGIN", value: "https://rumahl.test" },
      { name: "RUMAHL_LOCALE", value: "de" }
    ]
  });
  const view = render(<SystemInfo />);
  expect(await screen.findByText("settings.system.config")).toBeInTheDocument();
  await waitFor(() => expect(state.config).toHaveBeenCalledWith(expect.anything(), "test-device", expect.anything()));
  expect(screen.getByText("RUMAHL_PUBLIC_ORIGIN")).toBeInTheDocument();
  expect(screen.getByText("https://rumahl.test")).toBeInTheDocument();
  expect(screen.getByText("RUMAHL_LOCALE")).toBeInTheDocument();
  view.unmount();
});

test("an unavailable configuration reports itself instead of guessing", async () => {
  state.policy = { viewPlatformConfig: true };
  state.config.mockRejectedValue(new Error("forbidden"));
  const view = render(<SystemInfo />);
  expect(await screen.findByText("settings.system.config")).toBeInTheDocument();
  expect(await screen.findByText("settings.system.configSource")).toBeInTheDocument();
  expect(screen.getAllByText("settings.system.unavailable").length).toBeGreaterThan(0);
  view.unmount();
});
