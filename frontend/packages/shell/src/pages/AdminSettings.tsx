import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import type { ShellUserRole } from "@rumahl/contracts";
import { RumahlDialog, RumahlInput, RumahlModal, RumahlSelect } from "@rumahl/ui/components";
import { SettingsHeading } from "../components/SettingsHeading";
import { LoadingStatus } from "../components/LoadingStatus";
import { Button } from "../components/Button";
import { ShellLink } from "../routing/ShellLink";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/locales/en";
import {
  AdminRequestError,
  createAdminUser,
  deleteAdminUser,
  listAdminAudit,
  listAdminSessions,
  listAdminUsers,
  resetAdminPassword,
  revokeAdminSession,
  updateAdminUser,
  type AdminAuditEntry,
  type AdminSession,
  type AdminUser,
  type AdminUserStatus,
} from "../admin/api";

const assignableRoles: ShellUserRole[] = ["user", "manager", "administrator"];
const roleDescription: Record<ShellUserRole, MessageKey> = {
  user: "admin.role.user.description",
  manager: "admin.role.manager.description",
  administrator: "admin.role.administrator.description",
  owner: "admin.role.owner.description",
};

export function isAdministrator(role?: ShellUserRole): boolean {
  return role === "administrator" || role === "owner";
}

function useLoad<T>(load: () => Promise<T>) {
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const loader = useRef(load);
  loader.current = load;
  useEffect(() => {
    let active = true;
    setError(null);
    void loader.current().then(
      (next) => { if (active) setValue(next); },
      () => { if (active) setError("load"); }
    );
    return () => { active = false; };
  }, [revision]);
  return { value, error, reload: () => setRevision((current) => current + 1), setValue };
}

function useLabels() {
  const { t, locale } = useI18n();
  return {
    t,
    role: (role: ShellUserRole) => t(`admin.role.${role}`),
    status: (status: AdminUserStatus) => t(`admin.status.${status}`),
    time: (seconds: number) => new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(seconds * 1000)),
  };
}

function StatusPill({ status }: { status: AdminUserStatus }) {
  const { status: label } = useLabels();
  return <span className={`admin-status admin-status--${status}`}>{label(status)}</span>;
}

function RequestError({ error }: { error: string | null }) {
  const { t } = useI18n();
  return error ? <p className="settings-footnote" role="alert">{t(`admin.error.${error}` as MessageKey)}</p> : null;
}

function suggestedUsername(name: string): string {
  return name
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^[-._]+|[-._]+$/g, "")
    .slice(0, 32);
}

function AddUserModal({ open, onClose, onCreated }: { open: boolean; onClose: () => void; onCreated: (user: AdminUser) => void }) {
  const { t } = useI18n();
  const [displayName, setDisplayName] = useState("");
  const [customUsername, setCustomUsername] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [administrator, setAdministrator] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const username = customUsername ?? suggestedUsername(displayName);

  useEffect(() => {
    if (open) return;
    setDisplayName("");
    setCustomUsername(null);
    setPassword("");
    setAdministrator(false);
    setError(null);
  }, [open]);

  async function create(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      onCreated(await createAdminUser({
        username: username.trim() || "user",
        displayName,
        password,
        role: administrator ? "administrator" : "user"
      }));
      setDisplayName("");
      setCustomUsername(null);
      setPassword("");
      setAdministrator(false);
    } catch (failure) {
      setError(failure instanceof AdminRequestError && failure.status === 409 ? "conflict" : "create");
    } finally {
      setBusy(false);
    }
  }

  return <RumahlModal
    open={open}
    onClose={onClose}
    title={t("admin.users.add")}
    description={t("admin.users.addHelp")}
    application="rumahl OS"
    className="user-modal"
    closeLabel={t("admin.users.cancel")}
    actions={<>
      <Button onClick={onClose}>{t("admin.users.cancel")}</Button>
      <Button variant="primary" type="submit" form="admin-add-user" disabled={busy}>{t("admin.users.create")}</Button>
    </>}
  >
    <form id="admin-add-user" className="user-modal-form" onSubmit={(event) => void create(event)}>
      <RumahlInput required maxLength={128} data-autofocus aria-label={t("admin.users.fullName")} placeholder={t("admin.users.fullName")} value={displayName} onChange={(event) => setDisplayName(event.currentTarget.value)} />
      <RumahlInput required type="password" autoComplete="new-password" aria-label={t("admin.users.temporaryPassword")} placeholder={t("admin.users.temporaryPassword")} value={password} onChange={(event) => setPassword(event.currentTarget.value)} />
      <label className="user-modal-toggle">
        <span>
          <strong>{t("admin.users.adminToggle")}</strong>
          <small>{t("admin.users.adminToggleHelp")}</small>
        </span>
        <input type="checkbox" role="switch" checked={administrator} onChange={(event) => setAdministrator(event.currentTarget.checked)} />
      </label>
      <details className="user-modal-username">
        <summary>{t("admin.users.changeUsername")}</summary>
        <RumahlInput maxLength={128} aria-label={t("admin.username")} placeholder={t("admin.username")} value={username} onChange={(event) => setCustomUsername(event.currentTarget.value)} />
        <small>{t("admin.users.usernameHelp")}</small>
      </details>
      <span className="user-modal-hint">@{username || t("admin.username")}</span>
      <RequestError error={error} />
    </form>
  </RumahlModal>;
}

