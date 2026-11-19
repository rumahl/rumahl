import type { MessageKey } from "../i18n/locales/en";

type ThemeLabel = string | { en?: string; de?: string } | undefined;

/** Resolves a package label (literal or per locale) or falls back to i18n. */
export function resolveThemeLabel(
  label: ThemeLabel,
  labelKey: string | undefined,
  locale: string,
  t: (key: MessageKey) => string,
): string {
  if (typeof label === "string") return label;
  const localized = label?.[locale.toLowerCase().startsWith("de") ? "de" : "en"];
  if (localized) return localized;
  return labelKey ? t(labelKey as MessageKey) : "";
}
