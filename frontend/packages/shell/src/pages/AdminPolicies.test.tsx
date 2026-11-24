import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import type { AdminPolicies, AdminPolicyValue } from "../admin/api";
import { replaceAssignment } from "../admin/policy-editor";

vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key, locale: "en" }) }));
vi.mock("../components/SettingsHeading", () => ({ SettingsHeading: ({ title }: { title: string }) => <h1>{title}</h1> }));
vi.mock("../components/LoadingStatus", () => ({ LoadingStatus: () => <p>loading</p> }));
vi.mock("../components/Button", () => ({ Button: (props: React.ButtonHTMLAttributes<HTMLButtonElement> & { loading?: boolean }) => <button type={props.type ?? "button"} disabled={props.disabled || props.loading} onClick={props.onClick}>{props.children}</button> }));
vi.mock("../routing/ShellLink", () => ({ ShellLink: (props: React.AnchorHTMLAttributes<HTMLAnchorElement>) => <a href={props.href ?? "#"}>{props.children}</a> }));
vi.mock("@rumahl/ui/components", () => ({
  RumahlInput: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />,
  RumahlModal: ({ open, children }: { open: boolean; children: React.ReactNode }) => open ? <div role="dialog">{children}</div> : null,
}));

import { AdminGroupsSettings, AdminPoliciesSettings } from "./AdminPolicies";

const initialPolicies: AdminPolicies = {
  policiesVersion: 1,
  policies: [
    { key: "rumahl.password.min_length", area: "password", kind: "number", default: 15, minimum: 6, maximum: 128, description: "Minimum password length." },
    { key: "rumahl.session.lifetime_seconds", area: "session", kind: "number", default: 43200, minimum: 300, maximum: 2592000, description: "Session lifetime." },
    { key: "rumahl.os.advanced_mode", area: "os", kind: "boolean", default: true, minimum: null, maximum: null, description: "Advanced exposure mode." },
    { key: "rumahl.home.cross_user_read", area: "home", kind: "boolean", default: false, minimum: null, maximum: null, description: "Administrators may read other homes." },
    { key: "rumahl.files.quota_bytes", area: "files", kind: "number", default: 134217728, minimum: 16777216, maximum: 68719476736, description: "Personal files budget." },
    { key: "rumahl.apps.allowed_publishers", area: "apps", kind: "list", default: [], minimum: null, maximum: 64, description: "Trusted publishers." },
  ],
  assignments: [
    { subject: "device", key: "rumahl.files.quota_bytes", value: 33554432 },
    { subject: "device", key: "rumahl.apps.allowed_publishers", value: ["com.rumahl"] },
  ],
};
let policies: AdminPolicies;
const users = [
  { id: "alice", username: "alice", displayName: "Alice", status: "active", role: "owner" },
  { id: "bob", username: "bob", displayName: "Bob", status: "active", role: "user" },
];
const groups = [{ id: "engineering", name: "engineering", members: ["bob"] }];
const listPolicies = vi.fn(async () => structuredClone(policies));
const persist = async (subject: string, key: string, value: AdminPolicyValue | null) => {
  policies.assignments = replaceAssignment(policies.assignments, subject, key, value);
};
const setPolicy = vi.fn(persist);
const createGroup = vi.fn<(name: string) => Promise<(typeof groups)[number]>>(async () => groups[0]!);
const deleteGroup = vi.fn<(id: string) => Promise<void>>(async () => {});
const updateMembers = vi.fn<(id: string, add: readonly string[], remove: readonly string[]) => Promise<(typeof groups)[number]>>(async () => groups[0]!);
vi.mock("../admin/api", () => ({
  AdminRequestError: class extends Error {
    status: number;
    constructor(status: number) {
      super("failed");
      this.status = status;
    }
  },
  listAdminPolicies: () => listPolicies(), listAdminUsers: async () => users, listAdminGroups: async () => groups,
  setAdminPolicy: (...args: Parameters<typeof persist>) => setPolicy(...args),
  createAdminGroup: (...args: [string]) => createGroup(...args),
  deleteAdminGroup: (...args: [string]) => deleteGroup(...args),
  updateAdminGroupMembers: (...args: [string, readonly string[], readonly string[]]) => updateMembers(...args),
}));
beforeEach(() => {
  policies = structuredClone(initialPolicies);
  listPolicies.mockReset().mockImplementation(async () => structuredClone(policies));
  setPolicy.mockReset().mockImplementation(persist);
  createGroup.mockClear(); deleteGroup.mockClear(); updateMembers.mockClear();
});
const row = (key: string) => document.querySelector<HTMLElement>(`[data-policy-key="rumahl.${key}"]`)!;
const source = (key: string) => row(key).querySelector(".policy-row__source")!;
async function ready() { await waitFor(() => expect(row("os.advanced_mode")).not.toBeNull()); }
function tab(label: string) { return screen.getByRole("tab", { name: label }); }

