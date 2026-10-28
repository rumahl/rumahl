import { useI18n } from "../../i18n";
import { extensionOf } from "./fileTypes";
import { FileIcon, FolderArtwork } from "./icons";
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
  onSelect: (item: Item, event: SelectionEvent) => void;
  onOpen: (item: Item) => void;
  onRenameCommit: (item: Item, name: string) => void;
  onRenameCancel: () => void;
  onMenu: (item: Item, x: number, y: number) => void;
  onDragStart: (item: Item) => void;
  onDragEnd: () => void;
  onDragOver: (key: string | null) => void;
  onDropOn: (folder: Item) => void;
}

function RenameInput({ item, onCommit, onCancel }: { item: Item; onCommit: (item: Item, name: string) => void; onCancel: () => void }) {
  return <input className="rumahl-input files-rename" autoFocus defaultValue={item.name}
    onClick={(event) => event.stopPropagation()}
    onBlur={(event) => onCommit(item, event.target.value)}
    onKeyDown={(event) => { if (event.key === "Enter") (event.target as HTMLInputElement).blur(); if (event.key === "Escape") onCancel(); }} />;
}

function rowHandlers(props: ViewProps, item: Item) {
  const canDrop = item.directory && !item.host && props.draggingKey !== null && props.draggingKey !== item.key;
  return {
    draggable: !item.host,
    onClick: (event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) => props.onSelect(item, event),
    onDoubleClick: () => props.onOpen(item),
    onContextMenu: (event: { preventDefault: () => void; clientX: number; clientY: number }) => { event.preventDefault(); props.onMenu(item, event.clientX, event.clientY); },
    onDragStart: (event: { dataTransfer: DataTransfer | null }) => { if (item.host) return; if (event.dataTransfer) { event.dataTransfer.effectAllowed = "move"; event.dataTransfer.setData("text/plain", item.key); } props.onDragStart(item); },
    onDragEnd: () => props.onDragEnd(),
    onDragOver: (event: { preventDefault: () => void }) => { if (canDrop) { event.preventDefault(); props.onDragOver(item.key); } },
    onDragLeave: () => { if (props.dragOverKey === item.key) props.onDragOver(null); },
    onDrop: (event: { preventDefault: () => void }) => { if (canDrop) { event.preventDefault(); props.onDropOn(item); } },
  };
}

function classes(props: ViewProps, item: Item): string {
  return [
    props.selected.has(item.key) ? "is-selected" : "",
    props.draggingKey === item.key ? "is-dragging" : "",
    props.dragOverKey === item.key ? "is-drop-target" : "",
  ].filter(Boolean).join(" ");
}

export function FileGrid(props: ViewProps) {
  return <ul className="files-grid">{props.items.map((item, index) => <li key={item.key}
    data-file-key={item.key}
    data-kind={item.app || item.appPath ? "app" : item.directory ? "folder" : "file"}
    className={`files-tile ${classes(props, item)}`}
    style={{ animationDelay: `${Math.min(index, 12) * 24}ms` }}
    {...rowHandlers(props, item)}>
    <span className="files-tile__icon" aria-hidden="true">{item.icon ? <span className="files-tile__app">{item.icon}</span> : item.directory ? <FolderArtwork className="files-folder-art" /> : <FileIcon className="files-icon-file" />}</span>
    <span className="files-tile__copy">
      {props.renameKey === item.key
        ? <RenameInput item={item} onCommit={props.onRenameCommit} onCancel={props.onRenameCancel} />
        : <span className="files-tile__name" title={item.label ?? item.name}>{item.label ?? item.name}</span>}
    </span>
  </li>)}</ul>;
}

function itemType(item: Item, t: ReturnType<typeof useI18n>["t"]): string {
  if (item.app || item.appPath) return t("files.app");
  if (item.directory) return t("files.folder");
  const extension = extensionOf(item.name);
  return extension ? extension.toLocaleUpperCase() : t("files.file");
}

export function FileList(props: ViewProps) {
  const { t, locale } = useI18n();
  return <div className="files-list" role="table" aria-label={t("files.list")}>
    <div className="files-list__head" role="row">
      <span role="columnheader">{t("files.name")}</span>
      <span role="columnheader">{t("files.type")}</span>
      <span role="columnheader">{t("files.size")}</span>
      <span role="columnheader">{t("files.modified")}</span>
    </div>
    <ul role="rowgroup">{props.items.map((item) => <li key={item.key}
      data-file-key={item.key}
      data-kind={item.app || item.appPath ? "app" : item.directory ? "folder" : "file"}
      className={`files-row ${classes(props, item)}`}
      role="row"
      aria-selected={props.selected.has(item.key)}
      {...rowHandlers(props, item)}>
      <span className="files-row__name" role="cell">
        {item.icon ? <span className="files-row__app">{item.icon}</span> : item.directory ? <FolderArtwork className="files-folder-art" /> : <FileIcon className="files-icon-file" />}
        {props.renameKey === item.key
          ? <RenameInput item={item} onCommit={props.onRenameCommit} onCancel={props.onRenameCancel} />
          : <span title={item.label ?? item.name}>{item.label ?? item.name}</span>}
      </span>
      <span className="files-row__kind" role="cell">{itemType(item, t)}</span>
      <span className="files-row__meta" role="cell">{item.directory ? "—" : formatSize(item.size)}</span>
      <span className="files-row__meta" role="cell">{item.modified ? new Date(item.modified * 1000).toLocaleDateString(locale, { day: "2-digit", month: "short", year: "numeric" }) : "—"}</span>
    </li>)}</ul>
  </div>;
}
