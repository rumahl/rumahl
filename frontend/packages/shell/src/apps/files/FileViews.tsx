import { useRef, type DragEvent as ReactDragEvent, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent } from "react";
import { useI18n } from "../../i18n";
import { extensionOf } from "./fileTypes";
import { FileArtwork, FolderArtwork, MoreIcon } from "./icons";
import type { Item } from "./types";
import type { SelectionEvent } from "./useSelection";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes.toLocaleString()} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = units[0]!;
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index]!;
  }
  return `${value >= 10 ? Math.round(value) : value.toFixed(1)} ${unit}`;
}

export interface ViewProps {
  items: readonly Item[];
  selected: ReadonlySet<string>;
  renameKey: string | null;
  draggingKey: string | null;
  dragOverKey: string | null;
  writable: boolean;
  /** Host listings carry real modification times; personal entries do not. */
  hostView: boolean;
  /** Phones open on a single tap; desktop keeps click-to-select. */
  openOnTap?: boolean;
  onSelect: (item: Item, event: SelectionEvent) => void;
  onOpen: (item: Item) => void;
  onSelectAll: () => void;
  onRename: (item: Item) => void;
  onDelete: () => void;
  onRenameCommit: (item: Item, name: string) => void;
  onRenameCancel: () => void;
  onMenu: (item: Item, x: number, y: number, anchor: HTMLElement) => void;
  onDragStart: (item: Item) => void;
  onDragEnd: () => void;
  onDragOver: (key: string | null) => void;
  onDropOn: (folder: Item) => void;
}

function RenameInput({ item, onCommit, onCancel }: { item: Item; onCommit: (item: Item, name: string) => void; onCancel: () => void }) {
  const { t } = useI18n();
  return <input className="rumahl-input files-rename" aria-label={t("files.rename")} autoFocus defaultValue={item.name}
    onFocus={(event) => event.currentTarget.select()}
    onClick={(event) => event.stopPropagation()}
    onBlur={(event) => onCommit(item, event.target.value)}
    onKeyDown={(event) => { if (event.key === "Enter") (event.target as HTMLInputElement).blur(); if (event.key === "Escape") onCancel(); }} />;
}

function itemType(item: Item, t: ReturnType<typeof useI18n>["t"]): string {
  if (item.app || item.appPath) return t("files.app");
  if (item.directory) return t("files.folder");
  const extension = extensionOf(item.name);
  return extension ? extension.toLocaleUpperCase() : t("files.file");
}

function itemSubtitle(item: Item, t: ReturnType<typeof useI18n>["t"]): string {
  if (item.app || item.appPath) return t("files.app");
  if (item.directory) return item.children == null ? t("files.folder") : t("files.items", { count: item.children });
  return `${itemType(item, t)} · ${formatSize(item.size)}`;
}

function selectionEvent(event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }): SelectionEvent {
  return { ctrlKey: event.ctrlKey, metaKey: event.metaKey, shiftKey: event.shiftKey };
}

/**
 * One roving-tabindex keyboard model for both views. Only the selected (or
 * first) item is in the tab order; arrows move selection and focus, Shift
 * extends, Ctrl/Meta toggles, Enter/Space opens, F2 renames and Delete asks the
 * shell to remove the selection.
 */
