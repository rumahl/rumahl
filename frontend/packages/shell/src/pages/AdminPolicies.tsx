import { useEffect, useMemo, useRef, useState } from "react";
import { RumahlInput, RumahlModal, RumahlSelect } from "@rumahl/ui/components";
import { SettingsHeading } from "../components/SettingsHeading";
import { LoadingStatus } from "../components/LoadingStatus";
import { Button } from "../components/Button";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/locales/en";
import {
  AdminRequestError,
  createAdminGroup,
  deleteAdminGroup,
  listAdminGroups,
  listAdminPolicies,
  listAdminUsers,
  setAdminPolicy,
  updateAdminGroupMembers,
  type AdminGroup,
  type AdminPolicyDefinition,
  type AdminUser,
} from "../admin/api";

const policyLabel: Record<string, MessageKey> = {
  "rumahl.os.advanced_mode": "admin.policies.advancedMode",
  "rumahl.home.cross_user_read": "admin.policies.crossUserRead",
  "rumahl.files.quota_bytes": "admin.policies.quotaBytes"
};

function useLoad<T>(load: () => Promise<T>) {
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState(false);
  const [revision, setRevision] = useState(0);
  // The loader is read through a ref so only the revision drives reloads.
  const loader = useRef(load);
  loader.current = load;
  useEffect(() => {
    let active = true;
    setError(false);
    void loader.current().then(
      (next) => { if (active) setValue(next); },
      () => { if (active) setError(true); }
    );
    return () => { active = false; };
  }, [revision]);
  return { value, error, reload: () => setRevision((current) => current + 1) };
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) return `${Math.round(bytes / (1024 * 1024 * 1024))} GiB`;
  return `${Math.round(bytes / (1024 * 1024))} MiB`;
}

/**
 * The policy surface: one row per catalog policy, one assignment editor per
 * device, group or user. Assignments are validated by the service; the UI only
 * offers what the catalog declares.
 */
export function AdminPoliciesSettings() {
  const { t } = useI18n();
  const policies = useLoad(() => Promise.all([listAdminPolicies(), listAdminUsers(), listAdminGroups()]));
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const loaded = policies.value;

  async function assign(subject: string, key: string, value: boolean | number | null) {
    setBusy(true);
    setError(null);
    try {
      await setAdminPolicy(subject, key, value);
      policies.reload();
    } catch (failure) {
      setError(failure instanceof AdminRequestError && failure.status === 400 ? "invalid" : "save");
    } finally {
      setBusy(false);
    }
  }

  return <section className="display-settings admin-settings policy-settings">
    <SettingsHeading title={t("admin.policies.title")} help={t("admin.policies.help")} back="/settings" />
    <div className="settings-group admin-list">
      {policies.error ? <p className="settings-footnote" role="alert">{t("admin.error.load")}</p> : loaded === null ? <LoadingStatus label={t("admin.loading")} /> : loaded[0].policies.map((definition) => <PolicyRow
        key={definition.key}
        definition={definition}
        users={loaded[1]}
        groups={loaded[2]}
        assignments={loaded[0].assignments}
        busy={busy}
        onAssign={assign}
      />)}
      {error ? <p className="settings-footnote" role="alert">{t(`admin.error.${error}` as MessageKey)}</p> : null}
    </div>
  </section>;
}

type Assign = (subject: string, key: string, value: boolean | number | null) => Promise<void>;

function PolicyRow({
  definition,
  users,
  groups,
  assignments,
  busy,
  onAssign
}: {
  definition: AdminPolicyDefinition;
  users: readonly AdminUser[];
  groups: readonly AdminGroup[];
  assignments: { subject: string; key: string; value: boolean | number }[];
  busy: boolean;
  onAssign: Assign;
}) {
  const { t } = useI18n();
  const [subject, setSubject] = useState("device");
  const mine = useMemo(
    () => assignments.filter((assignment) => assignment.key === definition.key),
    [assignments, definition.key]
  );
  const current = mine.find((assignment) => assignment.subject === subject);
  const options = [
    { value: "device", label: t("admin.policies.subjectDevice") },
    ...groups.map((group) => ({ value: `group:${group.id}`, label: t("admin.policies.subjectGroup", { name: group.name }) })),
    ...users.map((user) => ({ value: `user:${user.id}`, label: t("admin.policies.subjectUser", { name: user.username }) }))
  ];

  return <div className="setting policy-row">
    <div className="copy">
      <strong>{t(policyLabel[definition.key] ?? "admin.policies.title")}</strong>
      <p>{definition.description}</p>
      <small>{t("admin.policies.default")} {definition.kind === "number" ? formatBytes(definition.default as number) : definition.default ? t("admin.policies.on") : t("admin.policies.off")}</small>
    </div>
    <div className="visual policy-row__control">
      <RumahlSelect
        label={t("admin.policies.subjectLabel")}
        value={subject}
        disabled={busy}
        options={options}
        onChange={setSubject}
      />
      {definition.kind === "boolean" ? <Button
        size="sm"
        disabled={busy}
        variant={current?.value === true ? "primary" : "secondary"}
        onClick={() => void onAssign(subject, definition.key, current?.value === true ? null : true)}
      >{t("admin.policies.on")}</Button> : <RumahlInput
        type="number"
        aria-label={t("admin.policies.quotaBytes")}
        defaultValue={String(Math.round((current?.value as number ?? definition.default as number) / (1024 * 1024)))}
        disabled={busy}
        onBlur={(event) => {
          const mebibytes = Number(event.currentTarget.value);
          if (!Number.isFinite(mebibytes) || mebibytes <= 0) return;
          const bytes = Math.round(mebibytes) * 1024 * 1024;
          if (definition.minimum !== null && bytes < definition.minimum) return;
          if (definition.maximum !== null && bytes > definition.maximum) return;
          if (bytes === current?.value) return;
          void onAssign(subject, definition.key, bytes);
        }}
      />}
      {current ? <Button size="sm" variant="ghost" disabled={busy} onClick={() => void onAssign(subject, definition.key, null)}>{t("admin.policies.clear")}</Button> : null}
    </div>
  </div>;
}

