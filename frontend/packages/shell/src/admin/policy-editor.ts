import type { AdminGroup, AdminPolicyAssignment, AdminPolicyDefinition, AdminPolicyValue } from "./api";

export interface PolicyDraft { mode: "inherit" | "custom"; text: string }
export interface PolicyResolution { value: AdminPolicyValue; source: string }

/** These enforcement points resolve only device assignments (LocalPolicyEngine). */
export function devicePolicy(key: string): boolean {
  return key.startsWith("rumahl.password.") || key === "rumahl.apps.storage_quota_bytes" || key === "rumahl.apps.allowed_publishers" || key === "rumahl.os.allow_full_access_themes";
}

export function policyUnit(definition: AdminPolicyDefinition): { factor: number; label: string } {
  if (definition.kind !== "number") return { factor: 1, label: "" };
  if (definition.key.endsWith("_bytes")) return { factor: 1024 * 1024, label: "MiB" };
  if (definition.key === "rumahl.session.lifetime_seconds") return { factor: 60, label: "min" };
  return { factor: 1, label: definition.key.endsWith("_seconds") ? "s" : "" };
}

/** Match Rust str::cmp, not locale-dependent collation, including non-BMP names. */
function compareNames(a: string, b: string): number {
  const left = Array.from(a, char => char.codePointAt(0)!);
  const right = Array.from(b, char => char.codePointAt(0)!);
  for (let i = 0; i < Math.min(left.length, right.length); i++) {
    if (left[i] !== right[i]) return left[i]! - right[i]!;
  }
  return left.length - right.length;
}

/** Display-only mirror of core::resolve_policy. Never used to authorize an action.
 * A group view is its baseline; individual members may have higher-priority rules.
 */
export function resolvePolicy(definition: AdminPolicyDefinition, subject: string, groups: readonly AdminGroup[], assignments: readonly AdminPolicyAssignment[]): PolicyResolution {
  const find = (target: string) => assignments.find(item => item.subject === target && item.key === definition.key);
  const candidates = ["device"];
  if (!devicePolicy(definition.key)) {
    if (subject.startsWith("user:")) {
      const userId = subject.slice(5);
      const memberships = groups.filter(group => group.members.includes(userId)).sort((a, b) => compareNames(a.name, b.name));
      candidates.unshift(subject, ...memberships.map(group => `group:${group.id}`));
    } else if (subject.startsWith("group:")) candidates.unshift(subject);
  }
  for (const source of candidates) {
    const assignment = find(source);
    if (assignment) return { value: assignment.value, source };
  }
  return { value: definition.default, source: "default" };
}

export function policyText(definition: AdminPolicyDefinition, value: AdminPolicyValue): string {
  if (definition.kind === "list") return (value as readonly string[]).join("\n");
  if (definition.kind === "number") return String(Number(value) / policyUnit(definition).factor);
  return String(value);
}

export type ParsedPolicy = { ok: true; value: AdminPolicyValue | null } | { ok: false };
export function parsePolicyDraft(definition: AdminPolicyDefinition, draft: PolicyDraft): ParsedPolicy {
  if (draft.mode === "inherit") return { ok: true, value: null };
  let value: AdminPolicyValue;
  let size: number;
  if (definition.kind === "boolean") {
    if (draft.text !== "true" && draft.text !== "false") return { ok: false };
    return { ok: true, value: draft.text === "true" };
  }
  if (definition.kind === "number") {
    value = Number(draft.text) * policyUnit(definition).factor;
    if (!draft.text.trim() || !Number.isSafeInteger(value) || value < 0) return { ok: false };
    size = value;
  } else if (definition.kind === "list") {
    value = draft.text.split("\n").map(line => line.trim()).filter(Boolean);
    if (value.some(item => new TextEncoder().encode(item).length > 255)) return { ok: false };
    size = value.length;
  } else {
    value = draft.text.trim();
    size = Array.from(value).length;
  }
  if ((definition.minimum !== null && size < definition.minimum) || (definition.maximum !== null && size > definition.maximum)) return { ok: false };
  return { ok: true, value };
}

export function samePolicy(a: AdminPolicyValue | null, b: AdminPolicyValue | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function replaceAssignment(assignments: readonly AdminPolicyAssignment[], subject: string, key: string, value: AdminPolicyValue | null): AdminPolicyAssignment[] {
  const next = assignments.filter(item => item.subject !== subject || item.key !== key);
  if (value !== null) next.push({ subject, key, value });
  return next;
}
