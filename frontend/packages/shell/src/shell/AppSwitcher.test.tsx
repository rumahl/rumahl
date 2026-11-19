import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterAll, beforeAll, expect, test } from "vitest";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";

beforeAll(() => {
  if (typeof HTMLDialogElement.prototype.showModal !== "function") {
    Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
      configurable: true,
      value(this: HTMLDialogElement) { this.open = true; }
    });
  }
  if (typeof HTMLDialogElement.prototype.close !== "function") {
    Object.defineProperty(HTMLDialogElement.prototype, "close", {
      configurable: true,
      value(this: HTMLDialogElement) { this.open = false; }
    });
  }
});

afterAll(() => {
  Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal");
  Reflect.deleteProperty(HTMLDialogElement.prototype, "close");
});

test("closing the last window leaves Open windows for the launcher", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/app/files" />);
  await screen.findByRole("region", { name: "Files" });

  fireEvent.click(screen.getByRole("button", { name: "Control centre" }));
  const control = screen.getByRole("dialog", { name: "Control centre" });
  fireEvent.click(within(control).getByRole("button", { name: /Open windows/ }));

  const switcher = await screen.findByRole("dialog", { name: "Open windows" });
  fireEvent.click(within(switcher).getByRole("button", { name: "Close Files" }));

  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Open windows" })).toBeNull());
  expect(screen.queryByRole("region", { name: "Files" })).toBeNull();
  expect(document.querySelector(".desktop-home, .launcher-home")).not.toBeNull();
});
