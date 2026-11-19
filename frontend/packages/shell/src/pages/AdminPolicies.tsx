import { useEffect, useMemo, useRef, useState } from "react";
import { RumahlInput, RumahlModal, RumahlSelect } from "@rumahl/ui/components";
import { SearchIcon } from "../icons";
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
  type AdminPolicyValue,
  type AdminUser,
} from "../admin/api";

/** One label per catalog key; the description comes from the server. */
const policyLabel: Record<string, MessageKey> = {
  "rumahl.password.min_length": "admin.policies.passwordMinLength",
  "rumahl.password.require_classes": "admin.policies.passwordRequireClasses",
  "rumahl.password.attempt_limit": "admin.policies.passwordAttemptLimit",
  "rumahl.password.lockout_seconds": "admin.policies.passwordLockoutSeconds",
  "rumahl.session.lifetime_seconds": "admin.policies.sessionLifetime",
  "rumahl.os.advanced_mode": "admin.policies.advancedMode",
  "rumahl.os.allow_store_install": "admin.policies.storeInstall",
  "rumahl.os.allow_app_uninstall": "admin.policies.appUninstall",
  "rumahl.home.cross_user_read": "admin.policies.crossUserRead",
  "rumahl.files.quota_bytes": "admin.policies.quotaBytes",
  "rumahl.files.max_file_bytes": "admin.policies.maxFileBytes",
  "rumahl.files.durable_grants": "admin.policies.durableGrants",
  "rumahl.apps.storage_quota_bytes": "admin.policies.appQuotaBytes",
  "rumahl.apps.allowed_publishers": "admin.policies.allowedPublishers"
};

type Unit = "mib" | "minutes" | "seconds";

/** How a number policy is presented: bytes as MiB, long durations as minutes. */
function unitOf(definition: AdminPolicyDefinition): Unit | null {
  if (definition.kind !== "number") return null;
  if (definition.key.endsWith("_bytes")) return "mib";
  if (definition.key === "rumahl.session.lifetime_seconds") return "minutes";
  if (definition.key.endsWith("_seconds")) return "seconds";
  return null;
}

function formatDefault(definition: AdminPolicyDefinition): string {
  const unit = unitOf(definition);
  if (unit === "mib") return `${Math.round(Number(definition.default) / (1024 * 1024))} MiB`;
  if (unit === "minutes") return `${Math.round(Number(definition.default) / 60)} min`;
  if (unit === "seconds") return `${definition.default} s`;
  if (definition.kind === "list") {
    const values = definition.default as readonly string[];
    return values.length === 0 ? "—" : values.join(", ");
  }
  return String(definition.default);
}

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

/**
 * The policy surface, grouped by area like a group policy editor: one section
 * per area, one row per catalog policy, one assignment editor per device,
 * group or user. Assignments are validated by the service; the UI only offers
 * what the catalog declares.
 */
