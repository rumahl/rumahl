import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RumahlInput, RumahlModal } from "@rumahl/ui/components";
import { SearchIcon, UserIcon } from "../icons";
import { SettingsHeading } from "../components/SettingsHeading";
import { LoadingStatus } from "../components/LoadingStatus";
import { Button } from "../components/Button";
import { ShellLink } from "../routing/ShellLink";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/locales/en";
import {
  AdminRequestError, createAdminGroup, deleteAdminGroup, listAdminGroups, listAdminPolicies,
  listAdminUsers, setAdminPolicy, updateAdminGroupMembers,
  type AdminGroup, type AdminPolicies, type AdminPolicyDefinition, type AdminPolicyValue, type AdminUser,
} from "../admin/api";
import {
  devicePolicy, parsePolicyDraft, policyText, policyUnit, replaceAssignment, resolvePolicy, samePolicy,
} from "../admin/policy-editor";
import "./admin-policies.css";

const labels: Record<string, MessageKey> = {
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
  "rumahl.apps.allowed_publishers": "admin.policies.allowedPublishers",
};

type Scope = "device" | "group" | "user";

interface EditorData {
  catalog: AdminPolicies;
  users: AdminUser[];
  groups: AdminGroup[];
}

/**
 * The policy editor. The three targets — device, groups, accounts — are tabs,
 * so it is always visible whose rules are on screen; the selected group or
 * account sits right beside them as a chip row. Every rule is one row with
 * its control directly in it and applies immediately; a small source line
 * says where the current value comes from (set here, a group, the device or
 * the system default). Machine-wide rules (passwords, app budget, trusted
 * publishers) only appear on the device tab — never greyed out elsewhere.
 */
