import {
  RumahlInput,
  type InputFeedbackProps,
  type ValidationProps,
} from "./RumahlInputs";
import { useEffect, useId, useRef, useState, type ComponentProps } from "react";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { RumahlSelect } from "./RumahlSelect";

export interface RumahlProfile {
  id: string;
  name: string;
  detail?: string;
  avatar?: string;
}
/** Constrains an avatar source to safe URL schemes (never `javascript:`/HTML data URLs). */
function safeAvatarSrc(value: string | undefined): string | undefined {
  if (!value) return undefined;
  const url = value.trim();
  if (!url) return undefined;
  if (/^data:image\//i.test(url)) return url;
  if (/^[a-z][a-z0-9+.-]*:/i.test(url)) return /^(https?|blob):/i.test(url) ? url : undefined;
  return url; // relative path
}

export function RumahlAvatar({
  name,
  src,
  size = "md",
  status,
}: {
  name: string;
  src?: string | undefined;
  size?: "sm" | "md" | "lg";
  status?: "online" | "away" | "offline";
}) {
  const [failed, setFailed] = useState<string>();
  const safeSrc = safeAvatarSrc(src);
  const initials =
    name
      .trim()
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => Array.from(word)[0])
      .join("")
      .toLocaleUpperCase() || "?";
  return (
    <span
      className={`rumahl-avatar rumahl-avatar--${size}`}
      role="img"
      aria-label={name}
    >
      {safeSrc && failed !== safeSrc ? (
        <img src={safeSrc} alt="" onError={() => setFailed(safeSrc)} />
      ) : (
        <span aria-hidden="true">{initials}</span>
      )}
      {status ? (
        <i
          className="rumahl-avatar__status"
          data-status={status}
          aria-hidden="true"
        />
      ) : null}
    </span>
  );
}
export function RumahlProfileCard({
  profile,
  status,
}: {
  profile: RumahlProfile;
  status?: string;
}) {
  return (
    <div className="rumahl-profile-card">
      <RumahlAvatar name={profile.name} src={profile.avatar} size="lg" />
      <div>
        <strong>{profile.name}</strong>
        {profile.detail ? <small>{profile.detail}</small> : null}
        {status ? (
          <span className="rumahl-profile-card__status">{status}</span>
        ) : null}
      </div>
    </div>
  );
}
export function RumahlProfileSwitch({
  profiles,
  value,
  onChange,
  disabled,
}: {
  profiles: readonly RumahlProfile[];
  value: string;
  onChange: (id: string) => void;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const render = (id: string) => {
    const profile = profiles.find((item) => item.id === id);
    return profile ? (
      <span className="rumahl-profile-option">
        <RumahlAvatar name={profile.name} src={profile.avatar} size="sm" />
        <span>
          <strong>{profile.name}</strong>
          {profile.detail ? <small>{profile.detail}</small> : null}
        </span>
      </span>
    ) : (
      id
    );
  };
  return (
    <RumahlSelect
      label={t("profile.switch")}
      value={value}
      onChange={onChange}
      disabled={disabled ?? false}
      options={profiles.map((p) => ({ value: p.id, label: p.name }))}
      renderValue={render}
      renderOption={render}
    />
  );
}
export function RumahlPassword({
  label,
  id: providedId,
  disabled,
  className = "",
  status,
  error,
  ...props
}: Omit<ComponentProps<"input">, "type"> &
  ValidationProps &
  InputFeedbackProps & { label: string }) {
  const { t } = useI18n();
  const generatedId = useId();
  const id = providedId ?? generatedId;
  const [visible, setVisible] = useState(false);
  const [caps, setCaps] = useState(false);
  return (
    <div
      className="rumahl-password"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget))
          setVisible(false);
      }}
    >
      <label htmlFor={id}>{label}</label>
      <div className="rumahl-password__field">
        <RumahlInput
          {...(status ? { status } : {})}
          {...(error !== undefined ? { error } : {})}
          {...props}
          id={id}
          disabled={disabled}
          className={`rumahl-input ${className}`}
          type={visible ? "text" : "password"}
          aria-describedby={
            [props["aria-describedby"], caps ? `${id}-caps` : undefined]
              .filter(Boolean)
              .join(" ") || undefined
          }
          onKeyDown={(event) => {
            setCaps(event.getModifierState("CapsLock"));
            props.onKeyDown?.(event);
          }}
          onKeyUp={(event) => {
            setCaps(event.getModifierState("CapsLock"));
            props.onKeyUp?.(event);
          }}
          onBlur={(event) => {
            setCaps(false);
            props.onBlur?.(event);
          }}
        />
        <button
          type="button"
          className="rumahl-password__toggle"
          disabled={disabled}
          aria-controls={id}
          aria-pressed={visible}
          aria-label={t(
            visible ? "profile.hidePassword" : "profile.showPassword",
          )}
          onPointerDown={(event) => event.preventDefault()}
          onClick={() => setVisible((state) => !state)}
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            aria-hidden="true"
          >
            <path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12Z" />
            <circle cx="12" cy="12" r="3" />
            {visible ? <path d="m3 3 18 18" /> : null}
          </svg>
        </button>
      </div>
      {caps ? (
        <small id={`${id}-caps`} role="status">
          {t("profile.capsLock")}
        </small>
      ) : null}
    </div>
  );
}