test("the scope is visible as tabs: device starts selected with every rule editable", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  // Device / Groups / Accounts are tabs on screen, Device selected first.
  expect(tab("admin.policies.subjectDevice")).toHaveAttribute("aria-selected", "true");
  expect(tab("admin.policies.tabGroups")).toHaveAttribute("aria-selected", "false");
  expect(tab("admin.policies.tabUsers")).toBeInTheDocument();
  // On the device tab every rule of the catalog appears, nothing disabled.
  expect(screen.getByText("admin.policies.area.password")).toBeInTheDocument();
  expect(row("password.min_length")).toBeInTheDocument();
  expect(row("files.quota_bytes")).toBeInTheDocument();
  expect(document.querySelectorAll('[role="switch"]:disabled')).toHaveLength(0);
  expect(setPolicy).not.toHaveBeenCalled();
});

test("number fields show their unit inside the input", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  // Byte budgets carry MiB, the session lifetime min; a plain count
  // (password characters) has no unit label at all.
  expect(within(row("files.quota_bytes")).getByText("MiB")).toBeInTheDocument();
  expect(within(row("session.lifetime_seconds")).getByText("min")).toBeInTheDocument();
  expect(within(row("password.min_length")).queryByText("s")).toBeNull();
});

test("the publisher allowlist is a wide, tall textarea", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  const field = within(row("apps.allowed_publishers")).getByRole("textbox") as HTMLTextAreaElement;
  expect(field.rows).toBeGreaterThanOrEqual(6);
  expect(row("apps.allowed_publishers")).toHaveClass("policy-row--wide");
});

test("machine-wide rules move to the device tab instead of greying out", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  fireEvent.click(tab("admin.policies.tabGroups"));
  // The group chip is visible without opening any dropdown.
  expect(await screen.findByRole("button", { name: "engineering" })).toHaveAttribute("aria-pressed", "true");
  // Machine-wide rules are absent — not disabled — with one note instead.
  expect(row("password.min_length")).toBeNull();
  expect(row("apps.allowed_publishers")).toBeNull();
  expect(screen.getByText("admin.policies.deviceWideNote")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "admin.policies.openDeviceTab" }));
  await waitFor(() => expect(row("password.min_length")).not.toBeNull());
  expect(tab("admin.policies.subjectDevice")).toHaveAttribute("aria-selected", "true");
});

test("group rows show inherited device values and allow overrides", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  fireEvent.click(tab("admin.policies.tabGroups"));
  // Inherited from the device, labelled as such, pre-filled with its value.
  await waitFor(() => expect(source("files.quota_bytes")).toHaveTextContent("32 MiB · admin.policies.subjectDevice"));
  expect((within(row("files.quota_bytes")).getByRole("spinbutton") as HTMLInputElement).value).toBe("32");
  const input = within(row("files.quota_bytes")).getByRole("spinbutton");
  await act(async () => {
    fireEvent.change(input, { target: { value: "64" } });
    fireEvent.blur(input);
  });
  expect(setPolicy).toHaveBeenCalledExactlyOnceWith("group:engineering", "rumahl.files.quota_bytes", 64 * 1024 * 1024);
  await waitFor(() => expect(source("files.quota_bytes")).toHaveTextContent("64 MiB · admin.policies.assignedHere"));
  // The own assignment makes Clear available, which removes the override.
  await act(async () => { fireEvent.click(within(row("files.quota_bytes")).getByRole("button", { name: "admin.policies.clear" })); });
  expect(setPolicy).toHaveBeenLastCalledWith("group:engineering", "rumahl.files.quota_bytes", null);
  await waitFor(() => expect(source("files.quota_bytes")).toHaveTextContent("32 MiB · admin.policies.subjectDevice"));
});

test("account rows resolve group inheritance", async () => {
  policies.assignments.push({ subject: "group:engineering", key: "rumahl.files.quota_bytes", value: 16 * 1024 * 1024 });
  render(<AdminPoliciesSettings />);
  await ready();
  fireEvent.click(tab("admin.policies.tabUsers"));
  // Account chips; bob is picked after alice (first is auto-selected).
  expect(screen.getByRole("group", { name: "admin.policies.chooseUser" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /bob/i }));
  await waitFor(() => expect(source("files.quota_bytes")).toHaveTextContent("16 MiB · admin.policies.subjectGroup"));
  expect((within(row("files.quota_bytes")).getByRole("spinbutton") as HTMLInputElement).value).toBe("16");
});

