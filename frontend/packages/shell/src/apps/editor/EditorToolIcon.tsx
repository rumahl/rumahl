export type EditorTool = "bold" | "italic" | "strike" | "code" | "link" | "list" | "numbered" | "task" | "rule" | "undo" | "redo";
const paths: Record<EditorTool, string> = {
  bold: "M7 4h6a4 4 0 0 1 0 8H7zm0 8h7a4 4 0 0 1 0 8H7z",
  italic: "M10 4h8M6 20h8M14 4l-4 16",
  strike: "M17 6c-1-2-3-3-5-3-3 0-5 2-5 4 0 2 2 3 5 4m-8 1h16m-3 3c1 3-2 6-5 6-2 0-5-1-6-3",
  code: "m8 7-5 5 5 5m8-10 5 5-5 5m-3-14-2 18",
  link: "M10 13a4 4 0 0 0 6 0l4-4a4 4 0 0 0-6-6l-2 2m2 6a4 4 0 0 0-6 0l-4 4a4 4 0 0 0 6 6l2-2",
  list: "M9 6h12M9 12h12M9 18h12M3 6h1M3 12h1M3 18h1",
  numbered: "M10 6h11M10 12h11M10 18h11M3 4h1v5M2 14c0-3 4-3 4 0 0 1-4 3-4 5h4",
  task: "M10 5h11M10 12h11M10 19h11M2 4l2 2 3-3M2 11h4v4H2z",
  rule: "M3 12h18",
  undo: "M8 4 3 9l5 5M3 9h11a6 6 0 0 1 0 12",
  redo: "m16 4 5 5-5 5m5-5H10a6 6 0 0 0 0 12"
};
export function EditorToolIcon({ tool }: { tool: EditorTool }) {
  return <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={paths[tool]} /></svg>;
}
