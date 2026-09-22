import { describe, expect, test, vi } from "vitest";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { DemoBackend } from "./demo/backend";
import { demoSnapshot } from "./demo/snapshot";
import { startShell } from "./runtime";

describe("shared shell runtime", () => {
  test("runs demo transport through the production client and bootstrap", async () => {
    const backend = new DemoBackend();
    const render = vi.fn<(snapshot: ShellSnapshotV1) => void>();
    const renderFailure = vi.fn<() => void>();

    await expect(
      startShell({ request: backend.request, render, renderFailure })
    ).resolves.toBe(true);
    expect(render).toHaveBeenCalledWith(demoSnapshot);
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
});