export function AdminPoliciesSettings() {
  const { t, locale } = useI18n();
  const [data, setData] = useState<EditorData | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [scope, setScope] = useState<Scope>("device");
  const [groupId, setGroupId] = useState<string | null>(null);
  const [userId, setUserId] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);

  const load = useCallback(async () => {
    const request = ++generation.current;
    setLoadError(false);
    try {
      const [catalog, users, groups] = await Promise.all([listAdminPolicies(), listAdminUsers(), listAdminGroups()]);
      if (request === generation.current) setData({ catalog, users, groups });
    } catch {
      if (request === generation.current) setLoadError(true);
    }
  }, []);
  useEffect(() => { void load(); return () => { generation.current++; }; }, [load]);

  // A newly opened tab starts at the first group or account, never in limbo.
  const selectedGroup = data?.groups.find((group) => group.id === groupId) ?? null;
  const selectedUser = data?.users.find((user) => user.id === userId) ?? null;
  useEffect(() => {
    if (!data) return;
    if (scope === "group" && !selectedGroup) setGroupId(data.groups[0]?.id ?? null);
    if (scope === "user" && !selectedUser) setUserId(data.users[0]?.id ?? null);
  }, [data, scope, selectedGroup, selectedUser]);

  const subject = scope === "device"
    ? "device"
    : scope === "group"
      ? (selectedGroup ? `group:${selectedGroup.id}` : null)
      : (selectedUser ? `user:${selectedUser.id}` : null);

  /** Applies one rule immediately; the catalog is updated from the reply. */
  const save = useCallback(async (key: string, value: AdminPolicyValue | null) => {
    if (busy || !subject) return;
    setBusy(true);
    setError(null);
    try {
      await setAdminPolicy(subject, key, value);
      setData((previous) => previous && subject ? {
        ...previous,
        catalog: { ...previous.catalog, assignments: replaceAssignment(previous.catalog.assignments, subject, key, value) },
      } : previous);
    } catch (failure) {
      setError(failure instanceof AdminRequestError && failure.status === 400 ? "invalid" : "save");
    } finally {
      setBusy(false);
    }
  }, [busy, subject]);

  // Machine-wide rules live on the device tab only; elsewhere they are not
  // shown greyed out but simply left out, with one note pointing at them.
  const machineWide = useMemo(
    () => (data?.catalog.policies ?? []).filter((definition) => devicePolicy(definition.key)),
    [data],
  );
  const areas = useMemo(() => {
    const grouped = new Map<string, AdminPolicyDefinition[]>();
    const needle = filter.trim().toLowerCase();
    for (const definition of data?.catalog.policies ?? []) {
      if (scope !== "device" && devicePolicy(definition.key)) continue;
      const label = labels[definition.key] ? t(labels[definition.key]!) : definition.key;
      const haystack = `${definition.key} ${label} ${definition.description}`.toLowerCase();
      if (needle && !haystack.includes(needle)) continue;
      const list = grouped.get(definition.area) ?? [];
      list.push(definition);
      grouped.set(definition.area, list);
    }
    return [...grouped];
  }, [data, filter, scope, t]);

  const sourceName = (source: string) => source === "default"
    ? t("admin.policies.sourceDefault")
    : (data?.groups ?? []).some((group) => `group:${group.id}` === source)
      ? t("admin.policies.subjectGroup", { name: (data?.groups ?? []).find((group) => `group:${group.id}` === source)!.name })
      : (data?.users ?? []).some((user) => `user:${user.id}` === source)
        ? t("admin.policies.subjectUser", { name: (data?.users ?? []).find((user) => `user:${user.id}` === source)!.displayName || (data?.users ?? []).find((user) => `user:${user.id}` === source)!.username })
        : t("admin.policies.subjectDevice");
  const formatValue = (definition: AdminPolicyDefinition, value: AdminPolicyValue): string => {
    if (definition.kind === "boolean") return t(value ? "admin.policies.on" : "admin.policies.off");
    if (definition.kind === "number") {
      const unit = policyUnit(definition);
      const shown = Number(value) / unit.factor;
      return `${shown.toLocaleString(locale, { maximumFractionDigits: 6 })}${unit.label ? ` ${unit.label}` : ""}`;
    }
    if (definition.kind === "list") {
      const items = value as readonly string[];
      return items.length ? items.join(", ") : t("admin.policies.none");
    }
    return String(value) || t("admin.policies.none");
  };

  const tabs: { id: Scope; label: string }[] = [
    { id: "device", label: t("admin.policies.subjectDevice") },
    { id: "group", label: t("admin.policies.tabGroups") },
    { id: "user", label: t("admin.policies.tabUsers") },
  ];

  return <section className="display-settings admin-settings policy-settings">
    <SettingsHeading title={t("admin.policies.title")} help={t("admin.policies.help")} back="/settings" />
    <div className="settings-group policy-settings__scope">
      <div className="policy-settings__tabs" role="tablist" aria-label={t("admin.policies.subjectLabel")}>
        {tabs.map((tab) => <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={scope === tab.id}
          className={`policy-settings__tab${scope === tab.id ? " is-selected" : ""}`}
          onClick={() => setScope(tab.id)}
        >{tab.label}</button>)}
      </div>
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
    </div>
    {scope === "group" ? <div className="policy-settings__chips" role="group" aria-label={t("admin.policies.chooseGroup")}>
      {data && data.groups.length === 0 ? <p className="policy-settings__chips-empty">
        {t("admin.groups.empty")} <ShellLink to="/settings/admin/groups">{t("admin.policies.manageGroups")}</ShellLink>
      </p> : (data?.groups ?? []).map((group) => <button
        key={group.id}
        type="button"
        className={`policy-chip${selectedGroup?.id === group.id ? " is-selected" : ""}`}
        aria-pressed={selectedGroup?.id === group.id}
        disabled={busy}
        onClick={() => setGroupId(group.id)}
      >{group.name}</button>)}
    </div> : null}
    {scope === "user" ? <div className="policy-settings__chips" role="group" aria-label={t("admin.policies.chooseUser")}>
      {(data?.users ?? []).map((user) => <button
        key={user.id}
        type="button"
        className={`policy-chip${selectedUser?.id === user.id ? " is-selected" : ""}`}
        aria-pressed={selectedUser?.id === user.id}
        disabled={busy}
        onClick={() => setUserId(user.id)}
      ><span className="policy-chip__avatar" aria-hidden="true">{(user.displayName || user.username).slice(0, 1).toUpperCase()}</span>{user.displayName || user.username}</button>)}
    </div> : null}
    {scope !== "device" && machineWide.length > 0 ? <p className="policy-settings__note">
      {t("admin.policies.deviceWideNote", { count: machineWide.length })}
      <button type="button" onClick={() => setScope("device")}>{t("admin.policies.openDeviceTab")}</button>
    </p> : null}
    {error ? <p className="settings-footnote" role="alert">{t(`admin.error.${error}` as MessageKey)}</p> : null}
    {scope !== "device" && subject === null ? <p className="settings-footnote">{t(scope === "group" ? "admin.groups.empty" : "admin.policies.noUsers")}</p> : null}
    <div className="policy-settings__catalog">
      {loadError ? <p className="settings-footnote" role="alert">{t("admin.error.load")}</p>
        : data === null ? <LoadingStatus label={t("admin.loading")} />
        : subject === null ? null
        : areas.length === 0 ? <p className="settings-footnote">{t("admin.policies.noMatches")}</p>
        : areas.map(([area, definitions]) => <div className="policy-settings__area" key={area}>
          <h3 className="policy-settings__area-title">{t(`admin.policies.area.${area}` as MessageKey)}</h3>
          <div className="settings-group policy-settings__list">
            {definitions.map((definition) => <PolicyRow
              key={definition.key}
              definition={definition}
              label={labels[definition.key] ? t(labels[definition.key]!) : definition.key}
              data={data}
              subject={subject}
              busy={busy}
              sourceName={sourceName}
              formatValue={formatValue}
              onSave={save}
            />)}
          </div>
        </div>)}
    </div>
  </section>;
}

