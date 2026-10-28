import { describe, expect, test, vi } from "vitest";
import { demoSnapshot } from "./demo/snapshot";
import { fetchShellSnapshot, SHELL_SNAPSHOT_ENDPOINT } from "./snapshot-client";

describe("production shell snapshot client", () => {
  test("loads a contract-v1 snapshot without demo fallbacks", async () => {
    const request = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(demoSnapshot), {
        headers: { "Content-Type": "application/json" },
        status: 200
      })
    );

    await expect(fetchShellSnapshot(request)).resolves.toEqual(demoSnapshot);
    expect(request).toHaveBeenCalledWith(SHELL_SNAPSHOT_ENDPOINT, {
      cache: "no-store",
      credentials: "same-origin",
      headers: { Accept: "application/json" }
    });
  });

  test("fails closed for unavailable or malformed snapshots", async () => {
    const unavailable = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response(null, { status: 503 }));
    await expect(fetchShellSnapshot(unavailable)).rejects.toThrow("status 503");

    const malformed = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({
          ...demoSnapshot,
          systemStatus: {
            ...demoSnapshot.systemStatus,
            lastActivityAtUnixMs: demoSnapshot.systemStatus.observedAtUnixMs + 1
          }
        }),
        { status: 200 }
      )
    );
    await expect(fetchShellSnapshot(malformed)).rejects.toThrow("contract v1");
  });
});
