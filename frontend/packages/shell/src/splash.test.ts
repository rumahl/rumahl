import { afterEach, describe, expect, test } from "vitest";
import { splashCopy } from "./splash";

afterEach(() => {
  document.head.querySelectorAll('meta[name="rumahl-locale"]').forEach((meta) => meta.remove());
  localStorage.removeItem("rumahl.locale");
});

function localeMeta(locale: string): void {
  const meta = document.createElement("meta");
  meta.name = "rumahl-locale";
  meta.content = locale;
  document.head.append(meta);
}

describe("splash language", () => {
  test("uses the OS locale from the document meta", () => {
    localeMeta("de-DE");
    expect(splashCopy("dataLoaded")).toBe("Systemdaten geladen");
  });

  test("uses English for unsupported locales", () => {
    localeMeta("fr-FR");
    expect(splashCopy("dataLoaded")).toBe("System data loaded");
  });

  test("remembers the last OS locale for the standalone splash", () => {
    localStorage.setItem("rumahl.locale", "de");
    expect(splashCopy("buildingDesktop")).toBe("Desktop wird aufgebaut");
  });
});
