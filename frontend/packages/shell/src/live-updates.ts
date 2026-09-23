import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { parseShellEvent } from "@rumahl/contracts/events";
import { SHELL_BUILD_ID } from "./build-id";
import { fetchShellSnapshot, ShellSnapshotHttpError, type ShellRequest } from "./snapshot-client";

export const SHELL_EVENTS_ENDPOINT = "/api/v1/shell/events";

export interface EventSocket {
  close(): void;
  onclose: ((event: CloseEvent) => void) | null;
  onerror: ((event: Event) => void) | null;
  onmessage: ((event: MessageEvent) => void) | null;
  onopen: ((event: Event) => void) | null;
}

export type OpenEvents = (url: string) => EventSocket;

export interface ShellLiveSource {
  request: ShellRequest;
  openEvents: OpenEvents;
  allowDevelopmentLoopback?: boolean | undefined;
}

export function browserEventSocket(url: string): EventSocket {
  return new WebSocket(url);
}

export function watchShellUpdates(
  initialRevision: string,
  source: ShellLiveSource,
  onSnapshot: (snapshot: ShellSnapshotV1) => void,
  onSessionExpired: () => void
): () => void {
  let stopped = false;
  let socket: EventSocket | null = null;
  let retry: ReturnType<typeof setTimeout> | null = null;
  let retryDelay = 1_000;
  let refreshRetry: ReturnType<typeof setTimeout> | null = null;
  let refreshDelay = 1_000;
  let currentRevision = initialRevision;
  let requested = false;
  let refreshing = false;
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  const url = `${protocol}//${location.host}${SHELL_EVENTS_ENDPOINT}`;

  function expire() {
    if (stopped) return;
    stop();
    onSessionExpired();
  }

  async function refresh() {
    if (refreshing || stopped) return;
    refreshing = true;
    while (requested && !stopped) {
      requested = false;
      try {
        const snapshot = await fetchShellSnapshot(source.request);
        if (stopped) break;
        if (snapshot.shellBuildId !== SHELL_BUILD_ID) {
          // Reload through the gateway so a matching immutable bundle is selected.
          stop();
          location.reload();
          break;
        }
        if (snapshot.revision !== currentRevision) {
          currentRevision = snapshot.revision;
          onSnapshot(snapshot);
        }
        refreshDelay = 1_000;
      } catch (error) {
        if (error instanceof ShellSnapshotHttpError && [401, 403].includes(error.status)) {
          expire();
        } else if (!stopped && refreshRetry === null) {
          refreshRetry = setTimeout(() => {
            refreshRetry = null;
            requestRefresh();
          }, refreshDelay);
          refreshDelay = Math.min(refreshDelay * 2, 30_000);
        }
      }
    }
    refreshing = false;
  }

  function requestRefresh() {
    if (refreshRetry !== null) clearTimeout(refreshRetry);
    refreshRetry = null;
    requested = true;
    void refresh();
  }

  function scheduleReconnect() {
    if (stopped || retry !== null) return;
    retry = setTimeout(() => {
      retry = null;
      connect();
    }, retryDelay);
    retryDelay = Math.min(retryDelay * 2, 30_000);
  }

  function connect() {
    if (stopped) return;
    try {
      const active = source.openEvents(url);
      socket = active;
      active.onopen = () => {
        if (stopped || socket !== active) return;
        retryDelay = 1_000;
        requestRefresh();
      };
      active.onmessage = (event) => {
        if (stopped || socket !== active) return;
        if (typeof event.data !== "string") {
          active.close();
          return;
        }
        try {
          const value = parseShellEvent(event.data);
          if (value.kind === "session_revoked") {
            expire();
          } else if (value.revision !== currentRevision) {
            requestRefresh();
          }
        } catch {
          active.close();
        }
      };
      active.onerror = () => active.close();
      active.onclose = (event) => {
        if (socket !== active) return;
        socket = null;
        if (event.code === 4401 || event.code === 4403) {
          expire();
          return;
        }
        scheduleReconnect();
      };
    } catch {
      scheduleReconnect();
    }
  }

  function stop() {
    stopped = true;
    if (retry !== null) clearTimeout(retry);
    if (refreshRetry !== null) clearTimeout(refreshRetry);
    retry = null;
    refreshRetry = null;
    if (socket) {
      socket.onclose = null;
      socket.close();
    }
    socket = null;
  }

  connect();
  return stop;
}