/** Group management: create, delete and edit membership. */
export function AdminGroupsSettings() {
  const { t } = useI18n();
  const groups = useLoad(() => Promise.all([listAdminGroups(), listAdminUsers()]));
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<AdminGroup | null>(null);

  async function create() {
    setBusy(true);
    setError(null);
    try {
      await createAdminGroup(name.trim());
      setName("");
      setCreateOpen(false);
      groups.reload();
    } catch (failure) {
      setError(failure instanceof AdminRequestError && failure.status === 409 ? "conflict" : "save");
    } finally {
      setBusy(false);
    }
  }

  async function remove(group: AdminGroup) {
    setBusy(true);
    setError(null);
    try {
      await deleteAdminGroup(group.id);
      setEditing(null);
      groups.reload();
    } catch {
      setError("delete");
    } finally {
      setBusy(false);
    }
  }

  async function toggle(group: AdminGroup, user: AdminUser, member: boolean) {
    setBusy(true);
    setError(null);
    try {
      const updated = await updateAdminGroupMembers(
        group.id,
        member ? [] : [user.id],
        member ? [user.id] : []
      );
      setEditing(updated);
      groups.reload();
    } catch {
      setError("save");
    } finally {
      setBusy(false);
    }
  }

  const loaded = groups.value;
  const members = (group: AdminGroup) => new Set(group.members);

  return <section className="display-settings admin-settings group-settings">
    <SettingsHeading title={t("admin.groups.title")} help={t("admin.groups.help")} back="/settings" />
    <div className="settings-group admin-list">
      {groups.error ? <p className="settings-footnote" role="alert">{t("admin.error.load")}</p> : loaded === null ? <LoadingStatus label={t("admin.loading")} /> : <>
        {loaded[0].length === 0 ? <p>{t("admin.groups.empty")}</p> : loaded[0].map((group) => <div className="admin-session-row" key={group.id}>
          <div className="admin-session__identity">
            <strong>{group.name}</strong>
            <small>{t("admin.groups.memberCount", { count: group.members.length })}</small>
          </div>
          <Button size="sm" disabled={busy} onClick={() => setEditing(group)}>{t("admin.groups.members")}</Button>
          <Button size="sm" variant="danger" disabled={busy} onClick={() => void remove(group)}>{t("admin.groups.delete")}</Button>
        </div>)}
        <Button size="sm" onClick={() => setCreateOpen(true)}>{t("admin.groups.create")}</Button>
      </>}
      {error ? <p className="settings-footnote" role="alert">{t(`admin.error.${error}` as MessageKey)}</p> : null}
    </div>

    <RumahlModal open={createOpen} onClose={() => setCreateOpen(false)} title={t("admin.groups.createTitle")} closeLabel={t("admin.users.cancel")}>
      <form onSubmit={(event) => { event.preventDefault(); void create(); }}>
        <RumahlInput
          required
          autoFocus
          aria-label={t("admin.groups.name")}
          placeholder={t("admin.groups.name")}
          value={name}
          onChange={(event) => setName(event.currentTarget.value)}
        />
        <menu>
          <Button type="button" variant="secondary" size="sm" onClick={() => setCreateOpen(false)}>{t("admin.users.cancel")}</Button>
          <Button type="submit" size="sm" disabled={busy || !name.trim()}>{t("admin.groups.create")}</Button>
        </menu>
      </form>
    </RumahlModal>

    {editing && loaded !== null ? <RumahlModal open onClose={() => setEditing(null)} title={t("admin.groups.membersTitle", { name: editing.name })} closeLabel={t("admin.users.cancel")}>
      <ul className="group-settings__members">
        {loaded[1].map((user) => {
          const member = members(editing).has(user.id);
          return <li key={user.id}>
            <span>{user.displayName} <small>@{user.username}</small></span>
            <Button size="sm" disabled={busy} onClick={() => void toggle(editing, user, member)}>
              {member ? t("admin.groups.removeMember") : t("admin.groups.addMember")}
            </Button>
          </li>;
        })}
      </ul>
    </RumahlModal> : null}
  </section>;
}
