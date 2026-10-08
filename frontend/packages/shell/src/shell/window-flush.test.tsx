import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@rumahl/ui", () => ({ useTheme: () => ({ theme: {} }) }));
vi.mock("../i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));

import { ProtectedWindow } from "../components/ProtectedWindow";
import { isInstalledAppWindow } from "./DesktopWindows";

describe("flush app windows", () => {
  it("detects installed iframe apps only", () => {
    expect(isInstalledAppWindow("app:com.rumahl.devapp")).toBe(true);
    expect(isInstalledAppWindow("app:files")).toBe(false);
    expect(isInstalledAppWindow("app:app-manager")).toBe(false);
    expect(isInstalledAppWindow("stream:abc")).toBe(false);
    expect(isInstalledAppWindow("/settings/*")).toBe(false);
  });

  it("marks a flush window with the flush class", () => {
    const { container } = render(
      <ProtectedWindow flush focused id="app:com.rumahl.devapp" title="App" subtitle="" variant="standard" onClose={() => {}} onFocus={() => {}} onMinimize={() => {}}>
        <iframe className="installed-app-frame" />
      </ProtectedWindow>,
    );
    expect(container.querySelector(".shell-window--flush")).not.toBeNull();
  });
});
