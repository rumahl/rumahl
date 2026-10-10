import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { ShellSystemStatus } from "@rumahl/contracts";
import { useI18n } from "../../i18n";
import { useClock } from "../clock";
import { useGlassHost } from "../../glass-engine/useGlassEngine";
import { GLASS_MATERIALS } from "@rumahl/ui/glass";
import { clampDesktopPosition, setDesktopPosition, useDesktopPositions } from "./desktop-positions";

const WIDGET_GAP = 12;
const EDGE = 16;

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
  const drag = useRef<null | { id: string; pointer: number; element: HTMLElement; x: number; y: number; baseX: number; baseY: number; moved: boolean }>(null);
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
        "widget:clock": { x: Math.max(EDGE, width - clockWidth - EDGE), y: EDGE },
        "widget:status": { x: Math.max(EDGE, width - statusWidth - EDGE), y: EDGE + clockHeight + WIDGET_GAP }
      });
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  function place(id: string): { position: "absolute"; left: number; top: number } | undefined {
    const point = positions[id] ?? defaults[id];
    return point ? { position: "absolute", left: point.x, top: point.y } : undefined;
  }
  function begin(id: string, event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0) return;
    const element = event.currentTarget;
    const current = positions[id] ?? defaults[id] ?? { x: element.offsetLeft, y: element.offsetTop };
    drag.current = { id, pointer: event.pointerId, element, x: event.clientX, y: event.clientY, baseX: current.x, baseY: current.y, moved: false };
    element.setPointerCapture?.(event.pointerId);
    element.classList.add("is-dragging");
  }
  function move(event: ReactPointerEvent<HTMLElement>) {
    const entry = drag.current;
    if (!entry || entry.pointer !== event.pointerId) return;
    const dx = event.clientX - entry.x, dy = event.clientY - entry.y;
    if (Math.abs(dx) + Math.abs(dy) > 4) entry.moved = true;
    entry.element.style.left = `${entry.baseX + dx}px`;
    entry.element.style.top = `${entry.baseY + dy}px`;
  }
  function end(event: ReactPointerEvent<HTMLElement>) {
    const entry = drag.current;
    if (!entry || entry.pointer !== event.pointerId) return;
    entry.element.classList.remove("is-dragging");
    if (entry.moved) {
      const host = hostRef.current;
      const bounds = { width: host?.clientWidth ?? 0, height: host?.clientHeight ?? 0 };
      const footprint = { width: entry.element.offsetWidth, height: entry.element.offsetHeight };
      const dx = event.clientX - entry.x, dy = event.clientY - entry.y;
      setDesktopPosition(entry.id, clampDesktopPosition({ x: entry.baseX + dx, y: entry.baseY + dy }, bounds, footprint));
    }
    drag.current = null;
  }

  const protection = systemStatus.protection === "active" ? t("status.protection.value") : t("status.protection.attention");
  return <section ref={hostRef} className="desktop-widgets widgets" aria-label={t("desktop.widgets.label")}>
    <article ref={clockRef} style={place("widget:clock")}
      onPointerDown={(event) => begin("widget:clock", event)} onPointerMove={move} onPointerUp={end} onPointerCancel={end}
      className="desktop-widget widget true-glass rumahl-glass-host desktop-widget--clock">
      <p className="desktop-widget__eyebrow">{t("desktop.widgets.clock")}</p>
      <strong className="desktop-widget__time">{now ? new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now) : "--:--"}</strong>
      <span>{now ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(now) : ""}</span>
    </article>
    <article ref={statusRef} style={place("widget:status")}
      onPointerDown={(event) => begin("widget:status", event)} onPointerMove={move} onPointerUp={end} onPointerCancel={end}
      className="desktop-widget widget true-glass rumahl-glass-host desktop-widget--status">
      <p className="desktop-widget__eyebrow">{t("status.title")}</p>
      <strong>{protection}</strong>
      <span>{t("status.apps.label")}: {systemStatus.installedAppCount}</span>
    </article>
  </section>;
}
