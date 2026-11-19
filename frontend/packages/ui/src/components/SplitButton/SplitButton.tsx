import { useState, type ReactNode } from "react";
import { Button, type ButtonVariant } from "../Button/Button";
import { Menu, MenuItem } from "../Menu/Menu";
import type { ControlSize } from "../../size";

export interface SplitAction {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
}

export interface SplitButtonProps {
  /** Primary action content (the main button). */
  children: ReactNode;
  actions: readonly SplitAction[];
  variant?: ButtonVariant;
  size?: ControlSize | undefined;
  /** Custom accent colour for both parts. */
  color?: string | undefined;
  /** Explicit fill colour for both parts. */
  backgroundColor?: string | undefined;
  /** Accessible name for the caret. */
  menuLabel?: string;
  className?: string;
}

/** Primary action plus a caret that opens a menu of related actions. */
export function SplitButton({
  children,
  actions,
  variant = "primary",
  size = "md",
  color,
  backgroundColor,
  menuLabel = "More actions",
  className = ""
}: SplitButtonProps) {
  const [open, setOpen] = useState(false);
  const tone = {
    ...(color ? { color } : {}),
    ...(backgroundColor ? { backgroundColor } : {})
  };
  return (
    <div className={`rumahl-split-button ${className}`} style={{ position: "relative", display: "inline-flex" }}>
      <Button variant={variant} size={size} {...tone}>
        {children}
      </Button>
      <Button
        variant={variant}
        size={size}
        className="rumahl-split-button__caret"
        {...tone}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={menuLabel}
        onClick={() => setOpen((value) => !value)}
      >
        ▾
      </Button>
      {open ? (
        <Menu
          style={{ position: "absolute", top: "calc(100% + 6px)", insetInlineEnd: 0, zIndex: 120 }}
          onBlur={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node)) setOpen(false);
          }}
        >
          {actions.map((action) => (
            <MenuItem
              key={action.label}
              disabled={action.disabled}
              onClick={() => {
                action.onSelect();
                setOpen(false);
              }}
            >
              {action.label}
            </MenuItem>
          ))}
        </Menu>
      ) : null}
    </div>
  );
}
