import { useId, useState, type ReactNode } from "react";
import { colorStyle } from "../../tint";

export interface AccordionItem {
  id: string;
  title: ReactNode;
  content: ReactNode;
  disabled?: boolean;
}

export interface AccordionProps {
  items: readonly AccordionItem[];
  /** Allow several sections open at once (default: single). */
  multiple?: boolean;
  defaultOpen?: readonly string[];
  /** Custom accent colour. */
  color?: string | undefined;
  className?: string;
}

function Chevron() {
  return (
    <svg className="rumahl-accordion__chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="m6 9 6 6 6-6" />
    </svg>
  );
}

/**
 * Disclosure list; single-open by default, `multiple` for independent sections.
 * Each trigger is a real button with `aria-expanded`/`aria-controls`.
 */
export function Accordion({ items, multiple, defaultOpen = [], color, className = "" }: AccordionProps) {
  const base = useId();
  const [open, setOpen] = useState<readonly string[]>(defaultOpen);
  const toggle = (id: string) =>
    setOpen((previous) =>
      previous.includes(id) ? previous.filter((value) => value !== id) : multiple ? [...previous, id] : [id]
    );
  return (
    <div className={`rumahl-accordion ${className}`} style={colorStyle(color)}>
      {items.map((item) => {
        const id = `${base}-${item.id}`;
        const expanded = open.includes(item.id);
        return (
          <div key={item.id} className="rumahl-accordion__item" data-open={expanded ? "true" : undefined}>
            <button
              type="button"
              id={id}
              className="rumahl-accordion__trigger"
              aria-expanded={expanded}
              aria-controls={`${id}-panel`}
              disabled={item.disabled}
              onClick={() => toggle(item.id)}
            >
              <span>{item.title}</span>
              <Chevron />
            </button>
            <div className="rumahl-accordion__panel" role="region" aria-labelledby={id} id={`${id}-panel`} hidden={!expanded}>
              {item.content}
            </div>
          </div>
        );
      })}
    </div>
  );
}