function ResetPasswordModal({ user, open, onClose }: { user: AdminUser; open: boolean; onClose: () => void }) {
  const { t } = useI18n();
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) return;
    setPassword("");
    setError(null);
  }, [open]);

  async function reset() {
    if (!password) return;
    setBusy(true);
    setError(null);
    try {
      await resetAdminPassword(user.id, password);
      setPassword("");
      onClose();
    } catch {
      setError("password");
    } finally {
      setBusy(false);
    }
  }

  return <RumahlModal
    open={open}
    onClose={onClose}
    title={t("admin.resetPassword")}
    description={t("admin.users.resetPasswordHelp", { name: user.displayName })}
    application="rumahl OS"
    className="user-modal"
    closeLabel={t("admin.users.cancel")}
    actions={<>
      <Button onClick={onClose}>{t("admin.users.cancel")}</Button>
      <Button variant="primary" disabled={busy || password.length === 0} onClick={() => void reset()}>{t("admin.setPassword")}</Button>
    </>}
  >
    <form className="user-modal-form" onSubmit={(event) => { event.preventDefault(); void reset(); }}>
      <RumahlInput required type="password" autoComplete="new-password" data-autofocus aria-label={t("admin.newPassword", { name: user.displayName })} placeholder={t("admin.users.temporaryPassword")} value={password} onChange={(event) => setPassword(event.currentTarget.value)} />
      <RequestError error={error} />
    </form>
  </RumahlModal>;
}

export function AdminUsersSettings() {
  const { t } = useI18n();
  const users = useLoad(listAdminUsers);
  const [adding, setAdding] = useState(false);

  function created(next: AdminUser) {
    users.setValue((current) => [...(current ?? []), next].sort((left, right) => left.username.localeCompare(right.username)));
    setAdding(false);
  }

  return <section className="display-settings admin-settings user-settings">
    <SettingsHeading title={t("admin.users.title")} help={t("admin.users.help")} back="/settings" />
    <div className="user-settings__bar">
      <div>
        <h3 className="section-title">{t("admin.users.list")}</h3>
        <p className="user-settings__lead">{t("admin.users.listHelp")}</p>
      </div>
      <Button variant="primary" onClick={() => setAdding(true)}>{t("admin.users.add")}</Button>
    </div>

    <div className="user-list">
      {users.error ? <RequestError error={users.error} /> : users.value === null ? <LoadingStatus className="loading-status--inline user-settings__lead" label={t("admin.loading")} /> : users.value.length === 0 ? <p className="user-settings__lead">{t("admin.users.empty")}</p> : users.value.map((user) => <ShellLink className="user-row" key={user.id} to={`/settings/admin/users/${user.id}`}>
        <span className={`user-avatar user-avatar--${user.role}`} aria-hidden="true">{user.displayName.slice(0, 1).toUpperCase()}</span>
        <span className="user-row__identity">
          <strong>{user.displayName}</strong>
          <small>@{user.username} · {t(`admin.role.${user.role}`)} · {t("admin.users.localAccount")}</small>
        </span>
        {user.status !== "active" ? <StatusPill status={user.status} /> : null}
        <span className="user-row__chevron" aria-hidden="true">›</span>
      </ShellLink>)}
    </div>

    <AddUserModal open={adding} onClose={() => setAdding(false)} onCreated={created} />
  </section>;
}

