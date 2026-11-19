import { useRef } from "react";
import { SearchIcon } from "../../icons";
import { useI18n } from "../../i18n";

/**
 * Shared folder search. A native search input owns Escape-to-clear, and a
 * visible clear button appears whenever the query is non-empty.
 */
export function SearchField({ value, onChange, className, autoFocus = false }: { value: string; onChange: (value: string) => void; className: string; autoFocus?: boolean }) {
  const { t } = useI18n();
  const input = useRef<HTMLInputElement>(null);
  const clear = () => {
    onChange("");
    input.current?.focus();
  };
  return <div className={className}>
    <SearchIcon className="files-search__icon" />
    <input
      ref={input}
      type="search"
      autoFocus={autoFocus}
      aria-label={t("files.search")}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      onKeyDown={(event) => {
        if (event.key === "Escape" && value) {
          event.preventDefault();
          event.stopPropagation();
          onChange("");
        }
      }}
      placeholder={t("files.search")}
      maxLength={255}
    />
    {value
      ? <button type="button" className="files-search__clear" aria-label={t("files.clearSearch")} title={t("files.clearSearch")} onClick={clear}>×</button>
      : null}
  </div>;
}
