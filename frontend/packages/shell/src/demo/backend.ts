import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { parseShellSnapshot } from "@rumahl/contracts/parse";
import { SHELL_EVENT_VERSION } from "@rumahl/contracts/events";
import { SHELL_EVENTS_ENDPOINT, type EventSocket, type OpenEvents } from "../live-updates";
import { SHELL_SNAPSHOT_ENDPOINT, type ShellRequest } from "../snapshot-client";
import { demoSnapshot } from "./snapshot";

export class DemoBackend {
  private snapshot: ShellSnapshotV1 = demoSnapshot;
  private readonly sockets = new Set<DemoEventSocket>();

  readonly request: ShellRequest = async (input, init) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";

    if (url === SHELL_SNAPSHOT_ENDPOINT && method === "GET") {
      return Response.json(this.snapshot, {
        headers: { "Cache-Control": "no-store" },
        status: 200
      });
    }

    const widget = url.match(/^\/api\/v1\/shell\/widgets\/([^/]+)\/frame$/);
    if (widget?.[1] && method === "GET") {
      const contributionId = decodeURIComponent(widget[1]);
      if (this.snapshot.contributions.some((item) => item.kind === "widget" && item.id === contributionId)) {
        return Response.json({
          frameVersion: 1,
          contributionId,
          frameUrl: "http://127.0.0.1:5174/"
        }, { status: 200 });
      }
    }

    return Response.json({ error: "demo route not found" }, { status: 404 });
  };

  readonly openEvents: OpenEvents = (url) => {
    if (new URL(url).pathname !== SHELL_EVENTS_ENDPOINT) {
      throw new Error("demo event route not found");
    }
    const socket = new DemoEventSocket(() => this.sockets.delete(socket));
    this.sockets.add(socket);
    queueMicrotask(() => socket.open());
    return socket;
  };

  publishSnapshot(value: unknown): void {
    this.snapshot = parseShellSnapshot(value);
    const event = JSON.stringify({
      eventVersion: SHELL_EVENT_VERSION,
      kind: "snapshot_changed",
      revision: this.snapshot.revision
    });
    for (const socket of this.sockets) socket.send(event);
  }
}

class DemoEventSocket implements EventSocket {
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onopen: ((event: Event) => void) | null = null;
  private closed = false;

  constructor(private readonly remove: () => void) {}

  open(): void {
    if (!this.closed) this.onopen?.(new Event("open"));
  }

  send(data: string): void {
    if (!this.closed) this.onmessage?.(new MessageEvent("message", { data }));
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.remove();
    this.onclose?.(new CloseEvent("close"));
  }
}
