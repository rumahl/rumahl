import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { App } from "./App";
import { demoSnapshot as fixtureSnapshot } from "./demo/snapshot";

describe("protected shell behavior", () => {
  test("navigates without allowing contributions to own shell navigation", () => {
    render(<App snapshot={fixtureSnapshot} initialLocation="/" />);

    const nav = screen.getByRole("navigation", { name: "Main navigation" });
    expect(within(nav).getByRole("link", { name: "Settings" })).toBeInTheDocument();
    fireEvent.click(within(nav).getByRole("link", { name: "Settings" }));

    expect(screen.getByRole("heading", { level: 1, name: "Settings" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeInTheDocument();
  });

  test("keeps close and minimize semantics in the first-party window chrome", () => {
    render(<App snapshot={fixtureSnapshot} initialLocation="/app/files" />);

    expect(screen.getByRole("region", { name: "Files" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Minimize Files" }));
    expect(screen.queryByRole("region", { name: "Files" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Files" }));
    fireEvent.click(screen.getByRole("button", { name: "Close Files" }));

    expect(screen.queryByRole("region", { name: "Files" })).not.toBeInTheDocument();
  });

  test("projects commands into the shell-owned command palette", () => {
    render(<App snapshot={fixtureSnapshot} initialLocation="/" />);

    fireEvent.keyDown(window, { key: "k", metaKey: true });

    const palette = screen.getByRole("dialog", { name: "Command palette" });
    expect(palette).toBeInTheDocument();
    expect(within(palette).getByRole("option", { name: /Install app/ })).toBeInTheDocument();
  });

  test("supports German while keeping English as the fallback", () => {
    const germanSnapshot = {
      ...fixtureSnapshot,
      user: { ...fixtureSnapshot.user, locale: "de-DE" }
    };
    const { unmount } = render(<App snapshot={germanSnapshot} initialLocation="/" />);

    expect(screen.getByRole("navigation", { name: "Hauptnavigation" })).toBeInTheDocument();
    expect(screen.getByText("Ein Zuhause für alles.")).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("de");

    unmount();
    render(
      <App
        initialLocation="/" snapshot={{ ...fixtureSnapshot, user: { ...fixtureSnapshot.user, locale: "fr-FR" } }}
      />
    );

    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeInTheDocument();
    expect(screen.getByText("A place for everything.")).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
  });

  test("opens a streamed app inside shell-owned window controls", async () => {
    const id = "4485f47e-a1cd-4b7b-a7c2-203086be13f5";
    const request = vi.fn(async (input: RequestInfo | URL) => {
      if (input === "/api/v1/shell/streams") {
        return Response.json({ sessions: [{ id, title: "Firefox" }] });
      }
      return Response.json({ frameUrl: `/api/v1/shell/streams/${id}/` });
    });
    const socket = {
      close: vi.fn(),
      onclose: null,
      onerror: null,
      onmessage: null,
      onopen: null
    };
    render(<App initialLocation={`/streams/${id}?title=Firefox`} snapshot={fixtureSnapshot} live={{ request, openEvents: () => socket }} />);
    const window = await screen.findByRole("region", { name: "Firefox" });
    expect(within(window).getByRole("button", { name: "Minimize Firefox" })).toBeInTheDocument();
    const frame = await within(window).findByTitle("Firefox");
    expect(frame).toHaveAttribute("src", `/api/v1/shell/streams/${id}/`);
    expect(frame).not.toHaveAttribute("src", expect.stringContaining("token="));
    fireEvent.click(within(window).getByRole("button", { name: "Close Firefox" }));
    expect(screen.queryByRole("region", { name: "Firefox" })).not.toBeInTheDocument();
  });
});