/** Local file selection only; callers own upload/persistence through onChange. */
export function RumahlAvatarChange({
  name,
  src,
  onChange,
  disabled,
}: {
  name: string;
  src?: string | undefined;
  onChange: (file: File | null) => void;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const input = useRef<HTMLInputElement>(null);
  const pending = useRef(0);
  const [file, setFile] = useState<File | null>(null);
  const [removed, setRemoved] = useState(false);
  const [preview, setPreview] = useState<string>();
  const [error, setError] = useState("");
  const id = useId();
  useEffect(() => {
    if (!file) {
      setPreview(undefined);
      return;
    }
    const url = URL.createObjectURL(file);
    setPreview(url);
    return () => URL.revokeObjectURL(url);
  }, [file]);
  useEffect(
    () => () => {
      pending.current++;
    },
    [],
  );
  async function select(next: File | undefined) {
    if (!next) return;
    const version = ++pending.current;
    if (
      !["image/png", "image/jpeg", "image/webp"].includes(next.type) ||
      next.size > 5 * 1024 * 1024
    ) {
      setError(t("profile.imageError"));
      return;
    }
    const url = URL.createObjectURL(next);
    const valid = await new Promise<boolean>((resolve) => {
      const image = new Image();
      image.onload = () => resolve(image.naturalWidth > 0);
      image.onerror = () => resolve(false);
      image.src = url;
    });
    URL.revokeObjectURL(url);
    if (version !== pending.current) return;
    if (!valid) {
      setError(t("profile.imageError"));
      return;
    }
    setError("");
    setRemoved(false);
    setFile(next);
    onChange(next);
  }
  return (
    <div className="rumahl-avatar-change">
      <RumahlAvatar
        name={name}
        src={removed ? undefined : (preview ?? src)}
        size="lg"
      />
      <div>
        <input
          ref={input}
          type="file"
          hidden
          accept="image/png,image/jpeg,image/webp"
          disabled={disabled}
          aria-label={t("profile.changeAvatar")}
          onChange={(event) => {
            void select(event.target.files?.[0]);
            event.target.value = "";
          }}
        />
        <div className="rumahl-avatar-change__actions">
          <Button
            disabled={disabled}
            aria-describedby={id}
            onClick={() => input.current?.click()}
          >
            {t("profile.changeAvatar")}
          </Button>
          <Button
            variant="ghost"
            disabled={disabled || (!file && (!src || removed))}
            onClick={() => {
              pending.current++;
              setFile(null);
              setRemoved(true);
              setError("");
              onChange(null);
            }}
          >
            {t("profile.removeAvatar")}
          </Button>
        </div>
        <small id={id}>{t("profile.imageHelp")}</small>
        {error ? <p role="alert">{error}</p> : null}
      </div>
    </div>
  );
}
