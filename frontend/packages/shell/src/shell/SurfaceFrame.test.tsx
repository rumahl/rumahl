import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const state = vi.hoisted(() => {
  class SafeModeError extends Error {
    constructor() { super("safe mode"); this.name = "SafeModeError"; }
  }
  return { requestSurfaceFrame: vi.fn(), fallback: vi.fn(), SafeModeError };
});

vi.mock("../apps/client", () => ({
  requestSurfaceFrame: state.requestSurfaceFrame,
  SafeModeError: state.SafeModeError
}));
vi.mock("./ShellContext", () => ({
  useShell: () => ({
    live: { request: vi.fn(), demo: undefined }
  })
}));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key, locale: "en" }) }));

import { SurfaceFrame } from "./SurfaceFrame";

const app = {
  id: "com.example.clock",
  installationId: "018f0e4a-7c3b-7d2e-8a91-1234567890ab",
  title: "Clock",
  version: "1.0.0",
  launchable: true
};
const descriptor = {
  surfaceId: "com.example.clock.desktop",
  appId: app.id,
  installationId: app.installationId,
  lease: "a".repeat(64),
  frameUrl: `https://${app.installationId}.apps.localhost:8443/launch/${"a".repeat(64)}/web/surface.html`,
  renewAfterSeconds: 30
};

beforeEach(() => {
  state.requestSurfaceFrame.mockReset();
  state.fallback.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

test("renders the signed surface in a script-only frame", async () => {
  state.requestSurfaceFrame.mockResolvedValue(descriptor);
  const { container } = render(
    <SurfaceFrame app={app} surfaceId="com.example.clock.desktop" size="medium" kind="companion" title="Clock" onFallback={state.fallback} />
  );
  const frame = await screen.findByTitle("surfaces.frame");
  expect(frame.tagName).toBe("IFRAME");
  expect(frame).toHaveAttribute("sandbox", "allow-scripts");
  expect(frame.getAttribute("sandbox")).not.toContain("allow-same-origin");
  expect(frame).toHaveAttribute("src", descriptor.frameUrl);
  expect(container.querySelector(".installed-surface-frame--companion")).not.toBeNull();
  expect(state.fallback).not.toHaveBeenCalled();
});

test("requests a fresh surface lease before rendering the frame", async () => {
  state.requestSurfaceFrame.mockResolvedValue(descriptor);
  render(
    <SurfaceFrame app={app} surfaceId="com.example.clock.desktop" size="small" kind="badge" title="Clock" onFallback={state.fallback} />
  );
  await screen.findByTitle("surfaces.frame");
  expect(state.requestSurfaceFrame).toHaveBeenCalled();
  const call = state.requestSurfaceFrame.mock.calls[0]!;
  expect(call[1]).toEqual(app);
  expect(call[2]).toBe("com.example.clock.desktop");
  expect(call[5]).toBeUndefined();
});

test("falls back instead of rendering an unauthorized or safe-mode frame", async () => {
  state.requestSurfaceFrame.mockRejectedValue(new Error("denied"));
  render(
    <SurfaceFrame app={app} surfaceId="com.example.clock.desktop" size="medium" kind="panel" title="Clock" onFallback={state.fallback} />
  );
  await waitFor(() => expect(state.fallback).toHaveBeenCalled());
  expect(screen.queryByTitle("surfaces.frame")).toBeNull();
});

test("safe mode hides the surface instead of showing a fallback card", async () => {
  state.requestSurfaceFrame.mockRejectedValue(new state.SafeModeError());
  render(
    <SurfaceFrame app={app} surfaceId="com.example.clock.desktop" size="medium" kind="panel" title="Clock" onFallback={state.fallback} />
  );
  await waitFor(() => expect(state.requestSurfaceFrame).toHaveBeenCalled());
  expect(state.fallback).not.toHaveBeenCalled();
  expect(screen.queryByTitle("surfaces.frame")).toBeNull();
});
