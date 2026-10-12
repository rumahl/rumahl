import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { closeHelp } from "./keyboard";

/** The hidden overview of rumahl OS keyboard commands (opened with `r` `?`). */
export function HelpOverlay() {
  const { t } = useI18n();
  const rows: [string[], string][] = [
    [["r", "?"], t("keyhelp.overview")],
    [["r", "e"], t("keyhelp.emergency")],
    [["r", "a"], t("keyhelp.activity")],
    [["r", "c"], t("keyhelp.center")],
    [["Ctrl", "Alt", "E"], t("keyhelp.emergency")]
  ];
  return createPortal(
    <div className="keyhelp" role="dialog" aria-modal="true" aria-label={t("keyhelp.title")} onClick={(event) => { if (event.target === event.currentTarget) closeHelp(); }}>
      <div className="keyhelp__card">
        <h2>{t("keyhelp.title")}</h2>
        <ul>
          {rows.map(([keys, label], index) => <li key={index}>
            <span className="keyhelp__keys">{keys.map((key) => <kbd key={key}>{key}</kbd>)}</span>
            <span className="keyhelp__label">{label}</span>
          </li>)}
        </ul>
        <button type="button" className="btn" onClick={closeHelp}>{t("emergency.close")}</button>
      </div>
    </div>,
    document.body
  );
}
