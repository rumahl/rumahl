import { describe, expect, test, vi } from "vitest";
import { fetchStreamSessions, grantStreamSession, streamFramePath } from "./stream-client";

const id = "4485f47e-a1cd-4b7b-a7c2-203086be13f5";

describe("stream gateway client", () => {
  test("accepts only validated session descriptors", async () => {
    const request = vi.fn(async () => Response.json({ sessions: [{ id, title: "Firefox" }] }));
    expect(await fetchStreamSessions(request)).toEqual([{ id, title: "Firefox" }]);
    expect(request).toHaveBeenCalledWith("/api/v1/shell/streams", expect.objectContaining({
      credentials: "same-origin",
      cache: "no-store"
    }));
    await expect(fetchStreamSessions(async () => Response.json({ sessions: [{ id, title: "" }] })))
      .rejects.toThrow("invalid stream session");
  });

  test("keeps grants in cookies and never accepts a tokened frame URL", async () => {
    const framePath = streamFramePath(id);
    const request = vi.fn(async () => Response.json({ frameUrl: framePath }));
    expect(await grantStreamSession(request, id)).toBe(framePath);
    expect(request).toHaveBeenCalledWith(`${framePath}grant`, expect.objectContaining({
      method: "POST",
      credentials: "same-origin"
    }));
    await expect(grantStreamSession(async () => Response.json({
      frameUrl: `${framePath}?token=leak`
    }), id)).rejects.toThrow("invalid stream frame URL");
    expect(() => streamFramePath("../../other")).toThrow("invalid stream session ID");
  });
});
