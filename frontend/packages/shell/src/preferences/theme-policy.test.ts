import { describe, expect, test } from "vitest";
import type { ShellRequest } from "../snapshot-client";
import { fetchFullAccessThemes } from "./theme-policy";

function requestReturning(value: unknown, status = 200): ShellRequest {
  return (async () => new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  })) as unknown as ShellRequest;
}

describe("theme policy client", () => {
  test("reads the device policy", async () => {
    await expect(fetchFullAccessThemes(requestReturning({ allowFullAccessThemes: true }))).resolves.toBe(true);
    await expect(fetchFullAccessThemes(requestReturning({ allowFullAccessThemes: false }))).resolves.toBe(false);
  });

  test("fails closed for malformed or failed responses", async () => {
    await expect(fetchFullAccessThemes(requestReturning({ allowFullAccessThemes: "yes" }))).resolves.toBe(false);
    await expect(fetchFullAccessThemes(requestReturning({}, 503))).resolves.toBe(false);
  });
});
