import { useEffect, useState } from "react";
import { SettingsHeading } from "../components/SettingsHeading";
import { Button } from "../components/Button";
import { useI18n } from "../i18n";
import { useOsMode } from "../preferences/OsMode";
import type { OsMode } from "../preferences/client";

const MODES: readonly OsMode[] = ["guided", "advanced", "developer"];
const RANK: Record<OsMode, number> = { guided: 0, advanced: 1, developer: 2 };

/** Guided/advanced/developer switch. Raising the mode asks for the password. */
export function OsModeSettings() {
  const { t } = useI18n();
  const { mode, change, saving, error, ready } = useOsMode();
  const [selected, setSelected] = useState<OsMode>(mode);
  const [password, setPassword] = useState("");
  const [saved, setSaved] = useState(false);
  useEffect(() => { setSelected(mode); }, [mode]);

  const raising = RANK[selected] > RANK[mode];
  const apply = async () => {
    setSaved(false);
    const ok = await change("user", selected, raising ? password : undefined);
    if (ok) { setPassword(""); setSaved(true); }
  };

  return <section className="display-settings">
    <SettingsHeading title={t("osMode.title")} help={t("osMode.titleHelp")} back="/settings" />
    <h3 className="section-title">{t("osMode.current")}</h3>
    <div className="settings-group">
      {MODES.map((candidate) => <div className="setting" key={candidate}>
        <div className="copy"><strong>{t(`osMode.${candidate}`)}</strong><p>{t(`osMode.${candidate}Help`)}</p></div>
        <div className="visual"><input type="radio" name="os-mode" aria-label={t(`osMode.${candidate}`)} checked={selected === candidate} onChange={() => setSelected(candidate)} /></div>
      </div>)}
    </div>
    {raising ? <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("osMode.password")}</strong><p>{t("osMode.passwordHelp")}</p></div>
        <div className="visual"><input type="password" aria-label={t("osMode.password")} value={password} autoComplete="current-password" onChange={(event) => setPassword(event.target.value)} /></div>
      </div>
    </div> : null}
    <div className="settings-group">
      <div className="setting">
        <div className="copy">
          {saved ? <p role="status">{t("osMode.saved")}</p> : error ? <p role="alert">{t(`osMode.error.${error}`)}</p> : null}
        </div>
        <div className="visual"><Button disabled={!ready || saving || selected === mode || (raising && password.length === 0)} onClick={() => void apply()}>{t("osMode.apply")}</Button></div>
      </div>
    </div>
  </section>;
}
