import { afterEach, describe, expect, test, vi } from "vitest";
import { DemoBackend } from "./demo/backend";
import { demoSnapshot } from "./demo/snapshot";
import { watchShellUpdates } from "./live-updates";

afterEach(() => vi.restoreAllMocks());

describe("shell revision events", () => {
  test("refetches authoritative state through the same client used at startup", async () => {
    const backend = new DemoBackend();
    const request = vi.spyOn(backend, "publishSnapshot");
    const onSnapshot = vi.fn();
    const stop = watchShellUpdates(
      demoSnapshot.revision,
      { request: backend.request, openEvents: backend.openEvents },
      onSnapshot,
      vi.fn()
    );

    await Promise.resolve();
    const updated = {
      ...demoSnapshot,
      revision: "demo-revision-002",
      systemStatus: { ...demoSnapshot.systemStatus, installedAppCount: 7 }
    };
    backend.publishSnapshot(updated);
    await vi.waitFor(() => expect(onSnapshot).toHaveBeenCalledWith(updated));
    expect(request).toHaveBeenCalledOnce();
    stop();

    backend.publishSnapshot({ ...updated, revision: "demo-revision-003" });
    expect(onSnapshot).toHaveBeenCalledOnce();
  });

  test("stops and drops private shell state after a revocation event", async () => {
    const backend = new DemoBackend();
    const expired = vi.fn();
    let send: ((data: string) => void) | undefined;
    const stop = watchShellUpdates(
      demoSnapshot.revision,
      {
        request: backend.request,
        openEvents: (url) => {
          const socket = backend.openEvents(url);
          send = (data) => socket.onmessage?.(new MessageEvent("message", { data }));
          return socket;
        }
      },
      vi.fn(),
      expired
    );

    send?.(JSON.stringify({ eventVersion: 1, kind: "session_revoked" }));
    expect(expired).toHaveBeenCalledOnce();
    stop();
  });

  test("a missing event transport does not affect the usable snapshot", () => {
    const onSnapshot = vi.fn();
    const stop = watchShellUpdates(
      demoSnapshot.revision,
      {
        request: async () => { throw new Error("should not fetch without an event connection"); },
        openEvents: () => { throw new Error("container or event service unavailable"); }
      },
      onSnapshot,
      vi.fn()
    );
    expect(onSnapshot).not.toHaveBeenCalled();
    stop();
  });

  test("recovers an authoritative snapshot after a transient API outage", async () => {
    const backend = new DemoBackend();
    const onSnapshot = vi.fn();
    let attempts = 0;
    const stop = watchShellUpdates(
      "older-revision",
      {
        openEvents: backend.openEvents,
        request: (input, init) => {
          attempts += 1;
          return attempts === 1
            ? Promise.resolve(new Response(null, { status: 503 }))
            : backend.request(input, init);
        }
      },
      onSnapshot,
      vi.fn()
    );

    await vi.waitFor(() => expect(onSnapshot).toHaveBeenCalledWith(demoSnapshot), {
      timeout: 2_500
    });
    expect(attempts).toBeGreaterThanOrEqual(2);
    stop();
  });
  test("rechecks the session when a WebSocket handshake is rejected", async () => {
    const expired = vi.fn();
    let disconnect: (() => void) | undefined;
    const stop = watchShellUpdates("initial-revision", {
      request: async () => new Response(null, { status: 401 }),
      openEvents: () => {
        const socket = {
          close: vi.fn(), onopen: null, onerror: null, onmessage: null,
          onclose: null as ((event: CloseEvent) => void) | null
        };
        disconnect = () => socket.onclose?.(new CloseEvent("close", { code: 1006 }));
        return socket;
      }
    }, vi.fn(), expired);
    disconnect?.();
    await vi.waitFor(() => expect(expired).toHaveBeenCalledOnce());
    stop();
  });

});
