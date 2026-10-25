import { useEffect, useState, type ReactNode } from "react";
import { useTheme } from "@rumahl/ui";
import { BoltIcon, MoonIcon, PulseIcon, SettingsIcon, SparkIcon, WindowStackIcon } from "../icons";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/locales/en";
import { useThemeTuning } from "../preferences/theme-tuning";

/** Quick settings popover (dark mode, transparency, motion, performance). */
export function ControlCenter({ protection, state, error, userName, windowCount, onOpenSettings, onOpenSwitcher, onOpenTour, onClose }: {
  protection: string;
  state: string;
  error?: string | null | undefined;
  userName: string;
  windowCount: number;
  onOpenSettings: () => void;
  onOpenSwitcher?: (() => void) | undefined;
  onOpenTour?: (() => void) | undefined;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [online, setOnline] = useState(true);
  useEffect(() => {
    const sync = () => setOnline(typeof navigator === "undefined" ? true : navigator.onLine);
    sync();
    window.addEventListener("online", sync);
    window.addEventListener("offline", sync);
    return () => {
      window.removeEventListener("online", sync);
      window.removeEventListener("offline", sync);
    };
  }, []);
  const { theme, tokens } = useTheme();
  const tuning = useThemeTuning();
  const dark = tuning.tuning.mode === "dark";
  const supportsDark = (theme.modes ?? []).includes("dark");
  const supportsTransparency = Number(tokens["material.morphism"]) > 0;
  const transparency = Number(tokens["material.opacity"]) < 1 && Number(tokens["material.morphism"]) > 0;
  const animations = tuning.tuning.animations !== false;
  const performance = tuning.tuning.performanceMode === true;
  const ultra = tuning.tuning.ultraPerformance === true;

  const tile = (active: boolean, onClick: () => void, icon: ReactNode, label: string, disabled = false) =>
    <button type="button" className={`control-center__tile${active ? " is-on" : ""}`} aria-pressed={active}
      disabled={disabled} aria-disabled={disabled || undefined} onClick={onClick}>
      {icon}<span>{label}</span>
    </button>;

  return <div className="control-center" role="dialog" aria-label={t("controlCenter.label")}>
    <div className="control-center__profile">
      <span className="control-center__avatar" aria-hidden="true">{userName.slice(0, 1).toUpperCase()}</span>
      <div className="control-center__identity">
        <strong>{userName}</strong>
        <small data-online={online ? "true" : "false"}>{t(online ? "controlCenter.online" : "controlCenter.offline")}</small>
      </div>
    </div>
    {onOpenSwitcher ? <button type="button" className="control-center__windows" onClick={() => { onOpenSwitcher(); onClose(); }}>
      <WindowStackIcon aria-hidden="true" /><span>{t("controlCenter.windows")}</span><b>{windowCount}</b>
    </button> : null}
    <div className="control-center__grid">
      {tile(dark, () => tuning.setMode(dark ? "light" : "dark"), <MoonIcon />, t("controlCenter.darkMode"), !supportsDark)}
      {tile(transparency, () => tuning.setTransparency(!transparency), <SparkIcon />, t("controlCenter.transparency"), !supportsTransparency)}
      {tile(animations, () => tuning.setAnimations(!animations), <PulseIcon />, t("controlCenter.animations"))}
      {tile(performance, () => tuning.setPerformanceMode(!performance), <SettingsIcon />, t("controlCenter.performance"))}
      {tile(ultra, () => tuning.setUltraPerformance(!ultra), <BoltIcon />, t("controlCenter.ultra"))}
    </div>
    <div className="control-center__status" data-state={state}>
      <span className="menubar__dot" aria-hidden="true" />{protection}
    </div>
    {error ? <p className="control-center__error" role="status">{t(`preferences.${error}` as MessageKey)}</p> : null}
    <div className="control-center__links">
      <button type="button" className="control-center__settings" onClick={() => { onOpenSettings(); onClose(); }}>{t("nav.settings")}</button>
      {onOpenTour ? <button type="button" className="control-center__settings" onClick={() => { onOpenTour(); onClose(); }}>{t("tour.replay")}</button> : null}
    </div>
  </div>;
}
