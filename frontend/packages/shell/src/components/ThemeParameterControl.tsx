import type { ThemeParameter } from "@rumahl/ui/themes";
import { useI18n } from "../i18n";
import { resolveThemeLabel } from "./theme-labels";

import { RumahlColorPicker } from "./RumahlColorPicker";
import { RumahlSelect } from "./RumahlSelect";

/**
 * One theme parameter. Shared by the appearance page and the theme gallery so a
 * package's options look and behave the same everywhere.
 */
export function ThemeParameterControl({
  parameter,
  tokens,
  onChange,
  onSeed,
}: {
  parameter: ThemeParameter;
  tokens: Readonly<Record<string, string>>;
  onChange: (token: string, value: string) => void;
  onSeed?: ((value: string) => void) | undefined;
}) {
  const { t, locale } = useI18n();
  const label = resolveThemeLabel(parameter.label, parameter.labelKey, locale, t);
  const current = tokens[parameter.token] ?? "";

  if (parameter.kind === "range") {
    const value = parameter.parse(current);
    return <label className="theme-parameter">
      <span>{label}</span>
      <input className="range" type="range" aria-label={label}
        min={parameter.min} max={parameter.max} step={parameter.step} value={value}
        onChange={(event) => onChange(parameter.token, parameter.format(Number(event.target.value)))} />
      <output>{parameter.format(value)}</output>
    </label>;
  }

  if (parameter.kind === "color") {
    return <div className="theme-parameter theme-parameter--color">
      <span>{label}</span>
      <RumahlColorPicker
        label={label}
        value={current.startsWith("#") ? current.slice(0, 7) : "#000000"}
        presets={parameter.presets?.map((preset) => ({ value: preset.value, label: resolveThemeLabel(preset.label, preset.labelKey, locale, t) }))}
        onChange={(value) => (parameter.seed && onSeed ? onSeed(value) : onChange(parameter.token, value))}
      />
    </div>;
  }

  return <label className="theme-parameter">
    <span>{label}</span>
    <RumahlSelect
      label={label}
      value={current}
      options={parameter.options.map((option) => ({ value: option.value, label: resolveThemeLabel(option.label, option.labelKey, locale, t) }))}
      onChange={(value) => onChange(parameter.token, value)}
    />
  </label>;
}
