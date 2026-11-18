import { useEffect, useMemo, useState } from "react";
import type { BridgeFilesAccess, BridgeFilesFile } from "@rumahl/contracts/bridge";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { RumahlModal } from "../components/RumahlDialog";
import { Button } from "../components/Button";
import { LoadingStatus } from "../components/LoadingStatus";
import { issueAppGrant } from "./client";
import type { InstalledApp } from "./client";

/** One pending picker request from an installed app. */
export interface FilePickerRequest {
  access: BridgeFilesAccess;
  resolve: (result: { files: BridgeFilesFile[] } | null) => void;
}

interface Entry {
  id: string;
  name: string;
  directory: boolean;
  size: number;
}

const MAX_PICKS = 16;

/**
 * The consent surface for app file access. The app asks for read or write
 * access; the user — never the app — picks the personal files, and only the
 * picked ids ever reach the app. A write consent survives the session only
 * after re-authentication, exactly like the OS mode raise.
 */
export function AppFilePickerDialog({ app, request, onDone }: {
  app: InstalledApp;
  request: FilePickerRequest;
  onDone: (result: { files: BridgeFilesFile[] } | null) => void;
}) {
  const { live } = useShell();
  const { t } = useI18n();
  const [parent, setParent] = useState<{ id: string; name: string }>({ id: "root", name: "" });
  const [entries, setEntries] = useState<Entry[] | null>(null);
  const [listFailed, setListFailed] = useState(false);
  const [selected, setSelected] = useState<Map<string, BridgeFilesFile>>(new Map());
  const [validity, setValidity] = useState<"session" | "durable">("session");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [grantError, setGrantError] = useState(false);

  useEffect(() => {
    if (!live) return;
    const controller = new AbortController();
    setEntries(null);
    setListFailed(false);
    live.request(`/api/v1/files?parent=${encodeURIComponent(parent.id)}&limit=200&offset=0`, {
      credentials: "same-origin", cache: "no-store", signal: controller.signal,
      headers: { Accept: "application/json" }
    }).then(async (response) => {
      if (!response.ok) throw new Error("unavailable");
      const value: unknown = await response.json();
      if (!Array.isArray(value)) throw new Error("invalid");
      const items: Entry[] = [];
      for (const entry of value) {
        const record = entry as Record<string, unknown>;
        if (typeof record.id !== "string" || typeof record.name !== "string" ||
            typeof record.directory !== "boolean" || typeof record.size !== "number" ||
            record.name.length > 512 || items.length >= 500) throw new Error("invalid");
        items.push({ id: record.id, name: record.name, directory: record.directory, size: record.size });
      }
      items.sort((a, b) => a.directory === b.directory ? a.name.localeCompare(b.name) : a.directory ? -1 : 1);
      if (!controller.signal.aborted) setEntries(items);
    }).catch(() => {
      if (!controller.signal.aborted) { setEntries(null); setListFailed(true); }
    });
    return () => controller.abort();
  }, [live, parent]);

  // Entering a folder drops selections made elsewhere: the consent names the
  // files the user sees right now.
  useEffect(() => { setSelected(new Map()); }, [parent]);

  const needsPassword = request.access === "write" && validity === "durable";
  const files = useMemo(() => [...selected.values()], [selected]);

  const grant = async () => {
    if (!live || busy || files.length === 0) return;
    if (needsPassword && password.length === 0) return;
    setBusy(true);
    setGrantError(false);
    try {
      await issueAppGrant(live.request, app, {
        permission: request.access === "read" ? "rumahl.files.read" : "rumahl.files.write",
        resources: files.map((file) => file.id),
        validity,
        ...(needsPassword ? { password } : {})
      }, new AbortController().signal);
      onDone({ files });
    } catch {
      setGrantError(true);
      setBusy(false);
    }
  };

  return <RumahlModal
    open
    onClose={() => onDone(null)}
    title={t(request.access === "read" ? "filesBridge.pickRead" : "filesBridge.pickWrite")}
    application={app.title}
    closeLabel={t("filesBridge.close")}
    description={t("filesBridge.pickHelp", { title: app.title })}
    actions={<>
      <Button variant="ghost" type="button" onClick={() => onDone(null)}>{t("filesBridge.decline")}</Button>
      <Button
        variant="primary"
        type="button"
        disabled={busy || files.length === 0 || (needsPassword && password.length === 0)}
        onClick={() => void grant()}
      >{t("filesBridge.grant")}</Button>
    </>}
  >
    {parent.id !== "root"
      ? <p><button type="button" className="files-picker__up" onClick={() => setParent({ id: "root", name: "" })}>
        {t("filesBridge.backToFiles")}
      </button></p>
      : null}
    {listFailed
      ? <p role="alert">{t("filesBridge.unavailable")}</p>
      : !entries
        ? <LoadingStatus label={t("apps.loading")} />
        : entries.length === 0
          ? <p>{t("filesBridge.empty")}</p>
          : <ul className="files-picker__list">
              {entries.map((entry) => entry.directory
                ? <li key={entry.id}>
                    <button type="button" onClick={() => setParent({ id: entry.id, name: entry.name })}>
                      {entry.name}/
                    </button>
                  </li>
                : <li key={entry.id}>
                    <label>
                      <input
                        type="checkbox"
                        checked={selected.has(entry.id)}
                        onChange={(event) => {
                          const next = new Map(selected);
                          if (event.target.checked) {
                            if (next.size >= MAX_PICKS) return;
                            next.set(entry.id, { id: entry.id, name: entry.name, size: entry.size });
                          } else next.delete(entry.id);
                          setSelected(next);
                        }}
                        disabled={!selected.has(entry.id) && selected.size >= MAX_PICKS}
                      />
                      <span>{entry.name}</span>
                      <span className="files-picker__size">{entry.size}</span>
                    </label>
                  </li>)}
            </ul>}
    <fieldset className="files-picker__validity">
      <legend>{t("filesBridge.duration")}</legend>
      <label>
        <input type="radio" name="files-picker-validity" checked={validity === "session"} onChange={() => setValidity("session")} />
        {t("filesBridge.untilSession")}
      </label>
      <label>
        <input type="radio" name="files-picker-validity" checked={validity === "durable"} onChange={() => setValidity("durable")} />
        {t("filesBridge.untilRevoked")}
      </label>
      {needsPassword
        ? <input
            type="password"
            value={password}
            maxLength={128}
            placeholder={t("filesBridge.password")}
            aria-label={t("filesBridge.password")}
            onChange={(event) => setPassword(event.target.value)}
          />
        : null}
    </fieldset>
    {grantError ? <p role="alert">{t("filesBridge.grantFailed")}</p> : null}
  </RumahlModal>;
}
