interface RumahlSplashApi {
  progress(value: number, label?: string): void;
  finish(): void;
  fail(message: string): void;
  setLocale?(locale: string): void;
}

declare global {
  interface Window {
    rumahlSplash?: RumahlSplashApi;
  }
}


export type SplashCopyKey =
  | "boot"
  | "preparing"
  | "loadingData"
  | "dataLoaded"
  | "buildingDesktop"
  | "failed";

const copy = {
  de: {
    boot: "rumahl OS",
    preparing: "Oberfläche wird vorbereitet",
    loadingData: "Systemdaten werden geladen",
    dataLoaded: "Systemdaten geladen",
    buildingDesktop: "Desktop wird aufgebaut",
    failed: "rumahl OS konnte nicht gestartet werden."
  },
  en: {
    boot: "rumahl OS",
    preparing: "Preparing the interface",
    loadingData: "Loading system data",
    dataLoaded: "System data loaded",
    buildingDesktop: "Building the desktop",
    failed: "rumahl OS could not start."
  }
} as const;

let requestedLocale: keyof typeof copy | null = null;

function splashLocale(): keyof typeof copy {
  if (requestedLocale) return requestedLocale;
  if (typeof document === "undefined") return "en";
  const meta = document.querySelector('meta[name="rumahl-locale"]')?.getAttribute("content");
  let stored = null;
  try { stored = localStorage.getItem("rumahl.locale"); } catch { /* private mode */ }
  const appDocument = Boolean(document.querySelector('meta[name="rumahl-shell"]'));
  const raw = meta ||
    stored ||
    (appDocument ? document.documentElement.lang : null) ||
    (typeof navigator !== "undefined" ? navigator.language : null) ||
    document.documentElement.lang ||
    "en";
  return raw.toLowerCase().startsWith("de") ? "de" : "en";
}

export function splashCopy(key: SplashCopyKey): string {
  return copy[splashLocale()][key];
}

export function splashSetLocale(locale: string): void {
  requestedLocale = locale.toLowerCase().startsWith("de") ? "de" : "en";
  if (typeof window === "undefined") return;
  window.rumahlSplash?.setLocale?.(locale);
}

export function splashProgress(value: number, label?: string): void {
  if (typeof window === "undefined") return;
  window.rumahlSplash?.progress(value, label);
}

export function splashFinish(): void {
  if (typeof window === "undefined") return;
  window.rumahlSplash?.finish();
}

export function splashFail(message: string): void {
  if (typeof window === "undefined") return;
  window.rumahlSplash?.fail(message);
}

/** Finish only after the first painted frame, never with an artificial delay. */
export function finishSplashAfterPaint(): void {
  if (typeof window === "undefined" || typeof window.requestAnimationFrame !== "function") {
    splashFinish();
    return;
  }
  window.requestAnimationFrame(() => window.requestAnimationFrame(() => splashFinish()));
}
