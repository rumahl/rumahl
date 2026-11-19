import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("../components/SettingsHeading", () => ({ SettingsHeading: ({ title }: { title: string }) => <h1>{title}</h1> }));
vi.mock("../components/LoadingStatus", () => ({ LoadingStatus: () => <p>loading</p> }));
vi.mock("../components/Button", () => ({ Button: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button type={props.type ?? "button"} disabled={props.disabled} onClick={props.onClick}>{props.children}</button> }));
vi.mock("@rumahl/ui/components", () => ({
  RumahlDialog: () => null,
  RumahlInput: (props: { value?: string; defaultValue?: string; "aria-label"?: string; placeholder?: string; onChange?: (event: { currentTarget: { value: string } }) => void; onBlur?: (event: { currentTarget: { value: string } }) => void; type?: string }) =>
    props.type === "number"
      ? <input type="number" aria-label={props["aria-label"]} defaultValue={props.defaultValue} onBlur={props.onBlur} />
      : <input aria-label={props["aria-label"]} placeholder={props.placeholder} value={props.value} onChange={props.onChange} />,
  RumahlModal: ({ open, children }: { open: boolean; children: React.ReactNode }) => open ? <div role="dialog">{children}</div> : null,
  RumahlSelect: ({ label, options, onChange }: { label?: string; options: { value: string; label: string }[]; onChange: (value: string) => void }) =>
    <select aria-label={label} onChange={(event) => onChange(event.currentTarget.value)}>
      {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
    </select>,
}));

import { AdminGroupsSettings, AdminPoliciesSettings } from "./AdminPolicies";

const policies = {
  policiesVersion: 1,
  policies: [
    { key: "rumahl.password.min_length", area: "password", kind: "number", default: 15, minimum: 6, maximum: 128, description: "Minimum password length." },
    { key: "rumahl.session.lifetime_seconds", area: "session", kind: "number", default: 43200, minimum: 300, maximum: 2592000, description: "Session lifetime." },
    { key: "rumahl.os.advanced_mode", area: "os", kind: "boolean", default: true, minimum: null, maximum: null, description: "Advanced exposure mode." },
    { key: "rumahl.home.cross_user_read", area: "home", kind: "boolean", default: false, minimum: null, maximum: null, description: "Administrators may read other homes." },
    { key: "rumahl.files.quota_bytes", area: "files", kind: "number", default: 134217728, minimum: 16777216, maximum: 68719476736, description: "Personal files budget." },
    { key: "rumahl.apps.allowed_publishers", area: "apps", kind: "list", default: [], minimum: null, maximum: 64, description: "Trusted publishers." }
  ],
  assignments: [
    { subject: "device", key: "rumahl.files.quota_bytes", value: 33554432 },
    { subject: "device", key: "rumahl.apps.allowed_publishers", value: ["com.rumahl"] }
  ]
};
const users = [
  { id: "01990000-0000-7000-8000-000000000001", username: "alice", displayName: "Alice", status: "active", role: "owner" },
  { id: "01990000-0000-7000-8000-000000000002", username: "bob", displayName: "Bob", status: "active", role: "user" }
];
const groups = [{ id: "01990000-0000-7000-8000-0000000000aa", name: "engineering", members: [users[1]!.id] }];

const setPolicy = vi.fn<(subject: string, key: string, value: boolean | number | string | readonly string[] | null) => Promise<void>>(async () => {});
const createGroup = vi.fn<(name: string) => Promise<(typeof groups)[number]>>(async () => groups[0]!);
const deleteGroup = vi.fn<(id: string) => Promise<void>>(async () => {});
const updateMembers = vi.fn<(id: string, add: readonly string[], remove: readonly string[]) => Promise<(typeof groups)[number]>>(async () => groups[0]!);

vi.mock("../admin/api", () => ({
  AdminRequestError: class extends Error {
    constructor(readonly status: number) { super("failed"); }
  },
  listAdminPolicies: vi.fn(async () => policies),
  listAdminUsers: vi.fn(async () => users),
  listAdminGroups: vi.fn(async () => groups),
  setAdminPolicy: (...args: [subject: string, key: string, value: boolean | number | null]) => setPolicy(...args),
  createAdminGroup: (...args: [name: string]) => createGroup(...args),
  deleteAdminGroup: (...args: [id: string]) => deleteGroup(...args),
  updateAdminGroupMembers: (...args: [id: string, add: readonly string[], remove: readonly string[]]) => updateMembers(...args),
}));

beforeEach(() => {
  setPolicy.mockClear();
  createGroup.mockClear();
  deleteGroup.mockClear();
  updateMembers.mockClear();
});

test("lists the catalog grouped by area with defaults and assignments", async () => {
  render(<AdminPoliciesSettings />);
  expect(await screen.findByText("admin.policies.advancedMode")).toBeInTheDocument();
  expect(screen.getByText("admin.policies.crossUserRead")).toBeInTheDocument();
  expect(screen.getByText("admin.policies.quotaBytes")).toBeInTheDocument();
  // Every area has its section heading.
  for (const area of ["password", "session", "os", "home", "files", "apps"]) {
    expect(screen.getByText(`admin.policies.area.${area}`)).toBeInTheDocument();
  }
  // The device quota assignment (32 MiB) shows as the input's value.
  const quota = screen.getByRole("spinbutton", { name: "admin.policies.quotaBytes" }) as HTMLInputElement;
  expect(quota.value).toBe("32");
  // The session lifetime is entered in minutes (12 hours = 720).
  const lifetime = screen.getByRole("spinbutton", { name: "admin.policies.sessionLifetime" }) as HTMLInputElement;
  expect(lifetime.value).toBe("720");
  // Each policy offers the device, groups and users as subjects.
  const subjects = screen.getAllByLabelText("admin.policies.subjectLabel");
  expect(subjects.length).toBe(6);
});

test("edits the publisher allowlist one entry per line", async () => {
  render(<AdminPoliciesSettings />);
  const field = await screen.findByRole("textbox", { name: "admin.policies.allowedPublishers" }) as HTMLTextAreaElement;
  expect(field.value).toBe("com.rumahl");
  await act(async () => { fireEvent.change(field, { target: { value: "com.rumahl\norg.example\n" } }); });
  await act(async () => { fireEvent.blur(field); });
  expect(setPolicy).toHaveBeenLastCalledWith(
    "device",
    "rumahl.apps.allowed_publishers",
    ["com.rumahl", "org.example"]
  );
});

test("filters the catalog by label and key", async () => {
  render(<AdminPoliciesSettings />);
  await screen.findByText("admin.policies.advancedMode");
  const filter = screen.getByRole("searchbox", { name: "admin.policies.filter" });
  await act(async () => { fireEvent.change(filter, { target: { value: "publisher" } }); });
  expect(screen.queryByText("admin.policies.advancedMode")).toBeNull();
  expect(screen.getByText("admin.policies.allowedPublishers")).toBeInTheDocument();
  await act(async () => { fireEvent.change(filter, { target: { value: "rumahl.password" } }); });
  expect(screen.getByText("admin.policies.passwordMinLength")).toBeInTheDocument();
  expect(screen.queryByText("admin.policies.allowedPublishers")).toBeNull();
});

test("assigns and clears a boolean policy through the service", async () => {
  render(<AdminPoliciesSettings />);
  await screen.findByText("admin.policies.advancedMode");
  const row = screen.getByText("admin.policies.advancedMode").closest(".policy-row")! as HTMLElement;
  const allow = within(row).getByRole("button", { name: "admin.policies.on" });
  await act(async () => { fireEvent.click(allow); });
  expect(setPolicy).toHaveBeenCalledWith("device", "rumahl.os.advanced_mode", true);
  // The clear button of the row that carries an assignment resets it.
  const quotaRow = screen.getByText("admin.policies.quotaBytes").closest(".policy-row")! as HTMLElement;
  const clear = within(quotaRow).getByRole("button", { name: "admin.policies.clear" });
  await act(async () => { fireEvent.click(clear); });
  expect(setPolicy).toHaveBeenLastCalledWith("device", "rumahl.files.quota_bytes", null);
});

test("edits the quota in mebibytes and stays inside the catalog bounds", async () => {
  render(<AdminPoliciesSettings />);
  await screen.findByText("admin.policies.quotaBytes");
  const quota = screen.getByRole("spinbutton", { name: "admin.policies.quotaBytes" }) as HTMLInputElement;
  await act(async () => { fireEvent.blur(quota, { target: { value: "64" } }); });
  expect(setPolicy).toHaveBeenLastCalledWith("device", "rumahl.files.quota_bytes", 64 * 1024 * 1024);
  // Below the minimum nothing is sent.
  await act(async () => { fireEvent.blur(quota, { target: { value: "1" } }); });
  expect(setPolicy).toHaveBeenLastCalledWith("device", "rumahl.files.quota_bytes", 64 * 1024 * 1024);
});

test("groups can be created, listed and edited for membership", async () => {
  render(<AdminGroupsSettings />);
  expect(await screen.findByText("engineering")).toBeInTheDocument();
  expect(screen.getByText("admin.groups.memberCount")).toBeInTheDocument();

  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "admin.groups.members" })); });
  const dialog = await screen.findByRole("dialog");
  // Bob is a member, Alice can be added.
  expect(dialog.textContent).toContain("Bob");
  const add = screen.getByRole("button", { name: "admin.groups.addMember" });
  await act(async () => { fireEvent.click(add); });
  expect(updateMembers).toHaveBeenCalledWith(groups[0]!.id, [users[0]!.id], []);

  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "admin.groups.create" })); });
  const name = screen.getByRole("textbox", { name: "admin.groups.name" });
  await act(async () => { fireEvent.change(name, { target: { value: "staff" } }); });
  await act(async () => { fireEvent.submit(name.closest("form")!); });
  expect(createGroup).toHaveBeenCalledWith("staff");
});

test("groups can be deleted", async () => {
  render(<AdminGroupsSettings />);
  expect(await screen.findByText("engineering"));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "admin.groups.delete" })); });
  await waitFor(() => expect(deleteGroup).toHaveBeenCalledWith(groups[0]!.id));
});
