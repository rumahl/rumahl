import { describe, expect, test } from "vitest";
import type { WeatherWarning } from "./warnings-client";
import { EXTREME_WARNING_IDLE_MS, isExtremeWeatherWarning } from "./weather-warning-policy";

function warning(overrides: Partial<WeatherWarning> = {}): WeatherWarning {
  return {
    id: "storm-1",
    source: { id: "com.example.weather", name: "Weather", registeredAt: 1 },
    level: "warning",
    headline: "Severe storm",
    description: "",
    region: "Berlin",
    event: "STORM",
    severity: "Severe",
    sender: "DWD",
    instruction: "",
    area: "",
    effective: null,
    expires: 2_000,
    test: false,
    publishedAt: 1_000,
    updatedAt: 1_000,
    ...overrides
  };
}

describe("extreme weather warning policy", () => {
  test("uses the idle threshold for fullscreen takeover", () => {
    expect(EXTREME_WARNING_IDLE_MS).toBeGreaterThanOrEqual(30_000);
  });

  test("reserves fullscreen for non-test emergency and extreme events", () => {
    expect(isExtremeWeatherWarning(warning({ level: "emergency" }))).toBe(true);
    expect(isExtremeWeatherWarning(warning({ level: "emergency", test: true }))).toBe(false);
    expect(isExtremeWeatherWarning(warning({ level: "critical", event: "TORNADO", headline: "Tornado warning" }))).toBe(true);
    expect(isExtremeWeatherWarning(warning({ level: "critical", event: "STORM" }))).toBe(false);
    expect(isExtremeWeatherWarning(warning({ level: "warning", severity: "Extreme" }))).toBe(true);
    expect(isExtremeWeatherWarning(warning({ level: "emergency", test: true, severity: "Extreme" }))).toBe(true);
  });
});