export function AdminPoliciesSettings() {
  const { t } = useI18n();
  const policies = useLoad(() => Promise.all([listAdminPolicies(), listAdminUsers(), listAdminGroups()]));
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState("");

  const loaded = policies.value;
  const grouped = useMemo(() => {
    if (!loaded) return [];
    const needle = filter.trim().toLowerCase();
    const areas = new Map<string, AdminPolicyDefinition[]>();
    for (const definition of loaded[0].policies) {
      const label = t(policyLabel[definition.key] ?? "admin.policies.title").toLowerCase();
      const haystack = `${definition.key} ${label} ${definition.description}`.toLowerCase();
      if (needle && !haystack.includes(needle)) continue;
      const list = areas.get(definition.area) ?? [];
      list.push(definition);
      areas.set(definition.area, list);
    }
    return [...areas.entries()];
  }, [loaded, filter, t]);

  async function assign(subject: string, key: string, value: AdminPolicyValue | null) {
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
      <div className="policy-settings__filter">
        <SearchIcon />
        <input
          type="search"
          aria-label={t("admin.policies.filter")}
          placeholder={t("admin.policies.filter")}
          value={filter}
          onChange={(event) => setFilter(event.currentTarget.value)}
        />
      </div>
      {policies.error ? <p className="settings-footnote" role="alert">{t("admin.error.load")}</p> : loaded === null ? <LoadingStatus label={t("admin.loading")} /> : grouped.length === 0
        ? <p>{t("admin.policies.noMatches")}</p>
        : grouped.map(([area, definitions]) => <div className="policy-settings__area" key={area}>
          <h3 className="menubar__section policy-settings__area-title">{t(`admin.policies.area.${area}` as MessageKey)}</h3>
          {definitions.map((definition) => <PolicyRow
            key={definition.key}
            definition={definition}
            users={loaded[1]}
            groups={loaded[2]}
            assignments={loaded[0].assignments}
            busy={busy}
            onAssign={assign}
          />)}
        </div>)}
      {error ? <p className="settings-footnote" role="alert">{t(`admin.error.${error}` as MessageKey)}</p> : null}
    </div>
  </section>;
}

type Assign = (subject: string, key: string, value: AdminPolicyValue | null) => Promise<void>;

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
  assignments: { subject: string; key: string; value: AdminPolicyValue }[];
  busy: boolean;
  onAssign: Assign;
}) {
  const { t } = useI18n();
  const [subject, setSubject] = useState("device");
  const [text, setText] = useState("");
  const mine = useMemo(
    () => assignments.filter((assignment) => assignment.key === definition.key),
    [assignments, definition.key]
  );
  const current = mine.find((assignment) => assignment.subject === subject);
  const currentText = useMemo(() => {
    const value = current?.value;
    if (definition.kind === "list") return (value as readonly string[] | undefined)?.join("\n") ?? "";
    if (definition.kind === "text") return (value as string | undefined) ?? "";
    return "";
  }, [current, definition.kind]);
  useEffect(() => { setText(currentText); }, [currentText]);
  const options = [
    { value: "device", label: t("admin.policies.subjectDevice") },
    ...groups.map((group) => ({ value: `group:${group.id}`, label: t("admin.policies.subjectGroup", { name: group.name }) })),
    ...users.map((user) => ({ value: `user:${user.id}`, label: t("admin.policies.subjectUser", { name: user.username }) }))
  ];
  const unit = unitOf(definition);

  const numberInput = () => {
    const value = Number(current?.value ?? definition.default);
    const shown = unit === "mib"
      ? Math.round(value / (1024 * 1024))
      : unit === "minutes"
        ? Math.round(value / 60)
        : value;
    const toBytes = (entered: number) =>
      unit === "mib" ? entered * 1024 * 1024 : unit === "minutes" ? entered * 60 : entered;
    return <RumahlInput
      type="number"
      aria-label={t(policyLabel[definition.key] ?? "admin.policies.title")}
      defaultValue={String(shown)}
      disabled={busy}
      onBlur={(event) => {
        const entered = Number(event.currentTarget.value);
        if (!Number.isFinite(entered) || entered <= 0) return;
        const value = toBytes(Math.round(entered));
        if (definition.minimum !== null && value < definition.minimum) return;
        if (definition.maximum !== null && value > definition.maximum) return;
        if (value === current?.value) return;
        void onAssign(subject, definition.key, value);
      }}
    />;
  };

  return <div className="setting policy-row">
    <div className="copy">
      <strong>{t(policyLabel[definition.key] ?? "admin.policies.title")}</strong>
      <p>{definition.description}</p>
      <small>{t("admin.policies.default")} {formatDefault(definition)}</small>
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
      >{t("admin.policies.on")}</Button> : null}
      {definition.kind === "number" ? numberInput() : null}
      {definition.kind === "text" ? <RumahlInput
        type="text"
        aria-label={t(policyLabel[definition.key] ?? "admin.policies.title")}
        placeholder={String(definition.default)}
        value={text}
        disabled={busy}
        onChange={(event) => setText(event.currentTarget.value)}
        onBlur={() => {
          const trimmed = text.trim();
          const next = trimmed === "" ? null : trimmed;
          if (next === (current?.value ?? null)) return;
          void onAssign(subject, definition.key, next);
        }}
      /> : null}
      {definition.kind === "list" ? <textarea
        className="policy-row__list"
        aria-label={t(policyLabel[definition.key] ?? "admin.policies.title")}
        placeholder={t("admin.policies.listHint")}
        value={text}
        disabled={busy}
        rows={Math.max(2, Math.min(6, (current?.value as readonly string[] | undefined)?.length ?? 2))}
        onChange={(event) => setText(event.currentTarget.value)}
        onBlur={() => {
          const next = text.split("\n").map((line) => line.trim()).filter((line) => line.length > 0);
          const previous = (current?.value as readonly string[] | undefined) ?? [];
          if (next.length === previous.length && next.every((line, index) => line === previous[index])) return;
          void onAssign(subject, definition.key, next);
        }}
      /> : null}
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
          const member = editing.members.includes(user.id);
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
