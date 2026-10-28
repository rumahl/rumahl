import { afterEach, describe, expect, test } from "vitest";
import { waitForUiStyles } from "./ui-ready";

const TOKEN = "--rumahl-ui-color-text-primary";

afterEach(() => {
  document.head.querySelectorAll("link[data-ui-ready-test]").forEach((link) => link.remove());
  document.documentElement.style.removeProperty(TOKEN);
  delete document.documentElement.dataset.uiReady;
});

describe("UI readiness", () => {
  test("marks the UI ready when there is nothing to load", async () => {
    await expect(waitForUiStyles(50)).resolves.toBe("ready");
    expect(document.documentElement.dataset.uiReady).toBe("ready");
  });

  test("waits for stylesheets and critical tokens", async () => {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.dataset.uiReadyTest = "";
    Object.defineProperty(link, "sheet", { value: { cssRules: [{}] } });
    document.head.append(link);
    document.documentElement.style.setProperty(TOKEN, "#1d1d1f");

    await expect(waitForUiStyles(100)).resolves.toBe("ready");
    expect(document.documentElement.dataset.uiReady).toBe("ready");
  });

  test("degrades instead of blocking when a stylesheet never loads", async () => {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.dataset.uiReadyTest = "";
    document.head.append(link);
    document.documentElement.style.setProperty(TOKEN, "#1d1d1f");

    await expect(waitForUiStyles(30)).resolves.toBe("degraded");
    expect(document.documentElement.dataset.uiReady).toBe("degraded");
  });
});
