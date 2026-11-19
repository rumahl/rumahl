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
  expect(await screen.findByRole("heading", { name: "Personalization" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("link", { name: /Surface effects/ }));
  expect(await screen.findByRole("heading", { level: 1, name: "Surface effects" })).toBeInTheDocument();
  expect(await screen.findByRole("switch", { name: "Transparency effects" })).toBeInTheDocument();
  const workspace = within(navigation).getByRole("link", { name: "Workspace" });
  fireEvent.click(workspace);
  await waitFor(() => expect(workspace).toHaveAttribute("aria-current", "page"));
  expect(await screen.findByRole("button", { name: "Save arrangement" })).toBeInTheDocument();
});

test("personalization overview links every section page", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
  expect(await screen.findByRole("heading", { name: "Personalization" })).toBeInTheDocument();
  for (const [name, href] of [["Appearance", "/settings/display/appearance"], ["Typography", "/settings/display/typography"], ["Surface effects", "/settings/display/surface"], ["Advanced", "/settings/display/advanced"]] as const) {
    expect(document.querySelector<HTMLAnchorElement>(`a[href="${href}"]`)).toHaveTextContent(name);
  }
});

test("personalization subpage keeps the back button and section content", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings/display/typography" />);
  expect(await screen.findByRole("heading", { name: "Typography" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Back" })).toHaveAttribute("href", "/settings/display");
  expect(screen.getByText("Text size")).toBeInTheDocument();
  expect(screen.queryByRole("switch", { name: "Transparency effects" })).toBeNull();
});

test("profile shortcut opens the account settings", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/" />);
  fireEvent.click(await screen.findByLabelText("Open user settings"));
  expect(await screen.findByRole("heading", { name: "Account" })).toBeInTheDocument();
  expect(screen.getAllByText("Kaim").length).toBeGreaterThan(0);
  expect(screen.getByRole("link", { name: "Back" })).toHaveAttribute("href", "/settings");
  expect(screen.getByRole("button", { name: "Sign out" })).toBeInTheDocument();
});

test("administration navigation is only shown to administrators", async () => {
  const owner = render(<App snapshot={demoSnapshot} initialLocation="/settings" />);
  expect(await screen.findByRole("link", { name: "Users" })).toBeInTheDocument();
  owner.unmount();
  render(<App snapshot={{ ...demoSnapshot, user: { ...demoSnapshot.user, role: "user" } }} initialLocation="/settings" />);
  await screen.findByText("Make this desktop your own.");
  expect(screen.queryByRole("link", { name: "Users" })).toBeNull();
});

test("administrator user management loads accounts", async () => {
  vi.stubGlobal("fetch", vi.fn(async () => ({
    ok: true,
    json: async () => ({ users: [{ id: "20000000-0000-4000-8000-000000000001", username: "ada", displayName: "Ada Example", status: "active", role: "administrator" }] })
  })));
  try {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/admin/users" />);
    expect(await screen.findByRole("heading", { name: "Users" })).toBeInTheDocument();
    expect(await screen.findByText("Ada Example")).toBeInTheDocument();
    expect(screen.getByText(/@ada/)).toBeInTheDocument();
  } finally {
    vi.unstubAllGlobals();
  }
});

test("selecting a user opens a focused account detail view", async () => {
  vi.stubGlobal("fetch", vi.fn(async () => ({
    ok: true,
    json: async () => ({ users: [{ id: "20000000-0000-4000-8000-000000000001", username: "ada", displayName: "Ada Example", status: "active", role: "administrator" }] })
  })));
  try {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/admin/users/20000000-0000-4000-8000-000000000001" />);
    expect(await screen.findByRole("heading", { level: 1, name: "Ada Example" })).toBeInTheDocument();
    expect(screen.getByText("Name and username")).toBeInTheDocument();
    expect(screen.getByText("Account type")).toBeInTheDocument();
    expect(screen.getByText("Account enabled")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reset password" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove user" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Manage sessions" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back" })).toHaveAttribute("href", "/settings/admin/users");
    const breadcrumb = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(within(breadcrumb).getByRole("link", { name: "Users" })).toHaveAttribute("href", "/settings/admin/users");
    expect(breadcrumb).toHaveTextContent("Ada Example");
  } finally {
    vi.unstubAllGlobals();
  }
});

test("settings pages expose their nesting as a breadcrumb", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings/display/surface" />);
  const breadcrumb = await screen.findByRole("navigation", { name: "Breadcrumb" });
  expect(within(breadcrumb).getByRole("link", { name: "Overview" })).toHaveAttribute("href", "/settings");
  expect(within(breadcrumb).getByRole("link", { name: "Personalization" })).toHaveAttribute("href", "/settings/display");
  expect(breadcrumb).toHaveTextContent("Surface effects");
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

test("system exposes device and frontend version as submenus", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings/system" />);
  expect(await screen.findByRole("heading", { name: "System" })).toBeInTheDocument();
  const device = document.querySelector<HTMLAnchorElement>('a[href="/settings/system/device"]');
  const version = document.querySelector<HTMLAnchorElement>('a[href="/settings/system/version"]');
  expect(device).toHaveTextContent("This device");
  expect(version).toHaveTextContent("Version");
});

test("frontend version lives under System Version", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings/system/version" />);
  expect(await screen.findByRole("heading", { name: "Version" })).toBeInTheDocument();
  expect(screen.getByText("Frontend version")).toBeInTheDocument();
  expect(screen.getByText(versionLabel(false))).toBeInTheDocument();
});

test("device settings show honest unavailable-worker state", async () => {
  render(<App snapshot={demoSnapshot} initialLocation="/settings/system/device" />);
  expect(await screen.findByRole("heading", { name: "This device" })).toBeInTheDocument();
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
  const view = render(<App snapshot={demoSnapshot} initialLocation="/settings/system/device" />);
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
