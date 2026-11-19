import type { ShellUserRole } from "@rumahl/contracts";

export type AdminUserStatus = "active" | "locked" | "disabled";

export interface AdminUser {
  id: string;
  username: string;
  displayName: string;
  status: AdminUserStatus;
  role: ShellUserRole;
}

export interface AdminSession {
  sessionId: string;
  userId: string;
  username: string;
  displayName: string;
  authenticatedAt: number;
  lastSeenAt: number;
  expiresAt: number;
  revokedAt: number | null;
  active: boolean;
}

export interface AdminAuditEntry {
  sequence: number;
  at: number;
  actor: string;
  action: string;
  outcome: "success" | "denied" | "failure";
  target: string | null;
}

export class AdminRequestError extends Error {
  constructor(readonly status: number) {
    super(`administration request failed with ${status}`);
    this.name = "AdminRequestError";
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    credentials: "same-origin",
    cache: "no-store",
    headers: { Accept: "application/json", ...init?.headers }
  });
  if (!response.ok) throw new AdminRequestError(response.status);
  return response.json() as Promise<T>;
}

function mutation(method: string, body?: unknown): RequestInit {
  return {
    method,
    headers: { "Content-Type": "application/json" },
    ...(body === undefined ? {} : { body: JSON.stringify(body) })
  };
}

export async function listAdminUsers(): Promise<AdminUser[]> {
  return (await request<{ users: AdminUser[] }>("/api/v1/admin/users")).users;
}

export async function createAdminUser(input: {
  username: string;
  displayName: string;
  password: string;
  role: ShellUserRole;
}): Promise<AdminUser> {
  return request<AdminUser>("/api/v1/admin/users", mutation("POST", input));
}

export async function updateAdminUser(
  id: string,
  input: { status?: AdminUserStatus; role?: ShellUserRole; username?: string; displayName?: string }
): Promise<AdminUser> {
  return request<AdminUser>(`/api/v1/admin/users/${encodeURIComponent(id)}`, mutation("PATCH", input));
}

export async function deleteAdminUser(id: string): Promise<void> {
  await request(`/api/v1/admin/users/${encodeURIComponent(id)}`, mutation("DELETE"));
}

export async function resetAdminPassword(id: string, password: string): Promise<void> {
  await request(`/api/v1/admin/users/${encodeURIComponent(id)}/password`, mutation("POST", { password }));
}

export async function listAdminSessions(): Promise<AdminSession[]> {
  return (await request<{ sessions: AdminSession[] }>("/api/v1/admin/sessions")).sessions;
}

export async function revokeAdminSession(sessionId: string): Promise<void> {
  await request(`/api/v1/admin/sessions/${encodeURIComponent(sessionId)}`, mutation("DELETE"));
}

export async function listAdminAudit(limit = 50, offset = 0): Promise<AdminAuditEntry[]> {
  const query = new URLSearchParams({ limit: String(limit), offset: String(offset) });
  return (await request<{ events: AdminAuditEntry[] }>(`/api/v1/admin/audit?${query}`)).events;
}
