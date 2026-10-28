import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { dateKey, parseDate, shiftMonth, RumahlCalendar } from "./RumahlCalendar";

describe("calendar", () => {
  test("rejects invalid days and preserves local date keys", () => {
    expect(parseDate("2025-02-29")).toBeNull();
    expect(parseDate("2024-02-29")).not.toBeNull();
    expect(parseDate("invalid")).toBeNull();
    expect(dateKey(parseDate("2026-09-30")!)).toBe("2026-09-30");
  });
  test("clamps month movement at leap years and crosses year boundaries", () => {
    expect(dateKey(shiftMonth(parseDate("2024-01-31")!, 1))).toBe("2024-02-29");
    expect(dateKey(shiftMonth(parseDate("2025-01-31")!, 1))).toBe("2025-02-28");
    expect(dateKey(shiftMonth(parseDate("2025-12-31")!, 1))).toBe("2026-01-31");
  });
  test("keeps keyboard navigation within date limits", () => {
    const change = vi.fn();
    render(<RumahlCalendar value="2026-09-30" min="2026-09-29" max="2026-10-02" onChange={change} />);
    const selected = screen.getByRole("button", { name: "Wednesday, September 30, 2026" });
    selected.focus();
    fireEvent.keyDown(selected, { key: "ArrowRight" });
    expect(document.activeElement).toHaveAttribute("data-date", "2026-10-01");
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(document.activeElement).toHaveAttribute("data-date", "2026-10-02");
    fireEvent.click(document.activeElement!);
    expect(change).toHaveBeenCalledWith("2026-10-02");
    expect(screen.getByRole("button", { name: "Saturday, October 3, 2026" })).toBeDisabled();
  });
});
