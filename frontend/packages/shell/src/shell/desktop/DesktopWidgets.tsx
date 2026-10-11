import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { ShellSystemStatus } from "@rumahl/contracts";
import { useI18n } from "../../i18n";
import { useClock } from "../clock";
import { useGlassHost } from "../../glass-engine/useGlassEngine";
import { GLASS_MATERIALS } from "@rumahl/ui/glass";
import { clampDesktopPosition, setDesktopPosition, useDesktopPositions } from "./desktop-positions";

const WIDGET_GAP = 12;
const EDGE = 16;
/** Top inset so widgets clear the floating top bar. */
const TOP_EDGE = 60;

/**
 * Desktop widgets. They default to a column in the top-right corner but can be
 * dragged anywhere; the position is stored like a desktop icon's.
 */
export function DesktopWidgets({ systemStatus }: { systemStatus: ShellSystemStatus }) {
  const { t, locale } = useI18n();
  const now = useClock();
  const hostRef = useRef<HTMLDivElement>(null);
  const clockRef = useRef<HTMLElement>(null);
  const statusRef = useRef<HTMLElement>(null);
  const positions = useDesktopPositions();
  const [defaults, setDefaults] = useState<Record<string, { x: number; y: number }>>({});
  const [dragged, setDragged] = useState<null | { id: string; x: number; y: number }>(null);
  useGlassHost(clockRef, GLASS_MATERIALS.widget);
  useGlassHost(statusRef, GLASS_MATERIALS.widget);

  useEffect(() => {
    const host = hostRef.current;
    const clock = clockRef.current;
    if (!host || !clock) return;
    const measure = () => {
      const width = host.clientWidth;
      const clockWidth = clock.offsetWidth || 230;
      const clockHeight = clock.offsetHeight || 120;
      const statusWidth = statusRef.current?.offsetWidth || clockWidth;
      setDefaults({
        "widget:clock": { x: Math.max(EDGE, width - clockWidth - EDGE), y: TOP_EDGE },
        "widget:status": { x: Math.max(EDGE, width - statusWidth - EDGE), y: TOP_EDGE + clockHeight + WIDGET_GAP }
      });
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  const place = (id: string): { x: number; y: number } | undefined => dragged?.id === id ? dragged : positions[id] ?? defaults[id];
  // Apply positions with CSSOM: the shell's strict style-src CSP blocks an
  // inline `style` attribute, so a `style` prop would not take effect.
  useLayoutEffect(() => {
    for (const [id, ref] of [["widget:clock", clockRef], ["widget:status", statusRef]] as const) {
      const element = ref.current;
      const point = place(id);
      if (!element || !point) continue;
      element.style.position = "absolute";
      element.style.left = `${point.x}px`;
      element.style.top = `${point.y}px`;
    }
  }, [dragged, positions, defaults]);
  function begin(id: string, event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0) return;
    const element = event.currentTarget;
    const current = positions[id] ?? defaults[id] ?? { x: element.offsetLeft, y: element.offsetTop };
    const startX = event.clientX, startY = event.clientY;
    let moved = false;
    setDragged({ id, x: current.x, y: current.y });
    element.classList.add("is-dragging");
    // Listen on the window: pointer capture can fail for synthetic pointers, so
    // the drag must not depend on the events reaching the widget again.
    const onMove = (move: Event) => {
      const { clientX, clientY } = move as unknown as { clientX: number; clientY: number };
      const dx = clientX - startX, dy = clientY - startY;
      if (Math.abs(dx) + Math.abs(dy) > 4) moved = true;
      if (moved) setDragged({ id, x: current.x + dx, y: current.y + dy });
    };
    const onUp = (up: Event) => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      element.classList.remove("is-dragging");
      if (moved) {
        const host = hostRef.current;
        const bounds = { width: host?.clientWidth ?? 0, height: host?.clientHeight ?? 0 };
        const footprint = { width: element.offsetWidth, height: element.offsetHeight };
        const { clientX, clientY } = up as unknown as { clientX: number; clientY: number };
        const dx = clientX - startX, dy = clientY - startY;
        setDesktopPosition(id, clampDesktopPosition({ x: current.x + dx, y: current.y + dy }, bounds, footprint));
      }
      setDragged(null);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  const protection = systemStatus.protection === "active" ? t("status.protection.value") : t("status.protection.attention");
  return <section ref={hostRef} className="desktop-widgets widgets" aria-label={t("desktop.widgets.label")}>
    <article ref={clockRef}
      onPointerDown={(event) => begin("widget:clock", event)}
      className="desktop-widget widget true-glass rumahl-glass-host desktop-widget--clock">
      <p className="desktop-widget__eyebrow">{t("desktop.widgets.clock")}</p>
      <strong className="desktop-widget__time">{now ? new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now) : "--:--"}</strong>
      <span>{now ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(now) : ""}</span>
    </article>
    <article ref={statusRef}
      onPointerDown={(event) => begin("widget:status", event)}
      className="desktop-widget widget true-glass rumahl-glass-host desktop-widget--status">
      <p className="desktop-widget__eyebrow">{t("status.title")}</p>
      <strong>{protection}</strong>
      <span>{t("status.apps.label")}: {systemStatus.installedAppCount}</span>
    </article>
  </section>;
}
