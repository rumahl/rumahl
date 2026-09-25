import { createContext, useContext, useMemo, type PropsWithChildren } from "react";
import { german } from "./locales/de";
import { english, type MessageKey, type Messages } from "./locales/en";

const catalogs = {
  de: german,
  en: english
} as const satisfies Record<string, Messages>;

export type SupportedLocale = keyof typeof catalogs;

type Parameters = Readonly<Record<string, string | number>>;
type Translate = (key: MessageKey, parameters?: Parameters) => string;

interface I18n {
  formatRelativeTime: (timestampUnixMs: number, referenceUnixMs: number) => string;
  locale: SupportedLocale;
  t: Translate;
}

const I18nContext = createContext<I18n>(createI18n("en"));

export function I18nProvider({ children, locale }: PropsWithChildren<{ locale: string }>) {
  const resolvedLocale = resolveLocale(locale);
  const value = useMemo(() => createI18n(resolvedLocale), [resolvedLocale]);
  return <I18nContext value={value}>{children}</I18nContext>;
}

export function resolveLocale(locale: string): SupportedLocale {
  const language = locale.toLowerCase().split("-", 1)[0];
  return language && Object.hasOwn(catalogs, language) ? (language as SupportedLocale) : "en";
}

export function useI18n(): I18n {
  return useContext(I18nContext);
}

function createI18n(locale: SupportedLocale): I18n {
  const messages = catalogs[locale];
  const numberFormat = new Intl.NumberFormat(locale);
  const pluralRules = new Intl.PluralRules(locale);
  const relativeTimeFormat = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });

  return {
    locale,
    t: (key, parameters) => {
      const message = messages[key];
      const template =
        typeof message === "string"
          ? message
          : message[pluralRules.select(Number(parameters?.count)) === "one" ? "one" : "other"];
      return interpolate(template, parameters, numberFormat);
    },
    formatRelativeTime: (timestampUnixMs, referenceUnixMs) => {
      const seconds = Math.round((timestampUnixMs - referenceUnixMs) / 1_000);
      const absoluteSeconds = Math.abs(seconds);
      if (absoluteSeconds < 60) return relativeTimeFormat.format(seconds, "second");
      const minutes = Math.round(seconds / 60);
      if (absoluteSeconds < 3_600) return relativeTimeFormat.format(minutes, "minute");
      const hours = Math.round(seconds / 3_600);
      if (absoluteSeconds < 86_400) return relativeTimeFormat.format(hours, "hour");
      return relativeTimeFormat.format(Math.round(seconds / 86_400), "day");
    }
  };
}

function interpolate(
  message: string,
  parameters: Parameters | undefined,
  numberFormat: Intl.NumberFormat
): string {
  if (!parameters) return message;
  return Object.entries(parameters).reduce(
    (result, [key, value]) =>
      result.replaceAll(`{${key}}`, typeof value === "number" ? numberFormat.format(value) : value),
    message
  );
}
