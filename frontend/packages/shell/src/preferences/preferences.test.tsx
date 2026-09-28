import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ShellPreferencesProvider, useShellPreferences } from "./ShellPreferences";
import { parsePreferences } from "./client";
import type { ShellLiveSource } from "../live-updates";
const shellTheme = "com.rumahl.default";
const defaults = { settingsVersion: 1, ownerId: "01990000-0000-7000-8000-000000000001", revision: 0, user: { shellMode: "desktop", shellTheme }, device: { shellMode: null, shellTheme: null }, effective: { shellMode: "desktop", shellTheme } };
function Probe() {
  const settings = useShellPreferences();
  return <><p>{settings.mode}</p><p>{settings.error}</p><button disabled={!settings.ready || settings.saving} onClick={() => settings.save("user", "launcher")}>Save user</button></>;
}
const source = (request: ShellLiveSource["request"]): ShellLiveSource => ({ request, openEvents: () => { throw new Error("not used by preferences"); } });
afterEach(() => vi.useRealTimers());
describe("synchronized preferences", () => {
  it("rejects inconsistent effective values and unsafe revisions", () => {
    expect(parsePreferences(defaults).effective.shellMode).toBe("desktop");
    expect(() => parsePreferences({ ...defaults, effective: { shellMode: "launcher", shellTheme } })).toThrow();
    expect(() => parsePreferences({ ...defaults, revision: Number.MAX_SAFE_INTEGER + 1 })).toThrow();
  });
  it("never loads another user's cached presentation before authenticated preferences", async () => {
    localStorage.setItem("rumahl.preferences.v1:another-user:profile", JSON.stringify({ ...defaults, effective: { shellMode: "launcher", shellTheme } }));
    let respond!: (response: Response) => void;
    const request = vi.fn(() => new Promise<Response>((resolve) => { respond = resolve; }));
    render(<ShellPreferencesProvider live={source(request)}><Probe /></ShellPreferencesProvider>);
    expect(screen.getByText("desktop")).toBeInTheDocument();
    expect(screen.getByRole("button")).toBeDisabled();
    await act(async () => respond(new Response(JSON.stringify(defaults))));
    expect(screen.getByRole("button")).toBeEnabled();
  });
  it("refreshes on conflicts without silently retrying a stale write", async () => {
    const latest = { ...defaults, revision: 2, user: { shellMode: "launcher", shellTheme }, effective: { shellMode: "launcher", shellTheme } };
    const request = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify(defaults))).mockResolvedValueOnce(new Response(null, { status: 409 })).mockResolvedValueOnce(new Response(JSON.stringify(latest)));
    render(<ShellPreferencesProvider live={source(request)}><Probe /></ShellPreferencesProvider>);
    await waitFor(() => expect(screen.getByRole("button")).toBeEnabled());
    fireEvent.click(screen.getByRole("button"));
    expect(await screen.findByText("conflict")).toBeInTheDocument();
    expect(screen.getByText("launcher")).toBeInTheDocument();
    const update = request.mock.calls[1]?.[1];
    expect(JSON.parse(update.body)).toEqual({ settingsVersion: 1, key: "shell.mode", revision: 0, scope: "user", value: "launcher" });
    expect(request.mock.calls.filter((call) => call[1]?.method === "PUT")).toHaveLength(1);
  });
  it("ignores a read started before a completed write", async () => {
    let staleRead!: (response: Response) => void;
    const request = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify(defaults)))
      .mockImplementationOnce(() => new Promise<Response>((resolve) => { staleRead = resolve; }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ ...defaults, revision: 1, user: { shellMode: "launcher", shellTheme }, effective: { shellMode: "launcher", shellTheme } })));
    render(<ShellPreferencesProvider live={source(request)}><Probe /></ShellPreferencesProvider>);
    await waitFor(() => expect(screen.getByRole("button")).toBeEnabled());
    act(() => window.dispatchEvent(new Event("focus")));
    fireEvent.click(screen.getByRole("button"));
    expect(await screen.findByText("launcher")).toBeInTheDocument();
    await act(async () => staleRead(new Response(JSON.stringify(defaults))));
    expect(screen.getByText("launcher")).toBeInTheDocument();
  });
  it("automatically applies remote changes and stops polling after unmount", async () => {
    vi.useFakeTimers();
    const request = vi.fn().mockImplementation(async () => new Response(JSON.stringify(defaults)));
    const view = render(<ShellPreferencesProvider live={source(request)}><Probe /></ShellPreferencesProvider>);
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    request.mockImplementation(async () => new Response(JSON.stringify({ ...defaults, revision: 1, device: { shellMode: "launcher", shellTheme: null }, effective: { shellMode: "launcher", shellTheme } })));
    await act(async () => { await vi.advanceTimersByTimeAsync(2_000); });
    expect(screen.getByText("launcher")).toBeInTheDocument();
    view.unmount();
    const count = request.mock.calls.length;
    await vi.advanceTimersByTimeAsync(10_000);
    expect(request).toHaveBeenCalledTimes(count);
  });
});