interface RowProps {
  definition: AdminPolicyDefinition;
  label: string;
  data: EditorData;
  subject: string;
  busy: boolean;
  sourceName: (source: string) => string;
  formatValue: (definition: AdminPolicyDefinition, value: AdminPolicyValue) => string;
  onSave: (key: string, value: AdminPolicyValue | null) => Promise<void>;
}

function PolicyRow({ definition, label, data, subject, busy, sourceName, formatValue, onSave }: RowProps) {
  const { t } = useI18n();
  const unit = policyUnit(definition);
  const effective = resolvePolicy(definition, subject, data.groups, data.catalog.assignments);
  const assigned = data.catalog.assignments.find((item) => item.subject === subject && item.key === definition.key);
  const source = assigned ? t("admin.policies.assignedHere") : sourceName(effective.source);
  const value = assigned?.value ?? effective.value;
  // The key makes the inputs re-mount when the subject or assignment shape
  // changes, so the shown value always matches the selected target.
  const key = `${subject}:${definition.key}:${assigned ? "set" : "inherit"}`;

  /** Applies a text/number/list edit; nothing happens on invalid or unchanged input. */
  const commit = (text: string) => {
    const parsed = parsePolicyDraft(definition, { mode: "custom", text });
    if (!parsed.ok || samePolicy(parsed.value, assigned?.value ?? null)) return;
    void onSave(definition.key, parsed.value);
  };

  return <div className={`setting policy-row${definition.kind === "list" ? " policy-row--wide" : ""}`} data-policy-key={definition.key}>
    <div className="copy">
      <strong>{label}</strong>
      <p>{definition.description}</p>
      <small className="policy-row__source" data-source={assigned ? "own" : effective.source === "default" ? "default" : "inherited"}>
        {formatValue(definition, value)} · {source}
      </small>
    </div>
    <div className="visual policy-row__control">
      {definition.kind === "boolean" ? <>
        <input
          key={key}
          type="checkbox"
          role="switch"
          aria-label={label}
          checked={value === true}
          disabled={busy}
          onChange={(event) => void onSave(definition.key, event.currentTarget.checked)}
        />
        {assigned ? <Button size="sm" variant="ghost" disabled={busy} onClick={() => void onSave(definition.key, null)}>{t("admin.policies.clear")}</Button> : null}
      </> : null}
      {definition.kind === "number" ? <>
        <span className="policy-unit">
          <RumahlInput
            key={key}
            type="number"
            aria-label={label}
            defaultValue={policyText(definition, value)}
            min={definition.minimum !== null ? definition.minimum / unit.factor : undefined}
            max={definition.maximum !== null ? definition.maximum / unit.factor : undefined}
            disabled={busy}
            onBlur={(event) => commit(event.currentTarget.value)}
          />
          <span className="policy-unit__suffix" aria-hidden="true">{unit.label}</span>
        </span>
        {assigned ? <Button size="sm" variant="ghost" disabled={busy} onClick={() => void onSave(definition.key, null)}>{t("admin.policies.clear")}</Button> : null}
      </> : null}
      {definition.kind === "text" ? <>
        <RumahlInput
          key={key}
          type="text"
          aria-label={label}
          placeholder={String(definition.default)}
          defaultValue={policyText(definition, value)}
          disabled={busy}
          onBlur={(event) => commit(event.currentTarget.value)}
        />
        {assigned ? <Button size="sm" variant="ghost" disabled={busy} onClick={() => void onSave(definition.key, null)}>{t("admin.policies.clear")}</Button> : null}
      </> : null}
      {definition.kind === "list" ? <>
        <textarea
          key={key}
          className="rumahl-textarea policy-row__list"
          aria-label={label}
          placeholder={t("admin.policies.listHint")}
          defaultValue={policyText(definition, value)}
          disabled={busy}
          rows={6}
          onBlur={(event) => commit(event.currentTarget.value)}
        />
        {assigned ? <Button size="sm" variant="ghost" disabled={busy} onClick={() => void onSave(definition.key, null)}>{t("admin.policies.clear")}</Button> : null}
      </> : null}
    </div>
  </div>;
}

