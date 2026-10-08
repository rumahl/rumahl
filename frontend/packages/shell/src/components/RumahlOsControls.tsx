import { useId, useRef, type ReactNode } from "react";
import { Button } from "./Button";
import { RumahlInputGroup } from "./RumahlInputs";

export function RumahlSearch({ label, clearLabel, value, onChange, disabled = false }: { label: string; clearLabel: string; value: string; onChange: (value: string) => void; disabled?: boolean }) {
  const ref = useRef<HTMLInputElement>(null);
  return <RumahlInputGroup ref={ref} type="search" aria-label={label} placeholder={label} value={value} disabled={disabled} onChange={event => onChange(event.target.value)}
    suffix={value ? <Button size="sm" disabled={disabled} aria-label={clearLabel} onClick={() => { onChange(""); ref.current?.focus(); }}>×</Button> : undefined} />;
}

export function RumahlStepper({ label, decreaseLabel, increaseLabel, value, onChange, min = 0, max = 100, step = 1, disabled = false }: {
  label: string; decreaseLabel: string; increaseLabel: string; value: number; onChange: (value: number) => void; min?: number; max?: number; step?: number; disabled?: boolean;
}) {
  const clamp = (next: number) => Math.min(max, Math.max(min, Number(next.toFixed(10))));
  return <RumahlInputGroup type="number" aria-label={label} value={value} min={min} max={max} step={step} disabled={disabled}
    onChange={event => { const next = event.target.valueAsNumber; if (Number.isFinite(next)) onChange(clamp(next)); }}
    prefix={<Button size="sm" aria-label={decreaseLabel} disabled={disabled || value <= min} onClick={() => onChange(clamp(value - step))}>−</Button>}
    suffix={<Button size="sm" aria-label={increaseLabel} disabled={disabled || value >= max} onClick={() => onChange(clamp(value + step))}>+</Button>} />;
}

export function RumahlBadge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "success" | "warning" | "danger" }) {
  return <span className={`rumahl-badge rumahl-badge--${tone}`}>{children}</span>;
}
export function RumahlNotice({ title, children, tone = "info", action }: { title: string; children: ReactNode; tone?: "info" | "warning" | "danger"; action?: ReactNode }) {
  return <div className={`rumahl-notice rumahl-notice--${tone}`}><span className="rumahl-notice__symbol" aria-hidden="true">{tone === "info" ? "i" : "!"}</span><div><strong>{title}</strong><p>{children}</p></div>{action}</div>;
}

export function RumahlTabs({ label, tabs, value, onChange }: { label: string; tabs: readonly { id: string; label: string; content: ReactNode; disabled?: boolean }[]; value: string; onChange: (value: string) => void }) {
  const id = useId();
  return <div className="rumahl-tabs">
    <div className="rumahl-button-group" role="tablist" aria-label={label} onKeyDown={event => {
      if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
      const enabled = tabs.filter(tab => !tab.disabled);
      const current = enabled.findIndex(tab => `${id}-${tab.id}` === (event.target as HTMLElement).id);
      if (current < 0) return;
      event.preventDefault();
      const next = enabled[event.key === "Home" ? 0 : event.key === "End" ? enabled.length - 1 : (current + (event.key === "ArrowRight" ? 1 : -1) + enabled.length) % enabled.length];
      if (next) { onChange(next.id); document.getElementById(`${id}-${next.id}`)?.focus(); }
    }}>
      {tabs.map(tab => <button key={tab.id} type="button" role="tab" id={`${id}-${tab.id}`} aria-controls={`${id}-${tab.id}-panel`} aria-selected={value === tab.id} disabled={tab.disabled} tabIndex={value === tab.id ? 0 : -1} onClick={() => onChange(tab.id)}>{tab.label}</button>)}
    </div>
    {tabs.map(tab => <div key={tab.id} role="tabpanel" id={`${id}-${tab.id}-panel`} aria-labelledby={`${id}-${tab.id}`} hidden={value !== tab.id} tabIndex={0}>{tab.content}</div>)}
  </div>;
}
