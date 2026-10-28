import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { fetchStreamSessions, type StreamSession } from "../stream-client";
import { Button } from "../components/Button";
import { browserProfile } from "../preferences/storage";
import { importPackage, readPackageFiles } from "./client";

export function AppManager() {
  const { snapshot, live, open } = useShell();
  const { t } = useI18n();
  const [sessions, setSessions] = useState<readonly StreamSession[]>([]);
  const [unavailable, setUnavailable] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installed, setInstalled] = useState<string | null>(null);
  const [installError, setInstallError] = useState(false);
  const packageInput = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!live?.request) return;
    let canceled = false;
    void fetchStreamSessions(live.request).then(
      (items) => { if (!canceled) { setSessions(items); setUnavailable(false); } },
      () => { if (!canceled) setUnavailable(true); }
    );
    return () => { canceled = true; };
  }, [snapshot.revision, live?.request]);
  // A directory picker is required to read a signed directory package; the
  // attribute is set imperatively because it is non-standard in JSX.
  useEffect(() => { packageInput.current?.setAttribute("webkitdirectory", ""); }, []);
  async function onSelect(event: ChangeEvent<HTMLInputElement>) {
    const selected = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (!selected.length || !live?.request) return;
    setInstalling(true); setInstallError(false); setInstalled(null);
    const controller = new AbortController();
    try {
      const files = await readPackageFiles(selected);
      const app = await importPackage(live.request, files, browserProfile().id, controller.signal);
      setInstalled(app.title);
    } catch {
      setInstallError(true);
    } finally {
      setInstalling(false);
    }
  }
  return <div className="app-manager">
    <div>
      <p className="eyebrow">{t("appManager.count", { count: snapshot.systemStatus.installedAppCount })}</p>
      <h3>{t("appManager.ready")}</h3><p>{t("appManager.body")}</p>
      <h4>{t("stream.available")}</h4>
      {unavailable ? <p role="status">{t("stream.unavailable")}</p> : sessions.length === 0 ? <p>{t("stream.none")}</p> :
        <div className="stream-list">{sessions.map((session) =>
          <button key={session.id} onClick={() => open(`/streams/${session.id}?title=${encodeURIComponent(session.title)}`)} type="button">{session.title}</button>
        )}</div>}
      <h4>{t("appManager.installTitle")}</h4>
      <p>{t("appManager.installHelp")}</p>
      {installed ? <p role="status">{t("appManager.installDone", { title: installed })}</p> : null}
      {installError ? <p role="alert">{t("appManager.installFailed")}</p> : null}
      <input ref={packageInput} type="file" hidden multiple onChange={onSelect} />
    </div>
    <Button onClick={() => packageInput.current?.click()} disabled={installing || !live}>{installing ? t("appManager.installing") : t("appManager.selectPackage")}</Button>
  </div>;
}
