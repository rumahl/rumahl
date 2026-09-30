import { useEffect, useId, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { useControlPopover } from "./useControlPopover";

export function dateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
export function parseDate(value: string): Date | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const date = new Date(`${value}T12:00:00`);
  return Number.isFinite(date.getTime()) && dateKey(date) === value ? date : null;
}
export function shiftMonth(date: Date, offset: number): Date {
  const last = new Date(date.getFullYear(), date.getMonth() + offset + 1, 0, 12).getDate();
  return new Date(date.getFullYear(), date.getMonth() + offset, Math.min(date.getDate(), last), 12);
}
interface CalendarProps { value: string; onChange: (value: string) => void; min?: string; max?: string; }
export function RumahlCalendar({ value, onChange, min, max }: CalendarProps) {
  const { locale, t } = useI18n();
  const lower = min && parseDate(min) ? min : undefined;
  const upper = max && parseDate(max) ? max : undefined;
  const bound = (key: string) => lower && key < lower ? lower : upper && key > upper ? upper : key;
  const [focus, setFocus] = useState(() => bound(parseDate(value) ? value : dateKey(new Date())));
  const heading = useId();
  const grid = useRef<HTMLDivElement>(null);
  const focusAfterRender = useRef(false);
  useEffect(() => { setFocus(previous => bound(parseDate(value) ? value : previous)); }, [value, lower, upper]);
  useEffect(() => {
    if (focusAfterRender.current) { grid.current?.querySelector<HTMLButtonElement>(`[data-date="${focus}"]`)?.focus(); focusAfterRender.current = false; }
  }, [focus]);
  const current = parseDate(focus)!;
  const first = new Date(current.getFullYear(), current.getMonth(), 1, 12);
  const start = new Date(first); start.setDate(1 - (first.getDay() + 6) % 7);
  const days = Array.from({ length: 42 }, (_, i) => new Date(start.getFullYear(), start.getMonth(), start.getDate() + i, 12));
  const move = (date: Date, keyboard = false) => { focusAfterRender.current = keyboard; setFocus(bound(dateKey(date))); };
  return <div className="rumahl-calendar">
    <div className="rumahl-calendar__header">
      <button type="button" aria-label={t("calendar.previous")} disabled={!!lower && dateKey(new Date(current.getFullYear(), current.getMonth(), 0, 12)) < lower} onClick={() => move(shiftMonth(current, -1))}>‹</button>
      <strong id={heading} aria-live="polite">{new Intl.DateTimeFormat(locale, { month: "long", year: "numeric" }).format(current)}</strong>
      <button type="button" aria-label={t("calendar.next")} disabled={!!upper && dateKey(new Date(current.getFullYear(), current.getMonth() + 1, 1, 12)) > upper} onClick={() => move(shiftMonth(current, 1))}>›</button>
    </div>
    <div className="rumahl-calendar__week" aria-hidden="true">{days.slice(0, 7).map(day => <span key={dateKey(day)}>{new Intl.DateTimeFormat(locale, { weekday: "short" }).format(day)}</span>)}</div>
    <div className="rumahl-calendar__days" ref={grid} role="group" aria-labelledby={heading}>
      {days.map(day => { const key = dateKey(day); return <button key={key} type="button" data-date={key} data-outside={day.getMonth() !== current.getMonth() || undefined}
        disabled={!!((lower && key < lower) || (upper && key > upper))} tabIndex={key === focus ? 0 : -1}
        aria-label={new Intl.DateTimeFormat(locale, { dateStyle: "full" }).format(day)} aria-pressed={key === value} aria-current={key === dateKey(new Date()) ? "date" : undefined}
        onClick={() => { move(day); onChange(key); }} onKeyDown={event => {
          const next = new Date(day);
          const offsets: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7, Home: -(day.getDay() + 6) % 7, End: 6 - (day.getDay() + 6) % 7 };
          if (event.key in offsets) { event.preventDefault(); next.setDate(next.getDate() + offsets[event.key]!); move(next, true); }
          else if (event.key === "PageUp" || event.key === "PageDown") { event.preventDefault(); move(shiftMonth(day, (event.key === "PageUp" ? -1 : 1) * (event.shiftKey ? 12 : 1)), true); }
        }}>{day.getDate()}</button>; })}
    </div>
    <button type="button" className="rumahl-calendar__today" disabled={!!((lower && dateKey(new Date()) < lower) || (upper && dateKey(new Date()) > upper))} onClick={() => { const today = bound(dateKey(new Date())); setFocus(today); onChange(today); }}>{t("calendar.today")}</button>
  </div>;
}
export function RumahlDatePicker({ label, disabled, ...props }: CalendarProps & { label: string; disabled?: boolean }) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const { triggerRef, popupRef } = useControlPopover(open, setOpen, 280);
  const id = useId();
  const date = parseDate(props.value);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);
  useEffect(() => { if (open) popupRef.current?.querySelector<HTMLButtonElement>('[data-date][tabindex="0"]')?.focus(); }, [open, popupRef]);
  return <div className="rumahl-date-picker">
    <button ref={triggerRef} className="rumahl-select__trigger" type="button" disabled={disabled} aria-label={label} aria-expanded={open} aria-haspopup="dialog" aria-controls={open ? id : undefined} onClick={() => setOpen(state => !state)}>
      <span>{date ? new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(date) : t("calendar.choose")}</span><span aria-hidden="true">▦</span>
    </button>
    {open ? <div ref={popupRef} id={id} popover="auto" role="dialog" aria-label={label} className="rumahl-calendar-popover">
      <RumahlCalendar {...props} onChange={value => { props.onChange(value); setOpen(false); triggerRef.current?.focus(); }} />
    </div> : null}
  </div>;
}
