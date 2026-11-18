import { useCallback, useEffect, useMemo, useRef, useState, type ClipboardEvent as ReactClipboardEvent, type KeyboardEvent, type MouseEvent as ReactMouseEvent } from "react";
import { Button } from "../../components/Button";
import { createPortal } from "react-dom";
import { RumahlButtonGroup, RumahlDialog } from "@rumahl/ui/components";
import { DocumentIcon, RefreshIcon } from "../../icons";
import { useAppTopbarHost } from "../../components/ProtectedWindow";
import { EditorToolIcon, type EditorTool } from "./EditorToolIcon";
import { useI18n } from "../../i18n";
import { browserProfile } from "../../preferences/storage";
import { useWindowLocation } from "../../routing/window-location";
import { useShell } from "../../shell/ShellContext";
import { hostContentUrl } from "../fsClient";
import { MarkdownDocument, htmlToMarkdown, isMarkdownFile, markdownToHtml } from "./markdown";
import "./editor.css";

/**
 * Native text editor.
 *
 * Markdown files open in a Word-like rich surface: headings, bold, lists and
 * quotes appear formatted while writing, so the author never types `#`, `**` or
 * `---`. The file stays Markdown on disk; the rich DOM is serialized back on
 * save. Plain files keep a monospace textarea. Host files are read-only.
 */
const MAX_EDIT_BYTES = 2 * 1024 * 1024;
const textDecoder = new TextDecoder("utf-8", { fatal: true });
type EditorMode = "edit" | "split" | "preview";
type BlockFormat = "p" | "h1" | "h2" | "h3" | "blockquote";

interface EditorTarget {
  name: string;
  id?: string;
  host?: { area: "apps" | "system"; path: string };
}

function readTarget(search: string): EditorTarget | null {
  const params = new URLSearchParams(search);
  const name = params.get("name")?.slice(0, 255) ?? "";
  const id = params.get("id");
  if (id && /^[a-zA-Z0-9._-]{1,255}$/.test(id)) return { name: name || id, id };
  const path = params.get("path");
  if (params.get("source") === "host" && path !== null && path.length <= 2048) {
    const area = params.get("area") === "system" ? "system" : "apps";
    return { name: name || path.split("/").at(-1) || path, host: { area, path } };
  }
  return null;
}

function decodeText(buffer: ArrayBuffer): string | null {
  if (buffer.byteLength > MAX_EDIT_BYTES) throw new Error("limit");
  try {
    const text = textDecoder.decode(buffer);
    return text.includes("\u0000") ? null : text;
  } catch {
    return null;
  }
}

type LoadState = "loading" | "ready" | "missing" | "binary" | "limit" | "unavailable";