export function AdminUserDetailSettings() {
  const { t } = useI18n();
  const { id } = useParams();
  const navigate = useNavigate();
  const users = useLoad(listAdminUsers);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [resetOpen, setResetOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [nameDraft, setNameDraft] = useState("");
  const [usernameDraft, setUsernameDraft] = useState("");
  const user = users.value?.find((item) => item.id === id);

  useEffect(() => {
    if (!user) return;
    setNameDraft(user.displayName);
    setUsernameDraft(user.username);
  }, [user?.id]);

  function replace(next: AdminUser) {
    users.setValue((current) => current?.map((item) => item.id === next.id ? next : item) ?? null);
  }

  async function update(input: { status?: AdminUserStatus; role?: ShellUserRole; username?: string; displayName?: string }) {
    if (!user) return;
    setBusy(true);
    setError(null);
    try {
      replace(await updateAdminUser(user.id, input));
    } catch (failure) {
      setError(failure instanceof AdminRequestError && failure.status === 409 ? "conflict" : "update");
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!user) return;
    setBusy(true);
    setError(null);
    try {
      await deleteAdminUser(user.id);
      setDeleteOpen(false);
      navigate("/settings/admin/users");
    } catch (failure) {
      setError(failure instanceof AdminRequestError && failure.status === 409 ? "conflict" : "delete");
      setDeleteOpen(false);
    } finally {
      setBusy(false);
    }
  }

  return <section className="display-settings admin-settings user-settings">
    <SettingsHeading title={user?.displayName ?? t("admin.users.detailTitle")} help={t("admin.users.detailHelp")} back="/settings/admin/users" />
    {users.error ? <RequestError error={users.error} /> : users.value === null ? <LoadingStatus className="loading-status--inline user-settings__lead" label={t("admin.loading")} /> : !user ? <p className="settings-footnote" role="alert">{t("admin.users.notFound")}</p> : <>
      <div className="user-detail__hero">
        <span className={`user-detail__avatar user-avatar--${user.role}`} aria-hidden="true">{user.displayName.slice(0, 1).toUpperCase()}</span>
        <div>
          <h3>{user.displayName}</h3>
          <p>@{user.username} · {t("admin.users.localAccount")}</p>
        </div>
        {user.status !== "active" ? <StatusPill status={user.status} /> : null}
      </div>

      <div className="settings-group user-detail">
        <div className="setting">
          <div className="copy">
            <strong>{t("admin.users.profile")}</strong>
            <p>{t("admin.users.profileHelp")}</p>
          </div>
          <div className="visual user-detail__profile">
            <RumahlInput required aria-label={t("admin.users.fullName")} placeholder={t("admin.users.fullName")} value={nameDraft} onChange={(event) => setNameDraft(event.currentTarget.value)} />
            <RumahlInput required aria-label={t("admin.username")} placeholder={t("admin.username")} value={usernameDraft} onChange={(event) => setUsernameDraft(event.currentTarget.value)} />
            <Button
              size="sm"
              disabled={busy || !nameDraft.trim() || !usernameDraft.trim() || (nameDraft === user.displayName && usernameDraft === user.username)}
              onClick={() => void update({ displayName: nameDraft, username: usernameDraft })}
            >{t("admin.users.saveChanges")}</Button>
          </div>
        </div>

        <div className="setting">
          <div className="copy">
            <strong>{t("admin.accountType")}</strong>
            <p>{t("admin.accountTypeHelp")}</p>
          </div>
          <div className="visual user-detail__control">
            {user.role === "owner" ? <span className="user-detail__static">{t("admin.role.owner")}</span> : <RumahlSelect
              label={t("admin.roleLabel", { name: user.displayName })}
              value={user.role}
              disabled={busy}
              options={assignableRoles.map((value) => ({ value, label: t(`admin.role.${value}`) }))}
              onChange={(value) => void update({ role: value as ShellUserRole })}
            />}
            <small>{t(roleDescription[user.role])}</small>
          </div>
        </div>

        <div className="setting">
          <div className="copy">
            <strong>{t("admin.users.accountEnabled")}</strong>
            <p>{t("admin.accessHelp")}</p>
          </div>
          <div className="visual">
            <input
              type="checkbox"
              role="switch"
              aria-label={t("admin.statusLabel", { name: user.displayName })}
              checked={user.status === "active"}
              disabled={busy}
              onChange={(event) => void update({ status: event.currentTarget.checked ? "active" : "disabled" })}
            />
          </div>
        </div>

        <div className="setting">
          <div className="copy">
            <strong>{t("admin.users.password")}</strong>
            <p>{t("admin.users.resetPasswordHelp", { name: user.displayName })}</p>
          </div>
          <div className="visual">
            <Button size="sm" onClick={() => setResetOpen(true)}>{t("admin.resetPassword")}</Button>
          </div>
        </div>

        <div className="setting">
          <div className="copy">
            <strong>{t("admin.sessions.title")}</strong>
            <p>{t("admin.users.sessionsHelp")}</p>
          </div>
          <div className="visual">
            <ShellLink className="btn btn--secondary btn--sm" to="/settings/admin/sessions">{t("admin.users.manageSessions")}</ShellLink>
          </div>
        </div>

        {user.role !== "owner" ? <div className="setting">
          <div className="copy">
            <strong>{t("admin.users.delete")}</strong>
            <p>{t("admin.users.deleteHelp")}</p>
          </div>
          <div className="visual">
            <Button size="sm" variant="danger" onClick={() => setDeleteOpen(true)}>{t("admin.users.delete")}</Button>
          </div>
        </div> : null}
      </div>
      <RequestError error={error} />
      <ResetPasswordModal user={user} open={resetOpen} onClose={() => setResetOpen(false)} />
      <RumahlDialog
        open={deleteOpen}
        onClose={() => setDeleteOpen(false)}
        onConfirm={() => void remove()}
        title={t("admin.users.deleteTitle")}
        description={t("admin.users.deleteConfirm", { name: user.displayName })}
        confirmLabel={t("admin.users.delete")}
        cancelLabel={t("admin.users.cancel")}
        application="rumahl OS"
        destructive
      />
    </>}
  </section>;
}

export function AdminSessionsSettings() {
  const { t, time } = useLabels();
  const sessions = useLoad(listAdminSessions);
  const [error, setError] = useState<string | null>(null);

  async function revoke(session: AdminSession) {
    setError(null);
    try {
      await revokeAdminSession(session.sessionId);
      sessions.reload();
    } catch {
      setError("revoke");
    }
  }

  return <section className="display-settings admin-settings">
    <SettingsHeading title={t("admin.sessions.title")} help={t("admin.sessions.help")} back="/settings" />
    <div className="settings-group admin-list">
      {sessions.error ? <RequestError error={sessions.error} /> : sessions.value === null ? <LoadingStatus label={t("admin.loading")} /> : sessions.value.length === 0 ? <p>{t("admin.sessions.empty")}</p> : sessions.value.map((session) => <div className="admin-session-row" key={session.sessionId}>
        <div className="admin-session__identity">
          <strong>{session.displayName}</strong>
          <small>@{session.username} · {session.sessionId}</small>
        </div>
        <div className="admin-session__times">
          <span>{t("admin.sessions.lastSeen")}: {time(session.lastSeenAt)}</span>
          <span>{t("admin.sessions.expires")}: {time(session.expiresAt)}</span>
        </div>
        <span className={`admin-status admin-status--${session.active ? "active" : "revoked"}`}>{session.active ? t("admin.sessions.active") : t("admin.sessions.revoked")}</span>
        <Button size="sm" variant="danger" disabled={!session.active} onClick={() => void revoke(session)}>{t("admin.sessions.revoke")}</Button>
      </div>)}
      <RequestError error={error} />
    </div>
  </section>;
}

export function AdminAuditSettings() {
  const { t, time } = useLabels();
  const audit = useLoad(() => listAdminAudit(100, 0));
  return <section className="display-settings admin-settings">
    <SettingsHeading title={t("admin.audit.title")} help={t("admin.audit.help")} back="/settings" />
    <div className="admin-audit__actions"><Button size="sm" onClick={audit.reload}>{t("admin.refresh")}</Button></div>
    <div className="settings-group admin-list">
      {audit.error ? <RequestError error={audit.error} /> : audit.value === null ? <LoadingStatus label={t("admin.loading")} /> : audit.value.length === 0 ? <p>{t("admin.audit.empty")}</p> : audit.value.map((event) => <AuditRow key={event.sequence} event={event} time={time} />)}
    </div>
  </section>;
}

function AuditRow({ event, time }: { event: AdminAuditEntry; time: (seconds: number) => string }) {
  const { t } = useLabels();
  return <div className="admin-audit-row">
    <span className={`admin-status admin-status--${event.outcome}`}>{t(`admin.outcome.${event.outcome}`)}</span>
    <div><strong>{event.action}</strong><small>{event.actor}{event.target ? ` · ${event.target}` : ""}</small></div>
    <time>{time(event.at)}</time>
  </div>;
}
