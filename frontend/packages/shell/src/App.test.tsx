import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { App } from "./App";
import { fixtureSnapshot } from "./fixture";

describe("protected shell behavior", () => {
  test("navigates without allowing contributions to own shell navigation", () => {
    render(<App snapshot={fixtureSnapshot} />);

    expect(screen.getByText("12 local apps")).toBeInTheDocument();
    expect(screen.getByText("4 minutes ago")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Apps" }));

    expect(screen.getByRole("heading", { level: 1, name: "Apps" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeInTheDocument();
  });

  test("keeps close and minimize semantics in the first-party window chrome", () => {
    render(<App snapshot={fixtureSnapshot} />);

    fireEvent.click(screen.getByRole("button", { name: "Open app manager" }));
    expect(screen.getByRole("region", { name: "App manager" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Minimize App manager" }));
    expect(screen.queryByRole("region", { name: "App manager" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "App manager" }));
    fireEvent.click(screen.getByRole("button", { name: "Close App manager" }));

    expect(screen.queryByRole("region", { name: "App manager" })).not.toBeInTheDocument();
  });

  test("projects commands into the shell-owned command palette", () => {
    render(<App snapshot={fixtureSnapshot} />);

    fireEvent.keyDown(window, { key: "k", metaKey: true });

    const palette = screen.getByRole("dialog", { name: "Command palette" });
    expect(palette).toBeInTheDocument();
    expect(within(palette).getByRole("button", { name: /Install app/ })).toBeInTheDocument();
  });

  test("supports German while keeping English as the fallback", () => {
    const germanSnapshot = {
      ...fixtureSnapshot,
      user: { ...fixtureSnapshot.user, locale: "de-DE" }
    };
    const { unmount } = render(<App snapshot={germanSnapshot} />);

    expect(screen.getByRole("navigation", { name: "Hauptnavigation" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Alles zuhause." })).toBeInTheDocument();
    expect(screen.getByText("12 lokale Apps")).toBeInTheDocument();
    expect(screen.getByText("vor 4 Minuten")).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("de");

    unmount();
    render(
      <App
        snapshot={{ ...fixtureSnapshot, user: { ...fixtureSnapshot.user, locale: "fr-FR" } }}
      />
    );

    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Everything at home." })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
  });

  test("formats singular runtime values instead of embedding them in translations", () => {
    render(
      <App
        snapshot={{
          ...fixtureSnapshot,
          systemStatus: { ...fixtureSnapshot.systemStatus, installedAppCount: 1 }
        }}
      />
    );

    expect(screen.getByText("1 local app")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open app manager" }));
    expect(screen.getByText("1 app")).toBeInTheDocument();
    expect(screen.getByText(/rumahl OS installs and updates apps/)).toBeInTheDocument();
  });
});