export function EditorApp() {
  const { t } = useI18n();
  const { live, open } = useShell();
  const topbarHost = useAppTopbarHost();
  const location = useWindowLocation();
  const target = useMemo(() => readTarget((location ?? "").split("?")[1] ?? ""), [location]);
  const markdown = target ? isMarkdownFile(target.name) : false;
  const [state, setState] = useState<LoadState>("loading");
  const [text, setText] = useState("");
  const [savedText, setSavedText] = useState("");
  const [wrap, setWrap] = useState(true);
  const [mode, setMode] = useState<EditorMode>("edit");
  const [block, setBlock] = useState<BlockFormat>("p");
  const [format, setFormat] = useState({ bold: false, italic: false, strike: false });
  const [linkOpen, setLinkOpen] = useState(false);
  const [linkValue, setLinkValue] = useState("https://");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [reloadConfirm, setReloadConfirm] = useState(false);
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  const area = useRef<HTMLTextAreaElement>(null);
  const rich = useRef<HTMLDivElement>(null);
  const savedRange = useRef<Range | null>(null);
  const loaded = useRef<string | null>(null);
  const generation = useRef(0);

  const readOnly = target?.host !== undefined || target?.id === undefined;
  const dirty = !readOnly && text !== savedText;
  const words = useMemo(() => (text.trim() ? text.trim().split(/\s+/).length : 0), [text]);
  const showWrite = mode !== "preview";
  const showPreview = markdown && mode !== "edit";
  const richVisible = markdown && showWrite;

  useEffect(() => { setMode("edit"); }, [target?.name]);

  const load = useCallback(async () => {
    if (!target) return;
    const request = live?.request;
    if (!request) { setState("unavailable"); return; }
    const current = ++generation.current;
    setState("loading");
    setSaveError(false);
    loaded.current = null;
    try {
      const device = browserProfile().id;
      const url = target.host
        ? hostContentUrl(target.host.area, target.host.path, device)
        : `/api/v1/files/content?id=${encodeURIComponent(target.id!)}`;
      const response = await request(url, { credentials: "same-origin", cache: "no-store" });
      if (current !== generation.current) return;
      if (response.status === 404) { setState("missing"); return; }
      if (!response.ok) { setState("unavailable"); return; }
      const body = await response.arrayBuffer();
      const next = decodeText(body);
      if (current !== generation.current) return;
      if (next === null) { setState("binary"); return; }
      setText(next);
      setSavedText(readOnly ? "" : next);
      setCursor({ line: 1, column: 1 });
      setState("ready");
    } catch (error) {
      if (current !== generation.current) return;
      setState(error instanceof Error && error.message === "limit" ? "limit" : "unavailable");
    }
  }, [live, target, readOnly]);

  useEffect(() => { void load(); }, [load]);

  // Load the rich surface once per file (or when returning from preview). Never
  // while typing: resetting innerHTML would move the caret.
  useEffect(() => {
    const element = rich.current;
    if (!element || !richVisible || state !== "ready") return;
    const key = `${target?.id ?? target?.name}:${markdown}`;
    if (loaded.current === key && element.childNodes.length) return;
    element.innerHTML = markdownToHtml(text);
    loaded.current = key;
  }, [richVisible, state, target?.id, target?.name, markdown, text]);

  const save = useCallback(async () => {
    const request = live?.request;
    if (!target?.id || readOnly || saving || !request) return;
    setSaving(true);
    setSaveError(false);
    try {
      const response = await request(`/api/v1/files/content?id=${encodeURIComponent(target.id)}`, {
        method: "PUT",
        headers: { "Content-Type": "application/octet-stream" },
        body: text
      });
      if (!response.ok) throw new Error("save");
      setSavedText(text);
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }, [live, readOnly, saving, target, text]);

  const syncRich = useCallback(() => {
    const element = rich.current;
    if (!element) return;
    setText(htmlToMarkdown(element));
  }, []);

  const updateFormat = useCallback(() => {
    if (!rich.current || !rich.current.contains(document.activeElement)) return;
    setFormat({
      bold: document.queryCommandState("bold"),
      italic: document.queryCommandState("italic"),
      strike: document.queryCommandState("strikeThrough")
    });
    const value = String(document.queryCommandValue("formatBlock") ?? "").toLowerCase();
    setBlock(value === "h1" || value === "h2" || value === "h3" || value === "blockquote" ? value : "p");
  }, []);

  useEffect(() => {
    document.addEventListener("selectionchange", updateFormat);
    return () => document.removeEventListener("selectionchange", updateFormat);
  }, [updateFormat]);

  const exec = (command: string, value?: string) => {
    rich.current?.focus();
    document.execCommand(command, false, value);
    syncRich();
    updateFormat();
  };

  const setBlockFormat = (next: BlockFormat) => {
    exec("formatBlock", next);
    setBlock(next);
  };

  /** Replaces the current range with a constructed node (never HTML text). */
  const insertNode = (node: Node, select = false) => {
    const selection = window.getSelection();
    const range = selection?.rangeCount ? selection.getRangeAt(0) : null;
    if (!range) return false;
    range.deleteContents();
    range.insertNode(node);
    if (select) range.selectNode(node);
    else {
      range.setStartAfter(node);
      range.collapse(true);
    }
    selection?.removeAllRanges();
    selection?.addRange(range);
    rich.current?.focus();
    syncRich();
    return true;
  };

  const insertInlineCode = () => {
    const selected = window.getSelection()?.toString() ?? "";
    const code = document.createElement("code");
    code.textContent = selected || "code";
    insertNode(code);
  };

  const openLink = () => {
    const selection = window.getSelection();
    savedRange.current = selection?.rangeCount ? selection.getRangeAt(0).cloneRange() : null;
    setLinkValue("https://");
    setLinkOpen(true);
  };

  const applyLink = () => {
    if (!linkOpen) return;
    const url = linkValue.trim();
    const valid = /^(?:https?:\/\/|mailto:)/i.test(url);
    rich.current?.focus();
    if (valid) {
      const selection = window.getSelection();
      if (savedRange.current && selection) {
        selection.removeAllRanges();
        selection.addRange(savedRange.current);
      }
      if (selection && !selection.isCollapsed) document.execCommand("createLink", false, url);
      else {
        // Insert the address as text and let the editor command turn the
        // selected text into a link; no DOM attribute is written from input.
        const text = document.createTextNode(url);
        if (insertNode(text, true)) document.execCommand("createLink", false, url);
      }
      syncRich();
    }
    setLinkOpen(false);
  };

  const insertTask = () => {
    rich.current?.focus();
    document.execCommand("insertHTML", false, '<div data-task="true" class="md-task-line"><input type="checkbox"> Task</div>');
    syncRich();
  };

  const onRichClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    const input = (event.target as HTMLElement).closest('input[type="checkbox"]') as HTMLInputElement | null;
    if (input) {
      event.preventDefault();
      if (readOnly) return;
      input.checked = !input.checked;
      syncRich();
    }
  };

  const onRichPaste = (event: ReactClipboardEvent<HTMLDivElement>) => {
    // Paste as plain text: never inject foreign HTML into the document.
    event.preventDefault();
    if (readOnly) return;
    const textValue = event.clipboardData.getData("text/plain");
    if (textValue) document.execCommand("insertText", false, textValue);
  };

  const onRichKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      event.stopPropagation();
      void save();
    }
    if (!readOnly && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      event.stopPropagation();
      openLink();
    }
  };

  const onPlainKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void save();
    }
  };

  const updateCursor = () => {
    const element = area.current;
    if (!element) return;
    const before = element.value.slice(0, element.selectionStart);
    const lines = before.split("\n");
    setCursor({ line: lines.length, column: (lines.at(-1)?.length ?? 0) + 1 });
  };

  if (!target) {
    return <section className="editor editor--empty">
      <span className="editor__empty-mark" aria-hidden="true"><DocumentIcon /></span>
      <h1>{t("editor.emptyTitle")}</h1>
      <p>{t("editor.emptyHelp")}</p>
      <Button onClick={() => open("/app/files")}>{t("editor.openFiles")}</Button>
    </section>;
  }

  const status = state === "loading"
    ? t("editor.loading")
    : state !== "ready" ? t("editor.errorTitle")
    : saving ? t("editor.saving")
    : saveError
      ? t("editor.saveFailed")
      : readOnly ? t("editor.readOnly")
      : dirty
        ? t("editor.dirty")
        : t("editor.saved");

  const header = <header className={`editor__bar${topbarHost ? " editor__bar--hosted" : ""}`}>
      <div className="editor__identity">
        <span className="editor__glyph" aria-hidden="true"><DocumentIcon /></span>
        <span className="editor__titles">
          <strong title={target.name}>{target.name}</strong>
          <small>{markdown ? t("editor.markdown") : t("editor.plain")}{readOnly ? ` · ${t("editor.readOnly")}` : dirty ? ` · ${t("editor.dirty")}` : ` · ${t("editor.saved")}`}</small>
        </span>
      </div>
      <div className="editor__actions" role="group" aria-label={t("editor.documentActions")}>
        {markdown ? <RumahlButtonGroup className="editor__modes" label={t("editor.modeLabel")} value={mode} onChange={setMode}
          options={(["edit", "split", "preview"] as const).map(value => ({ value, label: t(`editor.mode.${value}`) }))} /> : null}
        {markdown ? null : <label className="editor__toggle">
          <input type="checkbox" role="switch" checked={wrap} onChange={(event) => setWrap(event.target.checked)} />
          {t("editor.wrap")}
        </label>}
        <Button className="editor__reload" size="sm" variant="secondary" leftIcon={<RefreshIcon />} aria-label={t("editor.reload")}
          onClick={() => { if (dirty) setReloadConfirm(true); else void load(); }} disabled={state === "loading" || saving}><span className="editor__reload-label">{t("editor.reload")}</span></Button>
        <Button size="sm" variant="primary" onClick={() => void save()} loading={saving} disabled={readOnly || !dirty}>
          {t("editor.save")}
        </Button>
      </div>
    </header>;
  const tool = (icon: EditorTool, label: string, action: () => void, active?: boolean) => <Button
    className="editor__tool" variant="ghost" size="sm" title={label} aria-label={label} aria-pressed={active}
    disabled={readOnly} onMouseDown={event => event.preventDefault()} onClick={action}>
    <EditorToolIcon tool={icon} />
  </Button>;

  return <section className="editor" aria-label={t("editor.title")}>
    {topbarHost ? createPortal(header, topbarHost) : header}

    {markdown && showWrite && state === "ready" ? <div className="editor__format" role="toolbar" aria-label={t("editor.format")}>
      <select
        className="editor__block"
        aria-label={t("editor.format.block")}
        value={block}
        disabled={readOnly}
        onChange={(event) => setBlockFormat(event.target.value as BlockFormat)}
      >
        <option value="p">{t("editor.format.paragraph")}</option>
        <option value="h1">{t("editor.format.h1")}</option>
        <option value="h2">{t("editor.format.h2")}</option>
        <option value="h3">{t("editor.format.h3")}</option>
        <option value="blockquote">{t("editor.format.quoteBlock")}</option>
      </select>
      <span className="editor__tool-sep" aria-hidden="true" />
      <div className="editor__format-group" role="group" aria-label={t("editor.format.textGroup")}>
        {tool("bold", t("editor.format.bold"), () => exec("bold"), format.bold)}
        {tool("italic", t("editor.format.italic"), () => exec("italic"), format.italic)}
        {tool("strike", t("editor.format.strike"), () => exec("strikeThrough"), format.strike)}
        {tool("code", t("editor.format.code"), insertInlineCode)}
        {tool("link", t("editor.format.link"), openLink)}
      </div>
      <span className="editor__tool-sep" aria-hidden="true" />
      <div className="editor__format-group" role="group" aria-label={t("editor.format.blockGroup")}>
        {tool("list", t("editor.format.list"), () => exec("insertUnorderedList"))}
        {tool("numbered", t("editor.format.numbered"), () => exec("insertOrderedList"))}
        {tool("task", t("editor.format.task"), insertTask)}
        {tool("rule", t("editor.format.rule"), () => exec("insertHorizontalRule"))}
      </div>
      <span className="editor__tool-sep" aria-hidden="true" />
      <div className="editor__format-group" role="group" aria-label={t("editor.format.historyGroup")}>
        {tool("undo", t("editor.format.undo"), () => exec("undo"))}
        {tool("redo", t("editor.format.redo"), () => exec("redo"))}
      </div>
      {linkOpen ? <form className="editor__link" onSubmit={(event) => { event.preventDefault(); applyLink(); }}>
        <input
          autoFocus
          type="url"
          value={linkValue}
          aria-label={t("editor.linkUrl")}
          onChange={(event) => setLinkValue(event.target.value)}
          onKeyDown={(event) => { if (event.key === "Escape") setLinkOpen(false); }}
        />
        <Button size="sm" variant="primary" type="submit">{t("editor.linkApply")}</Button>
        <Button size="sm" variant="ghost" type="button" onClick={() => setLinkOpen(false)}>{t("editor.linkCancel")}</Button>
      </form> : null}
    </div> : null}

    {state === "ready" ? <div className={`editor__panes${showPreview && showWrite ? " is-split" : ""}`}>
      {showWrite ? <div className="editor__pane editor__pane--write">
        {markdown ? <div
          ref={rich}
          className="editor__rich markdown"
          contentEditable={!readOnly}
          suppressContentEditableWarning
          role="textbox"
           aria-multiline="true"
           aria-readonly={readOnly}
          aria-label={t("editor.textArea", { name: target.name })}
          data-placeholder={t("editor.placeholder")}
          onInput={syncRich}
          onClick={onRichClick}
          onPaste={onRichPaste}
          onKeyDown={onRichKeyDown}
          onBlur={syncRich}
        /> : <textarea
          ref={area}
          className={`editor__area${wrap ? " is-wrapped" : ""}`}
          value={text}
          onChange={(event) => { setText(event.target.value); updateCursor(); }}
          onKeyDown={onPlainKeyDown}
          onSelect={updateCursor}
          onClick={updateCursor}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          autoComplete="off"
          readOnly={readOnly}
          aria-label={t("editor.textArea", { name: target.name })}
        />}
      </div> : null}
      {showPreview ? <div className="editor__pane editor__pane--preview">
        <article className="editor__sheet">
          <MarkdownDocument source={text} />
        </article>
      </div> : null}
    </div> : <div className="editor__state" role="status">
      <h2>{state === "loading" ? t("editor.loading") : t("editor.errorTitle")}</h2>
      <p>{state === "loading" ? t("editor.loadingHelp") : state === "missing" ? t("editor.error.missing") : state === "binary" ? t("editor.error.binary") : state === "limit" ? t("editor.error.limit") : t("editor.error.unavailable")}</p>
      {state !== "loading" ? <Button size="sm" onClick={() => void load()}>{t("editor.reload")}</Button> : null}
    </div>}

    <footer className="editor__status">
      {markdown ? <>
        <span>{t("editor.words", { count: words })}</span>
        <span>{t("editor.characters", { count: text.length })}</span>
      </> : <>
        <span>{t("editor.lines", { line: cursor.line, column: cursor.column })}</span>
        <span>{t("editor.characters", { count: text.length })}</span>
      </>}
      <span className="editor__save-state" data-state={saveError ? "error" : saving ? "saving" : dirty ? "dirty" : "saved"} aria-live="polite">{status}</span>
      <span className="editor__path" title={target.host?.path ?? target.name}>{target.host ? target.host.path : t("editor.personal")}</span>
    </footer>
    <RumahlDialog open={reloadConfirm} onClose={() => setReloadConfirm(false)}
      title={t("editor.reloadConfirm")} description={t("editor.reloadConfirmHelp")} application={t("editor.title")}
      confirmLabel={t("editor.reload")} cancelLabel={t("editor.keepEditing")} destructive
      onConfirm={() => { setReloadConfirm(false); void load(); }} />
  </section>;
}