function useItemKeyboard(props: ViewProps) {
  const refs = useRef(new Map<string, HTMLElement>());
  const roving = props.items.find((item) => props.selected.has(item.key)) ?? props.items[0];
  const rovingKey = roving?.key ?? null;

  const focus = (key: string) => {
    const element = refs.current.get(key);
    element?.focus({ preventScroll: true });
    element?.scrollIntoView?.({ block: "nearest" });
  };
  const move = (event: ReactKeyboardEvent<HTMLElement>, index: number) => {
    const key = event.key;
    const last = props.items.length - 1;
    const nextIndex = key === "Home" ? 0
      : key === "End" ? last
        : key === "ArrowDown" || key === "ArrowRight" ? Math.min(last, index + 1)
          : Math.max(0, index - 1);
    const next = props.items[nextIndex];
    if (!next) return;
    event.preventDefault();
    event.stopPropagation();
    props.onSelect(next, selectionEvent(event));
    focus(next.key);
  };

  function onKeyDown(event: ReactKeyboardEvent<HTMLElement>, item: Item, index: number) {
    const target = event.target as HTMLElement;
    if (target.closest("input, textarea, select, [contenteditable='true'], button, a")) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
      event.preventDefault();
      event.stopPropagation();
      props.onSelectAll();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "ArrowLeft" || event.key === "ArrowRight" || event.key === "Home" || event.key === "End") {
      move(event, index);
      return;
    }
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      const element = refs.current.get(item.key);
      if (element) {
        const bounds = element.getBoundingClientRect();
        event.preventDefault();
        event.stopPropagation();
        props.onMenu(item, bounds.right, bounds.bottom, element);
      }
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === " " && (event.ctrlKey || event.metaKey)) props.onSelect(item, selectionEvent(event));
      else props.onOpen(item);
      return;
    }
    if (!props.writable) return;
    if (event.key === "F2" && !item.appPath && !item.appFolder) {
      event.preventDefault();
      event.stopPropagation();
      props.onRename(item);
      return;
    }
    if (event.key === "Delete") {
      event.preventDefault();
      event.stopPropagation();
      props.onDelete();
    }
  }

  const register = (key: string) => (element: HTMLElement | null) => {
    if (element) refs.current.set(key, element);
    else refs.current.delete(key);
  };

  return { rovingKey, register, onKeyDown };
}

function rowHandlers(props: ViewProps, item: Item, keyboard: ReturnType<typeof useItemKeyboard>, index: number) {
  const canDrop = item.directory && !item.host && props.draggingKey !== null && props.draggingKey !== item.key;
  return {
    tabIndex: keyboard.rovingKey === item.key ? 0 : -1,
    onKeyDown: (event: ReactKeyboardEvent<HTMLElement>) => keyboard.onKeyDown(event, item, index),
    draggable: !item.host,
    onClick: (event: ReactMouseEvent<HTMLElement>) => {
      if (props.openOnTap) {
        props.onOpen(item);
        return;
      }
      props.onSelect(item, selectionEvent(event));
      event.currentTarget.querySelector<HTMLElement>("[data-file-focus]")?.focus({ preventScroll: true });
    },
    onDoubleClick: () => props.onOpen(item),
    onContextMenu: (event: ReactMouseEvent<HTMLElement>) => {
      event.preventDefault();
      props.onMenu(item, event.clientX, event.clientY, event.currentTarget);
    },
    onDragStart: (event: ReactDragEvent<HTMLElement>) => {
      if (item.host) return;
      if (event.dataTransfer) {
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", item.key);
      }
      props.onDragStart(item);
    },
    onDragEnd: () => props.onDragEnd(),
    onDragOver: (event: ReactDragEvent<HTMLElement>) => {
      if (canDrop) {
        event.preventDefault();
        props.onDragOver(item.key);
      }
    },
    onDragLeave: () => { if (props.dragOverKey === item.key) props.onDragOver(null); },
    onDrop: (event: ReactDragEvent<HTMLElement>) => {
      if (canDrop) {
        event.preventDefault();
        props.onDropOn(item);
      }
    }
  };
}

function classes(props: ViewProps, item: Item): string {
  return [
    props.selected.has(item.key) ? "is-selected" : "",
    props.draggingKey === item.key ? "is-dragging" : "",
    props.dragOverKey === item.key ? "is-drop-target" : "",
  ].filter(Boolean).join(" ");
}

/** Pointer/touch path for the same actions the right-click menu exposes. */
function ItemMenuButton({ item, onMenu, tabIndex }: { item: Item; onMenu: ViewProps["onMenu"]; tabIndex: number }) {
  const { t } = useI18n();
  const open = (event: ReactMouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const bounds = event.currentTarget.getBoundingClientRect();
    onMenu(item, bounds.right, bounds.bottom, event.currentTarget);
  };
  return <button
    type="button"
    className="files-item-menu"
    aria-haspopup="menu"
    aria-label={`${t("files.actions")}: ${item.label ?? item.name}`}
    title={t("files.actions")}
    tabIndex={tabIndex}
    onClick={open}
    onContextMenu={open}
    onDragStart={(event) => { event.preventDefault(); event.stopPropagation(); }}
    onPointerDown={(event) => event.stopPropagation()}>
    <MoreIcon />
  </button>;
}

