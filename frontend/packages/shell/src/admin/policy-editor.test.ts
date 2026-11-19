import { expect, test } from "vitest";
import type { AdminGroup, AdminPolicyAssignment, AdminPolicyDefinition } from "./api";
import { devicePolicy, parsePolicyDraft, policyText, replaceAssignment, resolvePolicy } from "./policy-editor";

const definition: AdminPolicyDefinition = { key: "rumahl.files.quota_bytes", area: "files", kind: "number", default: 128 * 1024 * 1024, minimum: 16 * 1024 * 1024, maximum: 64 * 1024 * 1024 * 1024, description: "Quota" };
const groups: AdminGroup[] = [{ id: "b", name: "Beta", members: ["user"] }, { id: "a", name: "Alpha", members: ["user"] }, { id: "foreign", name: "Aardvark", members: ["other"] }];
const assignments: AdminPolicyAssignment[] = [
  { subject: "device", key: definition.key, value: 64 },
  { subject: "group:b", key: definition.key, value: 32 },
  { subject: "group:a", key: definition.key, value: 16 },
  { subject: "group:foreign", key: definition.key, value: 999 },
];
test("display resolution mirrors user > named membership > device > catalog", () => {
  expect(resolvePolicy(definition, "user:user", groups, assignments)).toEqual({ value: 16, source: "group:a" });
  expect(resolvePolicy(definition, "user:user", groups, [...assignments, { subject: "user:user", key: definition.key, value: 8 }])).toEqual({ value: 8, source: "user:user" });
  expect(resolvePolicy(definition, "user:nobody", groups, assignments)).toEqual({ value: 64, source: "device" });
  expect(resolvePolicy(definition, "user:user", groups, [])).toEqual({ value: definition.default, source: "default" });
  expect(resolvePolicy(definition, "group:b", groups, assignments)).toEqual({ value: 32, source: "group:b" });
});
test("device-wide enforcement ignores historical group and user assignments", () => {
  for (const key of ["rumahl.password.min_length", "rumahl.apps.storage_quota_bytes", "rumahl.apps.allowed_publishers"]) {
    expect(devicePolicy(key)).toBe(true);
    expect(resolvePolicy({ ...definition, key }, "user:user", groups, [{ subject: "user:user", key, value: 1 }, { subject: "group:a", key, value: 2 }]).source).toBe("default");
  }
  expect(devicePolicy("rumahl.os.allow_store_install")).toBe(false);
});
test("group names use Rust ordering, not locale collation", () => {
  const special = [{ id: "one", name: "😀", members: ["user"] }, { id: "two", name: "\ue000", members: ["user"] }];
  const rules = special.map((group, i) => ({ subject: `group:${group.id}`, key: definition.key, value: i }));
  expect(resolvePolicy(definition, "user:user", special, rules).source).toBe("group:two");
});
test("numbers convert exact units without rounding or accepting invalid drafts", () => {
  expect(parsePolicyDraft(definition, { mode: "custom", text: "64.5" })).toEqual({ ok: true, value: 64.5 * 1024 * 1024 });
  for (const text of ["", "-1", "1", "not-a-number", "Infinity", "99999999999999999", "16.0000000001"]) expect(parsePolicyDraft(definition, { mode: "custom", text }).ok).toBe(false);
  expect(policyText(definition, 16.5 * 1024 * 1024)).toBe("16.5");
  const session = { ...definition, key: "rumahl.session.lifetime_seconds", minimum: 300, maximum: 2592000 };
  expect(parsePolicyDraft(session, { mode: "custom", text: "720" })).toEqual({ ok: true, value: 43200 });
});
test("zero is valid for number rules that permit it", () => {
  expect(parsePolicyDraft({ ...definition, key: "rumahl.password.require_classes", minimum: null, maximum: 4 }, { mode: "custom", text: "0" })).toEqual({ ok: true, value: 0 });
});
test("explicit deny is different from inheriting a rule", () => {
  const boolean = { ...definition, kind: "boolean" as const, default: true };
  expect(parsePolicyDraft(boolean, { mode: "custom", text: "false" })).toEqual({ ok: true, value: false });
  expect(parsePolicyDraft(boolean, { mode: "inherit", text: "false" })).toEqual({ ok: true, value: null });
  expect(policyText(boolean, false)).toBe("false");
});
test("list drafts preserve an explicit empty list and enforce item bounds", () => {
  const list = { ...definition, kind: "list" as const, default: [], minimum: null, maximum: 2 };
  expect(parsePolicyDraft(list, { mode: "custom", text: "com.rumahl\n\n org.example " })).toEqual({ ok: true, value: ["com.rumahl", "org.example"] });
  expect(parsePolicyDraft(list, { mode: "custom", text: "" })).toEqual({ ok: true, value: [] });
  expect(parsePolicyDraft(list, { mode: "custom", text: "a\nb\nc" }).ok).toBe(false);
  expect(parsePolicyDraft(list, { mode: "custom", text: "é".repeat(128) }).ok).toBe(false);
});
test("replacing an assignment never touches another target or policy", () => {
  const next = replaceAssignment(assignments, "group:a", definition.key, null);
  expect(next).toHaveLength(assignments.length - 1);
  expect(assignments).toHaveLength(4);
  expect(next.some(item => item.subject === "group:b")).toBe(true);
});
