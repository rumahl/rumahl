import { useEffect, useId, useRef, useState, type ReactNode, type KeyboardEvent } from "react";
import { WarningTriangleIcon } from "../icons";
import { RumahlTooltip } from "./RumahlTooltip";
import { useControlPopover } from "./useControlPopover";

export interface SelectOption { value: string; label: string; disabled?: boolean | undefined; warning?: string | undefined }

/** Custom warning triangle with the rumahl tooltip (not the browser's). */
function WarnMark({ text }: { text: string }) {
  return <RumahlTooltip content={text}>
    <span className="rumahl-select__warn" role="img" aria-label={text}>
      <WarningTriangleIcon />
    </span>
  </RumahlTooltip>;
}

/** Theme-aware listbox, including keyboard navigation and native light dismissal. */
export function RumahlSelect({ value, options, onChange, label, disabled, title, renderValue, renderOption }: {
  value: string;
  options: readonly SelectOption[];
  onChange: (value: string) => void;
  label: string;
  disabled?: boolean;
  title?: string;
  renderValue?: (value: string) => ReactNode;
  renderOption?: (value: string) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const { triggerRef, popupRef } = useControlPopover(open, setOpen);
  const id = useId();
  const search = useRef({ text: "", time: 0 });
  useEffect(() => {
    if (open) {
      const selected = popupRef.current?.querySelector<HTMLButtonElement>('[aria-selected="true"]');
      (selected ?? popupRef.current?.querySelector<HTMLButtonElement>('[role="option"]'))?.focus();
    }
  }, [open, popupRef]);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);

  function navigate(event: KeyboardEvent<HTMLDivElement>) {
    const buttons = Array.from(popupRef.current?.querySelectorAll<HTMLButtonElement>('[role="option"]:not([aria-disabled="true"])') ?? []);
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    let next = index;
    if (event.key === "ArrowDown") next = (index + 1) % buttons.length;
    else if (event.key === "ArrowUp") next = (index - 1 + buttons.length) % buttons.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = buttons.length - 1;
    else if (event.key === "Escape") {
      event.preventDefault(); event.stopPropagation(); setOpen(false); triggerRef.current?.focus(); return;
    } else if (event.key === "Tab") {
      // Resume the normal tab sequence from the trigger, not the popup's last option.
      triggerRef.current?.focus(); setOpen(false); return;
    } else if (event.key.length === 1 && event.key !== " " && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const now = Date.now();
      search.current.text = (now - search.current.time < 700 ? search.current.text : "") + event.key.toLocaleLowerCase();
      search.current.time = now;
      const match = buttons.findIndex(button => (button.dataset.label ?? "").toLocaleLowerCase().startsWith(search.current.text));
      if (match >= 0) next = match;
    } else return;
    event.preventDefault(); buttons[next]?.focus();
  }

  const currentOption = options.find(option => option.value === value);
  const current = currentOption?.label ?? value;
  return <div className="rumahl-select">
    <button ref={triggerRef} type="button" className="rumahl-select__trigger" title={title}
      aria-label={label} aria-haspopup="listbox" aria-controls={open ? id : undefined} aria-expanded={open}
      disabled={disabled || options.length === 0} onClick={() => setOpen(state => !state)}
      onKeyDown={event => { if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); setOpen(true); } }}>
      <span className="rumahl-select__value">
        <span className="rumahl-select__label">{renderValue ? renderValue(value) : current}</span>
        {currentOption?.warning ? <WarnMark text={currentOption.warning} /> : null}
      </span>
      <svg className="rumahl-select__chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m7 10 5 5 5-5" /></svg>
    </button>
    {open ? <div ref={popupRef} id={id} popover="auto" className="rumahl-select__menu" role="listbox" aria-label={label} onKeyDown={navigate}>
      {options.map(option => <button key={option.value} type="button" role="option" tabIndex={-1} data-label={option.label}
        aria-selected={option.value === value} aria-disabled={option.disabled || undefined}
        className={`rumahl-select__option${option.value === value ? " is-active" : ""}`}
        onClick={() => { if (option.disabled) return; onChange(option.value); setOpen(false); triggerRef.current?.focus(); }}>
        {renderOption ? renderOption(option.value) : option.label}
        {option.warning ? <WarnMark text={option.warning} /> : null}
      </button>)}
    </div> : null}
  </div>;
}
