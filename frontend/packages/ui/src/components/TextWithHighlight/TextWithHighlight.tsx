import { Fragment, type ReactNode } from "react";
import { Text, type TextProps } from "../Text/Text";
import type { ControlSize } from "../../size";

export interface TextWithHighlightProps extends TextProps {
  /** Full text. */
  text: string;
  /** Substring to highlight (case-insensitive); empty renders plain text. */
  highlight: string;
  size?: ControlSize | undefined;
}

/**
 * Renders `text` with every case-insensitive occurrence of `highlight` wrapped
 * in a `<mark>`. Useful for search results and autocomplete lists.
 */
export function TextWithHighlight({ text, highlight, ...props }: TextWithHighlightProps) {
  const query = highlight.trim();
  if (!query) return <Text {...props}>{text}</Text>;
  const lower = text.toLocaleLowerCase();
  const needle = query.toLocaleLowerCase();
  const parts: ReactNode[] = [];
  let index = 0;
  let key = 0;
  for (;;) {
    const found = lower.indexOf(needle, index);
    if (found < 0) {
      parts.push(text.slice(index));
      break;
    }
    if (found > index) parts.push(text.slice(index, found));
    parts.push(
      <mark key={key++} className="rumahl-text__highlight">
        {text.slice(found, found + needle.length)}
      </mark>
    );
    index = found + needle.length;
  }
  return <Text {...props}>{parts.map((part, i) => <Fragment key={i}>{part}</Fragment>)}</Text>;
}
