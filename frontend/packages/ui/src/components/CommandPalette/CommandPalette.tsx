import { useId, type CSSProperties, type ReactNode } from "react";
import { SearchIcon } from "../../icons";

/**
 * System search / command palette, styled like the rumahl OS surface: a quiet,
 * mostly-opaque panel with an integrated search row and a results list.
 */
export interface CommandPaletteProps {
  value: string;
  onChange: (value: string) => void;
  /** Accessible name for the dialog and input. */
  label?: string;
  placeholder?: string;
  /** Keyboard hint shown right of the input (e.g. `Esc`). */
  hint?: string;
  /** Optional right-hand preview pane. */
  preview?: ReactNode;
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
}

export function CommandPalette({
  value,
  onChange,
  label = "Command palette",
  placeholder = "Search…",
  hint,
  preview,
  children,
  className = "",
  style
}: CommandPaletteProps) {
  const inputId = useId();
  return (
    <div className={`rumahl-command-palette ${className}`} role="dialog" aria-label={label} style={style}>
      <div className="rumahl-command-palette__input">
        <SearchIcon aria-hidden="true" />
        <input
          id={inputId}
          value={value}
          aria-label={label}
          placeholder={placeholder}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => onChange(event.target.value)}
        />
        {hint ? <kbd className="rumahl-command-palette__hint">{hint}</kbd> : null}
      </div>
      <div className="rumahl-command-palette__body">
        <div className="rumahl-command-palette__results">{children}</div>
        {preview ? <div className="rumahl-command-palette__preview">{preview}</div> : null}
      </div>
    </div>
  );
}

export interface CommandPaletteItemProps {
  children: ReactNode;
  icon?: ReactNode;
  shortcut?: string;
  selected?: boolean;
  onSelect?: () => void;
}

/** A selectable row in a {@link CommandPalette}. */
export function CommandPaletteItem({ children, icon, shortcut, selected, onSelect }: CommandPaletteItemProps) {
  return (
    <button
      type="button"
      className="rumahl-command-palette__item"
      data-selected={selected ? "true" : undefined}
      onClick={onSelect}
    >
      {icon ? (
        <span className="rumahl-command-palette__icon" aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span className="rumahl-command-palette__text">
        <strong>{children}</strong>
      </span>
      {shortcut ? <span className="rumahl-command-palette__shortcut">{shortcut}</span> : null}
    </button>
  );
}

/** A titled group inside the palette results. */
export function CommandPaletteSection({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <div className="rumahl-command-palette__section-wrap">
      <div className="rumahl-command-palette__section">{title}</div>
      {children}
    </div>
  );
}

/** Separator between palette groups. */
export function CommandPaletteSeparator() {
  return <div className="rumahl-command-palette__separator" role="separator" />;
}

/** Empty state for no results. */
export function CommandPaletteEmpty({ children }: { children: ReactNode }) {
  return <p className="rumahl-command-palette__empty">{children}</p>;
}
