import type { ShellRequest } from "./snapshot-client";

const SESSIONS_ENDPOINT = "/api/v1/shell/streams";
const ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

export interface StreamSession {
  id: string;
  title: string;
}

export function streamFramePath(id: string): string {
  if (!ID.test(id)) throw new Error("invalid stream session ID");
  return `${SESSIONS_ENDPOINT}/${id}/`;
}

export async function fetchStreamSessions(request: ShellRequest): Promise<readonly StreamSession[]> {
  const response = await request(SESSIONS_ENDPOINT, {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" }
  });
  if (!response.ok) throw new Error(`stream sessions unavailable: ${response.status}`);
  const payload: unknown = await response.json();
  if (typeof payload !== "object" || payload === null || !("sessions" in payload) ||
      !Array.isArray(payload.sessions) || payload.sessions.length > 64) {
    throw new Error("invalid stream session list");
  }
  return payload.sessions.map((value: unknown) => {
    if (typeof value !== "object" || value === null || !("id" in value) ||
        !("title" in value) || typeof value.id !== "string" || !ID.test(value.id) ||
        typeof value.title !== "string" || value.title.length < 1 || value.title.length > 128) {
      throw new Error("invalid stream session");
    }
    return { id: value.id, title: value.title };
  });
}

export async function grantStreamSession(request: ShellRequest, id: string): Promise<string> {
  const framePath = streamFramePath(id);
  const response = await request(`${SESSIONS_ENDPOINT}/${id}/grant`, {
    method: "POST",
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" }
  });
  if (!response.ok) throw new Error(`stream grant unavailable: ${response.status}`);
  const payload: unknown = await response.json();
  if (typeof payload !== "object" || payload === null || !("frameUrl" in payload) ||
      payload.frameUrl !== framePath) {
    throw new Error("invalid stream frame URL");
  }
  return framePath;
}
