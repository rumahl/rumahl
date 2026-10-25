import { describe, expect, test, vi } from "vitest";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { DemoBackend } from "./demo/backend";
import { startShell } from "./runtime";
import { splashCopy } from "./splash";

describe("shared shell runtime", () => {
  test("runs demo transport through the production client and bootstrap", async () => {
    const backend = new DemoBackend();
    const render = vi.fn<(snapshot: ShellSnapshotV1) => void>();
    const renderFailure = vi.fn<() => void>();

    const snapshot = await (await backend.request("/api/v1/shell/snapshot")).json();
    await expect(
      startShell({ request: backend.request, openEvents: backend.openEvents, render, renderFailure })
    ).resolves.toBe(true);
    expect(render).toHaveBeenCalledWith(snapshot, {
      request: backend.request,
      openEvents: backend.openEvents,
      allowDevelopmentLoopback: false
    });
    expect(renderFailure).not.toHaveBeenCalled();
  });

  test("uses the same fail-closed startup path for transport failures", async () => {
    const render = vi.fn<(snapshot: ShellSnapshotV1) => void>();
    const renderFailure = vi.fn<() => void>();

    await expect(
      startShell({
        request: async () => new Response(null, { status: 503 }),
        render,
        renderFailure
      })
    ).resolves.toBe(false);
    expect(render).not.toHaveBeenCalled();
    expect(renderFailure).toHaveBeenCalledOnce();
  });

  test("reports boot progress and finishes the splash after paint", async () => {
    const backend = new DemoBackend();
    const progress = vi.fn<(value: number, label?: string) => void>();
    const finish = vi.fn<() => void>();
    const fail = vi.fn<(message: string) => void>();
    window.rumahlSplash = { progress, finish, fail };
    const frame = vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      callback(0);
      return 0;
    });

    try {
      await expect(
        startShell({
          request: backend.request,
          openEvents: backend.openEvents,
          render: vi.fn(),
          renderFailure: vi.fn()
        })
      ).resolves.toBe(true);
      expect(progress).toHaveBeenCalledWith(8, splashCopy("boot"));
      expect(progress).toHaveBeenCalledWith(58, splashCopy("dataLoaded"));
      expect(progress).toHaveBeenCalledWith(72, splashCopy("buildingDesktop"));
      expect(finish).toHaveBeenCalledOnce();
      expect(fail).not.toHaveBeenCalled();
    } finally {
      frame.mockRestore();
      delete window.rumahlSplash;
    }
  });
});
