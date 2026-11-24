import { describe, expect, test, vi } from "vitest";
import type { ShellRequest } from "../snapshot-client";
import {
  clearWeatherWarnings,
  fetchWeatherWarnings,
  parseWarningsState,
  publishTestWarning,
  publishWeatherWarnings,
  registerWarningSource
} from "./warnings-client";

const source = { id: "com.example.weather", name: "Weather", registeredAt: 1 };
const warning = {
  id: "storm-1",
  source,
  level: "warning",
  headline: "Severe storm",
  description: "Gusts up to 120 km/h.",
  region: "Berlin",
  event: "STORM",
  severity: "Severe",
  sender: "DWD",
  instruction: "Secure loose objects.",
  area: "Berlin",
  effective: 1_000,
  expires: 2_000,
  test: false,
  publishedAt: 1_000,
  updatedAt: 1_000
};

function response(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

function requestWith(body: unknown): ShellRequest {
  return vi.fn(async () => response(body)) as unknown as ShellRequest;
}

describe("weather warning client", () => {
  test("parses a bounded active warning set", () => {
    const parsed = parseWarningsState({
      warningsVersion: 1,
      revision: "abc123",
      observedAt: 1_500,
      warnings: [warning]
    });
    expect(parsed.warnings[0]).toMatchObject({ id: "storm-1", level: "warning", source: { name: "Weather" } });
  });

  test("rejects unknown levels, duplicate ids and unbounded lists", () => {
    const base = { warningsVersion: 1, revision: "abc123", observedAt: 1_500 };
    expect(() => parseWarningsState({ ...base, warnings: [{ ...warning, level: "catastrophe" }] })).toThrow();
    expect(() => parseWarningsState({ ...base, warnings: [warning, warning] })).toThrow();
    expect(() => parseWarningsState({ ...base, warnings: Array.from({ length: 65 }, (_, index) => ({ ...warning, id: `w-${index}` })) })).toThrow();
    expect(() => parseWarningsState({ ...base, revision: "../escape", warnings: [] })).toThrow();
  });

  test("fetches, registers and publishes through the shell session", async () => {
    const state = { warningsVersion: 1, revision: "abc123", observedAt: 1_500, warnings: [warning] };
    const fetchCall = requestWith(state);
    expect((await fetchWeatherWarnings(fetchCall, new AbortController().signal)).warnings).toHaveLength(1);

    const registration = requestWith({ registrationVersion: 1, source });
    expect(await registerWarningSource(registration, "com.example.weather", new AbortController().signal)).toEqual(source);

    const publish = requestWith({ publishVersion: 1, accepted: 1, warnings: [warning] });
    expect(await publishWeatherWarnings(publish, "com.example.weather", [{
      id: "storm-1",
      level: "warning",
      headline: "Severe storm",
      expires: 2_000
    }], new AbortController().signal)).toBe(1);

    const clear = requestWith({ clearVersion: 1, cleared: 1 });
    expect(await clearWeatherWarnings(clear, "com.example.weather", ["storm-1"], new AbortController().signal)).toBe(1);
  });

  test("publishes developer test warnings only for a valid browser profile", async () => {
    const test = requestWith({ publishVersion: 1, accepted: 1, warnings: [warning] });
    await expect(publishTestWarning(
      test,
      "00000000-0000-4000-8000-000000000001",
      "critical",
      new AbortController().signal
    )).resolves.toBe(1);
    await expect(publishTestWarning(test, "../escape", "critical", new AbortController().signal)).rejects.toThrow();
  });
});