test("number edits apply immediately with unit conversion", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  const input = within(row("files.quota_bytes")).getByRole("spinbutton") as HTMLInputElement;
  expect(input.value).toBe("32");
  await act(async () => {
    fireEvent.change(input, { target: { value: "64" } });
    fireEvent.blur(input);
  });
  expect(setPolicy).toHaveBeenCalledExactlyOnceWith("device", "rumahl.files.quota_bytes", 64 * 1024 * 1024);
  await waitFor(() => expect(source("files.quota_bytes")).toHaveTextContent("64 MiB"));
  // Unchanged or out-of-bounds edits write nothing.
  await act(async () => { fireEvent.blur(input); });
  await act(async () => {
    fireEvent.change(input, { target: { value: "1" } }); // below the 16 MiB minimum
    fireEvent.blur(input);
  });
  expect(setPolicy).toHaveBeenCalledTimes(1);
});

test("session lifetime edits apply in minutes", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  const input = within(row("session.lifetime_seconds")).getByRole("spinbutton") as HTMLInputElement;
  expect(input.value).toBe("720");
  await act(async () => {
    fireEvent.change(input, { target: { value: "10" } });
    fireEvent.blur(input);
  });
  expect(setPolicy).toHaveBeenCalledExactlyOnceWith("device", "rumahl.session.lifetime_seconds", 600);
});

test("booleans apply on toggle and clear back to inheritance", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  const toggle = within(row("home.cross_user_read")).getByRole("switch");
  await act(async () => { fireEvent.click(toggle); });
  expect(setPolicy).toHaveBeenCalledExactlyOnceWith("device", "rumahl.home.cross_user_read", true);
  const clear = await within(row("home.cross_user_read")).findByRole("button", { name: "admin.policies.clear" });
  await act(async () => { fireEvent.click(clear); });
  expect(setPolicy).toHaveBeenLastCalledWith("device", "rumahl.home.cross_user_read", null);
  await waitFor(() => expect(source("home.cross_user_read")).toHaveTextContent("admin.policies.off · admin.policies.sourceDefault"));
});

test("publisher lists apply one entry per line; empty is an explicit empty list", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  const field = within(row("apps.allowed_publishers")).getByRole("textbox") as HTMLTextAreaElement;
  expect(field.value).toBe("com.rumahl");
  await act(async () => {
    fireEvent.change(field, { target: { value: "com.rumahl\n org.example\n" } });
    fireEvent.blur(field);
  });
  expect(setPolicy).toHaveBeenCalledExactlyOnceWith("device", "rumahl.apps.allowed_publishers", ["com.rumahl", "org.example"]);
  await act(async () => {
    fireEvent.change(field, { target: { value: "" } });
    fireEvent.blur(field);
  });
  expect(setPolicy).toHaveBeenLastCalledWith("device", "rumahl.apps.allowed_publishers", []);
});

test("failed saves surface the error and keep the catalog unchanged", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  setPolicy.mockRejectedValueOnce(new Error("offline"));
  const input = within(row("files.quota_bytes")).getByRole("spinbutton");
  await act(async () => {
    fireEvent.change(input, { target: { value: "64" } });
    fireEvent.blur(input);
  });
  expect(await screen.findByText("admin.error.save")).toBeInTheDocument();
  expect(source("files.quota_bytes")).toHaveTextContent("32 MiB");
});

test("the search filters by label, key and description on every tab", async () => {
  render(<AdminPoliciesSettings />);
  await ready();
  const filter = screen.getByRole("searchbox", { name: "admin.policies.filter" });
  await act(async () => { fireEvent.change(filter, { target: { value: "publisher" } }); });
  expect(row("os.advanced_mode")).toBeNull();
  expect(row("apps.allowed_publishers")).not.toBeNull();
  await act(async () => { fireEvent.change(filter, { target: { value: "rumahl.password" } }); });
  expect(row("password.min_length")).not.toBeNull();
  expect(row("apps.allowed_publishers")).toBeNull();
});

test("load failure shows the error and no writable rows", async () => {
  listPolicies.mockRejectedValueOnce(new Error("forbidden"));
  render(<AdminPoliciesSettings />);
  expect(await screen.findByText("admin.error.load")).toBeInTheDocument();
  expect(document.querySelector(".policy-row")).toBeNull();
  expect(setPolicy).not.toHaveBeenCalled();
});

test("groups can still be created and edited for membership", async () => {
  render(<AdminGroupsSettings />);
  expect(await screen.findByText("engineering")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "admin.groups.members" }));
  expect((await screen.findByRole("dialog")).textContent).toContain("Bob");
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "admin.groups.addMember" })); });
  expect(updateMembers).toHaveBeenCalledWith("engineering", ["alice"], []);
  fireEvent.click(screen.getByRole("button", { name: "admin.groups.create" }));
  const name = screen.getByRole("textbox", { name: "admin.groups.name" });
  fireEvent.change(name, { target: { value: "staff" } });
  await act(async () => { fireEvent.submit(name.closest("form")!); });
  expect(createGroup).toHaveBeenCalledWith("staff");
});

test("groups can still be deleted", async () => {
  render(<AdminGroupsSettings />);
  await screen.findByText("engineering");
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "admin.groups.delete" })); });
  expect(deleteGroup).toHaveBeenCalledWith("engineering");
});
