import { act } from "react";
import { fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { renderShellDocument } from "../../ssr/src/render";
import { hydrateShell } from "./bootstrap";
import { demoSnapshot } from "./demo/snapshot";
import { readEmbeddedShellSnapshot } from "./embedded-snapshot";

afterEach(() => {
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

test("hydrates the server markup with the same snapshot and interactive controls", async () => {
  const rendered = await renderShellDocument(
    demoSnapshot,
    { script: "/assets/index-abc123.js", stylesheet: "/assets/index-abc123.css" },
    "AbCdEfGhIjKlMnOpQrStUvWx"
  );
  let html = "";
  for await (const chunk of rendered.stream) html += chunk.toString();
  const parsed = new DOMParser().parseFromString(html, "text/html");
  document.documentElement.lang = parsed.documentElement.lang;
  document.body.innerHTML = parsed.body.innerHTML;
  const initialMarkup = document.getElementById("root")?.innerHTML;
  const snapshot = readEmbeddedShellSnapshot();
  const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);

  expect(snapshot).toEqual(demoSnapshot);
  let root: ReturnType<typeof hydrateShell> | undefined;
  await act(async () => {
    root = hydrateShell(snapshot!);
  });

  expect(document.getElementById("root")?.innerHTML).toContain("Everything at home.");
  expect(initialMarkup).toContain("Everything at home.");
  // Server and browser renderers share one module instance only inside this test process.
  expect(
    errors.mock.calls
      .flat()
      .map(String)
      .filter((message) => !message.includes("Detected multiple renderers concurrently"))
  ).toEqual([]);
  fireEvent.click(screen.getByRole("button", { name: "Open app manager" }));
  expect(screen.getByRole("region", { name: "App manager" })).toBeInTheDocument();
  await act(async () => root?.unmount());
});
