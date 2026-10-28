import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";
import { versionLabel } from "../system-labels";

test("settings overview keeps existing routes and the active navigation state", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings" />);
  expect(await screen.findByText("Make this desktop your own.")).toBeInTheDocument();
  const navigation = screen.getByRole("navigation", { name: "Settings" });
  const display = within(navigation).getByRole("link", { name: "Personalization" });
  fireEvent.click(display);
  await waitFor(() => expect(display).toHaveAttribute("aria-current", "page"));
  expect(await screen.findByRole("switch", { name: "Transparency effects" })).toBeInTheDocument();
  const workspace = within(navigation).getByRole("link", { name: "Workspace" });
  fireEvent.click(workspace);
  await waitFor(() => expect(workspace).toHaveAttribute("aria-current", "page"));
  expect(await screen.findByRole("button", { name: "Save arrangement" })).toBeInTheDocument();
});

test("resizes the settings navigation with the inner separator", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
  const separator = await screen.findByRole("separator", { name: "Resize settings navigation" });
  fireEvent.keyDown(separator, { key: "ArrowRight" });
  await waitFor(() => expect(separator).toHaveAttribute("aria-valuenow", "150"));
  expect(localStorage.getItem("rumahl.settings.sidenav-width")).toBe("150");
});

test("apps settings lists installed apps for management", async () => {
  render(<App snapshot={{ ...demoSnapshot, apps: [{ id: "com.example.notes", title: "Notes", launchable: true }] }} initialLocation="/settings/apps" />);
  const navigation = screen.getByRole("navigation", { name: "Settings" });
  expect(within(navigation).getByRole("link", { name: "Apps" })).toHaveAttribute("aria-current", "page");
  const notes = await waitFor(() => document.querySelector<HTMLAnchorElement>('.settings-content a[href="/settings/apps/com.example.notes"]'));
  expect(notes).toHaveTextContent("Notes");
  expect(document.querySelector('.settings-content a[href="/settings/store"]')).toBeNull();
});

test("app management opens without an app-specific settings link", async () => {
  render(<App snapshot={{ ...demoSnapshot, apps: [{ id: "com.example.notes", title: "Notes", launchable: true }] }} initialLocation="/settings/apps/com.example.notes" />);
  expect(await screen.findByRole("heading", { name: "Notes" })).toBeInTheDocument();
  expect(screen.queryByRole("link", { name: /App settings/ })).toBeNull();
});

test("device settings show version and honest unavailable-worker state", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings/device" />);
  expect(await screen.findByRole("heading", { name: "This device" })).toBeInTheDocument();
  expect(screen.getByText(versionLabel(false))).toBeInTheDocument();
  expect(screen.getByText("Secure connection required")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Retry registration" })).toBeDisabled();
  expect(screen.getByText(/private files and apps still require the server/)).toBeInTheDocument();
});


test("supported browser without a registration is not reported as active", async () => {
  vi.stubGlobal("isSecureContext", true);
  Object.defineProperty(navigator, "serviceWorker", { configurable: true, value: {
    controller: null, getRegistration: async () => undefined,
    addEventListener: vi.fn(), removeEventListener: vi.fn()
  } });
  const view = render(<App snapshot={demoSnapshot} initialLocation="/settings/device" />);
  try {
    expect(await screen.findByText("Not registered")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry registration" })).toBeEnabled();
    expect(screen.queryByText("Active · controls this window")).not.toBeInTheDocument();
  } finally {
    view.unmount();
    Reflect.deleteProperty(navigator, "serviceWorker");
    vi.unstubAllGlobals();
  }
});