export function FileGrid(props: ViewProps) {
  const { t } = useI18n();
  const keyboard = useItemKeyboard(props);
  return <ul className="files-grid" aria-label={`${t("files.title")}, ${t("files.grid")}`}>{props.items.map((item, index) => {
    const label = item.label ?? item.name;
    return <li key={item.key} className="files-grid-cell">
      <div
        ref={keyboard.register(item.key)}
        data-file-key={item.key}
        data-file-focus
        data-kind={item.app || item.appPath ? "app" : item.directory ? "folder" : "file"}
        className={`files-tile ${classes(props, item)}`}
        role="button"
        aria-pressed={props.selected.has(item.key)}
        aria-label={`${label}, ${itemType(item, t)}`}
        style={{ animationDelay: `${Math.min(index, 12) * 24}ms` }}
        {...rowHandlers(props, item, keyboard, index)}>
        <span className="files-tile__icon" aria-hidden="true">{item.icon ? <span className="files-tile__app">{item.icon}</span> : item.directory ? <FolderArtwork className="files-folder-art" count={item.children} /> : <FileArtwork className="files-icon-file" name={item.name} />}</span>
        <span className="files-tile__copy">
          {props.renameKey === item.key
            ? <RenameInput item={item} onCommit={props.onRenameCommit} onCancel={props.onRenameCancel} />
            : <span className="files-tile__name" title={label}>{label}</span>}
        </span>
      </div>
      <span className="files-grid-cell__action">
        <ItemMenuButton item={item} onMenu={props.onMenu} tabIndex={-1} />
      </span>
    </li>;
  })}</ul>;
}

export function FileList(props: ViewProps) {
  const { t, locale } = useI18n();
  const keyboard = useItemKeyboard(props);
  return <div className={`files-list${props.hostView ? "" : " files-list--no-modified"}`} role="table" aria-label={`${t("files.title")}, ${t("files.list")}`}>
    <div className="files-list__head" role="row">
      <span className="files-list__name" role="columnheader">{t("files.name")}</span>
      <span className="files-list__kind" role="columnheader">{t("files.type")}</span>
      <span className="files-list__size" role="columnheader">{t("files.size")}</span>
      {props.hostView ? <span className="files-list__modified" role="columnheader">{t("files.modified")}</span> : null}
      <span className="files-list__actions" role="columnheader"><span className="visually-hidden">{t("files.actions")}</span></span>
    </div>
    <ul role="rowgroup">{props.items.map((item, index) => {
      const label = item.label ?? item.name;
      const handlers = rowHandlers(props, item, keyboard, index);
      const { tabIndex, onKeyDown, ...row } = handlers;
      return <li key={item.key}
        data-file-key={item.key}
        data-kind={item.app || item.appPath ? "app" : item.directory ? "folder" : "file"}
        className={`files-row ${classes(props, item)}`}
        role="row"
        aria-selected={props.selected.has(item.key)}
        {...row}>
        <span className="files-row__name" role="cell" data-file-focus ref={keyboard.register(item.key)} tabIndex={tabIndex} onKeyDown={onKeyDown}>
          {item.icon ? <span className="files-row__app">{item.icon}</span> : item.directory ? <FolderArtwork className="files-folder-art" count={item.children} /> : <FileArtwork className="files-icon-file" name={item.name} />}
          <span className="files-row__text">
            {props.renameKey === item.key
              ? <RenameInput item={item} onCommit={props.onRenameCommit} onCancel={props.onRenameCancel} />
              : <>
                  <span className="files-row__label" title={label}>{label}</span>
                  <span className="files-row__sub">{itemSubtitle(item, t)}</span>
                </>}
          </span>
        </span>
        <span className="files-row__kind" role="cell">{itemType(item, t)}</span>
        <span className="files-row__meta files-row__size" role="cell">{item.directory ? "—" : formatSize(item.size)}</span>
        {props.hostView ? <span className="files-row__meta files-row__modified" role="cell">{item.modified ? new Date(item.modified * 1000).toLocaleDateString(locale, { day: "2-digit", month: "short", year: "numeric" }) : "—"}</span> : null}
        <span className="files-row__action" role="cell" onClick={event => event.stopPropagation()} onDoubleClick={event => event.stopPropagation()}>
          <ItemMenuButton item={item} onMenu={props.onMenu} tabIndex={-1} />
        </span>
      </li>;
    })}</ul>
  </div>;
}
