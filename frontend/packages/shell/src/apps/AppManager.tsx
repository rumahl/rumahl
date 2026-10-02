import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { fetchStreamSessions, type StreamSession } from "../stream-client";

export function AppManager() {
  const { snapshot, live, open } = useShell();
  const { t } = useI18n();
  const [sessions, setSessions] = useState<readonly StreamSession[]>([]);
  const [unavailable, setUnavailable] = useState(false);
  useEffect(() => {
    if (!live?.request) return;
    let canceled = false;
    void fetchStreamSessions(live.request).then(
      (items) => { if (!canceled) { setSessions(items); setUnavailable(false); } },
      () => { if (!canceled) setUnavailable(true); }
    );
    return () => { canceled = true; };
  }, [snapshot.revision, live?.request]);
  return <div className="app-manager">
    <div>
      <p className="eyebrow">{t("appManager.count", { count: snapshot.systemStatus.installedAppCount })}</p>
      <h3>{t("appManager.ready")}</h3><p>{t("appManager.body")}</p>
      <h4>{t("stream.available")}</h4>
      {unavailable ? <p role="status">{t("stream.unavailable")}</p> : sessions.length === 0 ? <p>{t("stream.none")}</p> :
        <div className="stream-list">{sessions.map((session) =>
          <button key={session.id} onClick={() => open(`/streams/${session.id}?title=${encodeURIComponent(session.title)}`)} type="button">{session.title}</button>
        )}</div>}
    </div>
    <button type="button" disabled>{t("appManager.selectPackage")}</button>
  </div>;
}