function useLoad<T>(load: () => Promise<T>) {
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState(false);
  const [revision, setRevision] = useState(0);
  const loader = useRef(load);
  loader.current = load;
  useEffect(() => {
    let active = true;
    setError(false);
    void loader.current().then(
      (next) => { if (active) setValue(next); },
      () => { if (active) setError(true); },
    );
    return () => { active = false; };
  }, [revision]);
  return { value, error, reload: () => setRevision((current) => current + 1) };
}

/** Group management: create groups and edit their membership. */
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
      const updated = await updateAdminGroupMembers(group.id, member ? [] : [user.id], member ? [user.id] : []);
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
    <div className="group-settings__toolbar">
      <Button size="sm" disabled={busy || loaded === null} onClick={() => setCreateOpen(true)}>{t("admin.groups.create")}</Button>
    </div>
    <div className="settings-group group-settings__list">
      {groups.error ? <p className="settings-footnote" role="alert">{t("admin.error.load")}</p> : loaded === null ? <LoadingStatus label={t("admin.loading")} /> : <>
        {loaded[0].length === 0 ? <p className="group-settings__empty">{t("admin.groups.empty")}</p> : loaded[0].map((group) => <div className="group-settings__row" key={group.id}>
          <span className="group-settings__icon" aria-hidden="true"><UserIcon /></span>
          <div className="group-settings__identity"><strong>{group.name}</strong><small>{t("admin.groups.memberCount", { count: group.members.length })}</small></div>
          <div className="group-settings__actions">
            <Button size="sm" disabled={busy} onClick={() => setEditing(group)}>{t("admin.groups.members")}</Button>
            <Button size="sm" variant="danger" disabled={busy} onClick={() => void remove(group)}>{t("admin.groups.delete")}</Button>
          </div>
        </div>)}
      </>}
      {error ? <p className="settings-footnote" role="alert">{t(`admin.error.${error}` as MessageKey)}</p> : null}
    </div>
    <RumahlModal open={createOpen} onClose={() => setCreateOpen(false)} title={t("admin.groups.createTitle")} closeLabel={t("admin.users.cancel")}>
      <form className="group-settings__form" onSubmit={(event) => { event.preventDefault(); void create(); }}>
        <label htmlFor="admin-group-name">{t("admin.groups.name")}</label>
        <RumahlInput id="admin-group-name" required autoFocus aria-label={t("admin.groups.name")} placeholder={t("admin.groups.name")} value={name} onChange={(event) => setName(event.currentTarget.value)} />
        <menu>
          <Button type="button" variant="secondary" size="sm" onClick={() => setCreateOpen(false)}>{t("admin.users.cancel")}</Button>
          <Button type="submit" size="sm" loading={busy} disabled={busy || !name.trim()}>{t("admin.groups.create")}</Button>
        </menu>
      </form>
    </RumahlModal>
    {editing && loaded !== null ? <RumahlModal open onClose={() => setEditing(null)} title={t("admin.groups.membersTitle", { name: editing.name })} closeLabel={t("admin.users.cancel")}>
      <ul className="group-settings__members">{loaded[1].map((user) => {
        const member = editing.members.includes(user.id);
        return <li key={user.id}>
          <span className="group-settings__member-identity"><strong>{user.displayName}</strong><small>@{user.username}</small></span>
          <Button size="sm" disabled={busy} onClick={() => void toggle(editing, user, member)}>{member ? t("admin.groups.removeMember") : t("admin.groups.addMember")}</Button>
        </li>;
      })}</ul>
    </RumahlModal> : null}
  </section>;
}
