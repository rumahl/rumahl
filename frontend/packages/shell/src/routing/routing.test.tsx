import { useState } from "react";
import { ShellLink } from "./ShellLink";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { useParams, useSearchParams } from "react-router";
import type * as AppRegistry from "../apps/registry";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";
import { appPath, localPath } from "./paths";

vi.mock("../apps/registry", async (original) => {
  const module = await original<typeof AppRegistry>();
  const apps = [...module.firstPartyApps, {
    id: "test", title: "nav.apps" as const,
    routes: [{ path: "documents/:documentId", Component: FixtureDocument }]
  }];
  return { firstPartyApps: apps, findFirstPartyApp: (id: string) => apps.find((app) => app.id === id) };
});
function FixtureDocument() {
  const { documentId } = useParams();
  const [search] = useSearchParams();
  const [note, setNote] = useState("");
  return <><h1>Document {documentId}: {search.get("view")}</h1>
    <input aria-label="Draft note" value={note} onChange={(event) => setNote(event.target.value)} />
    <ShellLink to="/app/test/documents/43?view=list">Next document</ShellLink>
  </>;
}

describe("shell routes and presentation", () => {
  test("resolves a registered app's nested parameters and query without shell changes", () => {
    localStorage.setItem("rumahl.demo.shell-mode", "launcher");
    render(<App snapshot={demoSnapshot} initialLocation="/app/test/documents/42?view=grid" />);
    expect(screen.getByRole("heading", { name: "Document 42: grid" })).toBeInTheDocument();
    expect(document.querySelector(".shell-window")).toBeNull();
    expect(document.querySelector(".shell")).toHaveAttribute("data-shell-mode", "launcher");
  });

  test("preserves healthy app component state across its own nested navigation", async () => {
    localStorage.setItem("rumahl.demo.shell-mode", "launcher");
    render(<App snapshot={demoSnapshot} initialLocation="/app/test/documents/42?view=grid" />);
    fireEvent.change(screen.getByRole("textbox", { name: "Draft note" }), { target: { value: "unsaved" } });
    fireEvent.click(screen.getByRole("link", { name: "Next document" }));
    expect(await screen.findByRole("heading", { name: "Document 43: list" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Draft note" })).toHaveValue("unsaved");
  });

  test("renders a deep linked app in the desktop's protected window immediately", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/app-manager" />);
    expect(screen.getByRole("region", { name: "App manager" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close App manager" })).toBeInTheDocument();
  });

  test("changes presentation without losing the selected route", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/test/documents/42?view=grid" />);
    fireEvent.change(screen.getByRole("combobox", { name: "Shell mode" }), { target: { value: "launcher" } });
    expect(await screen.findByRole("heading", { name: "Document 42: grid" })).toBeInTheDocument();
    expect(document.querySelector(".shell-window")).toBeNull();
    fireEvent.change(screen.getByRole("combobox", { name: "Shell mode" }), { target: { value: "desktop" } });
    expect(await screen.findByRole("region", { name: "Apps" })).toBeInTheDocument();
  });

  test("keeps the settings scope when a mode change remounts its presentation", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/display" />);
    fireEvent.change(screen.getByRole("combobox", { name: "Save for" }), { target: { value: "device" } });
    fireEvent.click(screen.getByRole("button", { name: "Launcher" }));
    expect(await screen.findByRole("combobox", { name: "Save for" })).toHaveValue("device");
    expect(document.querySelector(".shell")).toHaveAttribute("data-shell-mode", "launcher");
    fireEvent.click(screen.getByRole("button", { name: "Desktop" }));
    expect(document.querySelector(".shell")).toHaveAttribute("data-shell-mode", "desktop");
    expect(screen.getByRole("combobox", { name: "Save for" })).toHaveValue("device");
  });

  test("retains independent windows and routes when closing one", async () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/app-manager" />);
    fireEvent.click(screen.getByRole("link", { name: "Settings" }));
    const settings = await screen.findByRole("region", { name: "Settings" });
    expect(screen.getByRole("region", { name: "App manager" })).toBeInTheDocument();
    fireEvent.click(within(settings).getByRole("button", { name: "Close Settings" }));
    expect(screen.queryByRole("region", { name: "Settings" })).toBeNull();
    expect(screen.getByRole("region", { name: "App manager" })).toBeInTheDocument();
  });

  test("does not invent an app or source URL from an unknown ID", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/app/not-installed/sub/page?mode=launcher" />);
    expect(screen.getByRole("heading", { name: "App unavailable", level: 1 })).toBeInTheDocument();
    expect(document.querySelector("iframe")).toBeNull();
  });

  test("gives unknown nested system pages a shell-owned not-found view", () => {
    render(<App snapshot={demoSnapshot} initialLocation="/settings/missing?mode=launcher" />);
    expect(screen.getByRole("heading", { name: "Page not found" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeInTheDocument();
  });

  test("builds encoded app paths and refuses ambiguous IDs and external navigation", () => {
    expect(appPath("com.example.notes", ["documents", "hello world"])).toBe("/app/com.example.notes/documents/hello%20world");
    for (const id of ["../admin", "a/b", "test?x=1", "", "a\\b"]) expect(() => appPath(id)).toThrow();
    expect(() => appPath("test", [".."])) .toThrow();
    expect(() => localPath("//foreign.test")).toThrow();
    expect(localPath("/app/test?mode=launcher&view=grid#note")).toBe("/app/test?view=grid#note");
  });
});
