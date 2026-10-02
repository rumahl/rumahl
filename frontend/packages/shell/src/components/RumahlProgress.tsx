export function RumahlProgress({ value, label }: { value?: number; label: string }) {
  return <div className="rumahl-progress"><span>{label}</span><progress aria-label={label} max={100} value={value === undefined ? undefined : Math.max(0, Math.min(100, value))} />{value !== undefined ? <output>{Math.round(Math.max(0, Math.min(100, value)))}%</output> : null}</div>;
}
