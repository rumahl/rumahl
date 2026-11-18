import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n";

interface AddressBarProps {
  /** Canonical rumahl path of the current folder. */
  path: string;
  busy?: boolean;
  onSubmit: (input: string) => void;
}

/**
 * The Files address line: a `rumahl://` path that works the same on Windows,
 * macOS, Linux and mobile because it names a rumahl area, never a host path.
 * Typing is local; navigation and permission checks happen when the service
 * resolves the path.
 */
export function AddressBar({ path, busy = false, onSubmit }: AddressBarProps) {
  const { t } = useI18n();
  const [value, setValue] = useState(path);
  const [focused, setFocused] = useState(false);
  const typed = useRef(path);
  const input = useRef<HTMLInputElement>(null);

  // A navigation or a deep link replaces the draft; an active edit is kept.
  useEffect(() => {
    if (focused) return;
    typed.current = path;
    setValue(path);
  }, [path, focused]);

  const submit = () => {
    if (busy) return;
    onSubmit(typed.current);
  };

  return <form className="files-address__form" onSubmit={(event) => { event.preventDefault(); submit(); }}>
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
      onBlur={() => { setFocused(false); setValue(path); }}
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
  </form>;
}
