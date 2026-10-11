import type { ReactNode } from "react";
import { useTheme } from "@rumahl/ui";
import { BoltIcon, MoonIcon, PulseIcon, SettingsIcon, ShieldIcon, SparkIcon } from "../icons";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/locales/en";
import { useThemeTuning } from "../preferences/theme-tuning";
import { openEmergency } from "./emergency";

/** Quick settings popover (dark mode, transparency, motion, performance). */
export function ControlCenter({ protection, state, error, onOpenSettings, onClose }: {
  protection: string;
  state: string;
  error?: string | null | undefined;
  onOpenSettings: () => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const { tokens } = useTheme();
  const tuning = useThemeTuning();
  const dark = tuning.tuning.mode === "dark";
  const transparency = Number(tokens["material.opacity"]) < 1 && Number(tokens["material.morphism"]) > 0;
  const animations = tuning.tuning.animations !== false;
  const performance = tuning.tuning.performanceMode === true;
  const ultra = tuning.tuning.ultraPerformance === true;

  const tile = (active: boolean, onClick: () => void, icon: ReactNode, label: string) =>
    <button type="button" className={`control-center__tile${active ? " is-on" : ""}`} aria-pressed={active} onClick={onClick}>
      {icon}<span>{label}</span>
    </button>;

  return <div className="control-center" role="dialog" aria-label={t("controlCenter.label")}>
    <div className="control-center__grid">
      {tile(dark, () => tuning.setMode(dark ? "light" : "dark"), <MoonIcon />, t("controlCenter.darkMode"))}
      {tile(transparency, () => tuning.setTransparency(!transparency), <SparkIcon />, t("controlCenter.transparency"))}
      {tile(animations, () => tuning.setAnimations(!animations), <PulseIcon />, t("controlCenter.animations"))}
      {tile(performance, () => tuning.setPerformanceMode(!performance), <SettingsIcon />, t("controlCenter.performance"))}
      {tile(ultra, () => tuning.setUltraPerformance(!ultra), <BoltIcon />, t("controlCenter.ultra"))}
      {tile(false, () => { onClose(); openEmergency(); }, <ShieldIcon />, t("controlCenter.emergency"))}
    </div>
    <div className="control-center__status" data-state={state}>
      <span className="menubar__dot" aria-hidden="true" />{protection}
    </div>
    {error ? <p className="control-center__error" role="status">{t(`preferences.${error}` as MessageKey)}</p> : null}
    <button type="button" className="control-center__settings" onClick={() => { onOpenSettings(); onClose(); }}>{t("nav.settings")}</button>
  </div>;
}
