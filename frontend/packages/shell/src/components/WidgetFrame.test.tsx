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

  test("accepts a demo host path on the demo origin", () => {
    const origin = "https://demo.rumahl.com";
    expect(parseWidgetFrameDescriptor({
      frameVersion: 1,
      contributionId: widget.id,
      frameUrl: `${origin}/widgets/home/`
    }, widget.id, origin, false, origin).frameUrl).toBe(`${origin}/widgets/home/`);
    expect(() => parseWidgetFrameDescriptor({
      frameVersion: 1,
      contributionId: widget.id,
      frameUrl: `${origin}/not-a-widget/`
    }, widget.id, origin, false, origin)).toThrow("unsafe widget frame URL");
  });

  test("rejects another port on the shell host because cookies ignore ports", () => {
    expect(() => parseWidgetFrameDescriptor({
      frameVersion: 1,
      contributionId: widget.id,
      frameUrl: "https://shell.rumahl.com:8443/"
    }, widget.id, "https://shell.rumahl.com")).toThrow("unsafe widget frame URL");
  });

  test("falls back instead of leaving a refused-connection iframe on screen", async () => {
    const request = vi.fn<typeof fetch>().mockResolvedValue(Response.json({
      frameVersion: 1,
      contributionId: widget.id,
      frameUrl: "http://127.0.0.1:1/"
    }));
    const preflight = vi.fn<typeof fetch>().mockRejectedValue(new TypeError("refused"));
    vi.stubGlobal("fetch", preflight);
    try {
      render(<I18nProvider locale="en"><WidgetFrame allowDevelopmentLoopback request={request} widget={widget} /></I18nProvider>);
      expect(await screen.findByText(/Widget unavailable/)).toBeInTheDocument();
      expect(screen.queryByTitle("Home")).not.toBeInTheDocument();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
