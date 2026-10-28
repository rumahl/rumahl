import { render, screen } from "@testing-library/react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test, vi } from "vitest";
import { parseWidgetFrameDescriptor } from "@rumahl/contracts/widget";
import { I18nProvider } from "../i18n";
import { IsolatedFrame, WidgetFrame } from "./WidgetFrame";

const widget = {
  kind: "widget",
  id: "com.example.home.dashboard",
  title: "Home",
  slot: "dashboard_widgets",
  entrypoint: "dashboard"
} as const;

describe("isolated widget frame", () => {
  test("renders a validated app-origin frame without shell-origin privileges", () => {
    expect(parseWidgetFrameDescriptor({
      frameVersion: 1,
      contributionId: widget.id,
      frameUrl: "http://127.0.0.1:5174/"
    }, widget.id, location.origin, true).frameUrl).toBe("http://127.0.0.1:5174/");

    const markup = renderToStaticMarkup(<IsolatedFrame title="Home" url="https://app.example.com/" />);
    expect(markup).toContain('sandbox="allow-scripts"');
    expect(markup).not.toContain("allow-same-origin");
    expect(markup).toMatch(/referrerpolicy="no-referrer"/i);
  });

  test("requests the frame descriptor through the same shell API", async () => {
    const request = vi.fn<typeof fetch>().mockResolvedValue(Response.json({
      frameVersion: 1,
      contributionId: widget.id,
      frameUrl: "http://127.0.0.1:5174/"
    }));
    render(<I18nProvider locale="en"><WidgetFrame allowDevelopmentLoopback request={request} widget={widget} /></I18nProvider>);

    await vi.waitFor(() => expect(request).toHaveBeenCalledOnce());
    expect(request).toHaveBeenCalledWith(
      `/api/v1/shell/widgets/${widget.id}/frame`,
      expect.objectContaining({ credentials: "same-origin", cache: "no-store" })
    );
  });

  test("rejects a same-origin descriptor while leaving shell rendering intact", async () => {
    const request = vi.fn<typeof fetch>().mockResolvedValue(Response.json({
      frameVersion: 1,
      contributionId: widget.id,
      frameUrl: `${location.origin}/bad-widget`
    }));
    render(<I18nProvider locale="en"><WidgetFrame request={request} widget={widget} /></I18nProvider>);

    expect(await screen.findByRole("status")).toHaveTextContent("Widget unavailable");
    expect(screen.queryByTitle("Home")).not.toBeInTheDocument();
  });

  test("rejects another port on the shell host because cookies ignore ports", () => {
    expect(() => parseWidgetFrameDescriptor({
      frameVersion: 1,
      contributionId: widget.id,
      frameUrl: "https://shell.rumahl.com:8443/"
    }, widget.id, "https://shell.rumahl.com")).toThrow("unsafe widget frame URL");
  });
});
