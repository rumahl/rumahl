import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n";
import { ChevronRightIcon, PencilIcon } from "./icons";
import { ContextMenu } from "./ContextMenu";

export interface PathCrumb {
  key: string;
  label: string;
}

/** One navigable folder offered by a crumb's dropdown. */
export interface PathOption {
  label: string;
  onSelect: () => void;
}

interface PathBarProps {
  /** Canonical rumahl path of the current folder, used while editing. */
  path: string;
  /** Crumbs from the area root to the current folder. */
  crumbs: readonly PathCrumb[];
  /** Windows-style crumbs are an advanced-mode feature; guided keeps the input. */
  advanced: boolean;
  busy?: boolean;
  /** Navigates to the crumb at `index`. */
  onNavigate: (index: number) => void;
  /** The subfolders of the crumb at `index`, for its dropdown. */
  onOptions: (index: number) => Promise<readonly PathOption[]>;
  onSubmit: (input: string) => void;
}

/**
 * The Files path bar. In advanced mode it works like the Windows Explorer
 * address bar: a crumb per folder, each with a dropdown of its subfolders, and
 * an edit toggle that swaps the crumbs for the raw `rumahl://` input. In guided
 * mode it stays the plain input. Either way the service resolves what is
 * typed; the bar never interprets a host path.
 */
export function PathBar({ path, crumbs, advanced, busy = false, onNavigate, onOptions, onSubmit }: PathBarProps) {
  const { t } = useI18n();
  const [value, setValue] = useState(path);
  const [focused, setFocused] = useState(false);
  const [editing, setEditing] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; index: number; items: readonly PathOption[]; anchor: HTMLElement } | null>(null);
  const typed = useRef(path);
  const input = useRef<HTMLInputElement>(null);
  const crumbsMode = advanced && !editing;

  // A navigation or a deep link replaces the draft; an active edit is kept.
  useEffect(() => {
    if (focused) return;
    typed.current = path;
    setValue(path);
  }, [path, focused]);
  useEffect(() => {
    if (!advanced) setEditing(false);
  }, [advanced]);

  const submit = () => {
    if (busy) return;
    onSubmit(typed.current);
    setEditing(false);
    input.current?.blur();
  };

  const openOptions = async (index: number, anchor: HTMLElement) => {
    if (busy) return;
    const bounds = anchor.getBoundingClientRect();
    const pending = { x: bounds.left, y: bounds.bottom + 6, index, items: [] as readonly PathOption[], anchor };
    setMenu(pending);
    const options = await onOptions(index);
    // Only apply while the same dropdown is still open.
    setMenu((current) => (current && current.index === index && current.anchor === anchor ? { ...current, items: options } : current));
  };

  return <>
    {crumbsMode ? <div className="files-path" data-busy={busy ? "true" : undefined}>
      {crumbs.map((crumb, index) => {
        const current = index === crumbs.length - 1;
        return <span key={crumb.key} className="files-path__crumb">
          <button
            type="button"
            className={`files-path__segment${current ? " files-path__segment--current" : ""}`}
            aria-current={current ? "page" : undefined}
            disabled={busy}
            onClick={() => onNavigate(index)}
          >{crumb.label}</button>
          <button
            type="button"
            className="files-path__chevron"
            aria-haspopup="menu"
            aria-expanded={menu?.index === index}
            aria-label={t("files.subfolders", { name: crumb.label })}
            title={t("files.subfolders", { name: crumb.label })}
            disabled={busy}
            onClick={(event) => void openOptions(index, event.currentTarget)}
          ><ChevronRightIcon /></button>
        </span>;
      })}
      <span className="files-path__spacer" />
      <button
        type="button"
        className="files-path__edit"
        aria-label={t("files.editPath")}
        title={t("files.editPath")}
        disabled={busy}
        onClick={() => {
          setEditing(true);
          requestAnimationFrame(() => input.current?.focus());
        }}
      ><PencilIcon /></button>
    </div> : null}
    {crumbsMode ? null : <form className="files-address__form" onSubmit={(event) => { event.preventDefault(); submit(); }}>
      <input
        ref={input}
        className="files-address__input"
        type="text"
        value={value}
        disabled={busy}
        spellCheck={false}
        autoComplete="off"
        autoCapitalize="off"
        aria-label={t("files.address")}
        placeholder={t("files.addressHint")}
        onFocus={() => setFocused(true)}
        onBlur={() => { setFocused(false); setValue(path); if (advanced) setEditing(false); }}
        onChange={(event) => {
          typed.current = event.target.value;
          setValue(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key !== "Escape") return;
          event.stopPropagation();
          typed.current = path;
          setValue(path);
          setFocused(false);
          input.current?.blur();
        }}
      />
      <button className="files-address__go" type="submit" disabled={busy} aria-label={t("files.go")}>
        {t("files.go")}
      </button>
    </form>}
    {menu ? <ContextMenu
      x={menu.x}
      y={menu.y}
      label={t("files.subfoldersLabel")}
      restoreFocus={menu.anchor}
      onClose={() => setMenu(null)}
      items={menu.items.map((option) => ({
        label: option.label,
        onSelect: () => {
          option.onSelect();
          setMenu(null);
        },
      }))}
    /> : null}
  </>;
}
