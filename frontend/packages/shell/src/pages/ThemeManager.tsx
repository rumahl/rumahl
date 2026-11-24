import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { Button } from "../components/Button";
import { RumahlSelect } from "../components/RumahlSelect";
import { SettingsHeading } from "../components/SettingsHeading";
import { ThemeConsent } from "../components/ThemeConsent";
import { useI18n } from "../i18n";
import { useShellPreferences } from "../preferences/ShellPreferences";
import {
  exportThemePackage,
  installTheme,
  isInstalledTheme,
  isThemeTrusted,
  prepareThemeFile,
  revokeThemeTrust,
  trustTheme,
  uninstallTheme,
  useThemeRegistry,
  type PreparedThemeInstall,
} from "../preferences/installed-themes";
import { ThemeSafetyError } from "../preferences/theme-safety";
import { useOptionalShell } from "../shell/ShellContext";
import { useFullAccessThemes } from "../preferences/theme-policy";
import { reportActivity } from "../apps/recovery-client";
import { dismissToast, showToast } from "../shell/toasts";
import { useTheme } from "@rumahl/ui";
import { getTheme } from "@rumahl/ui/sdk";

import { DEFAULT_THEME_ID, type Theme } from "@rumahl/ui/themes";

/** Miniature of the desktop rendered with the theme's own roles. */
function previewValues(theme: Theme) {
  const wallpaper = theme.tokens["texture.wallpaper"];
  return {
    wall: wallpaper.startsWith("url(")
      ? wallpaper
      : `radial-gradient(circle at 30% 24%, color-mix(in srgb, ${theme.tokens["color.accent"]} 34%, transparent), ${theme.tokens["color.canvas.background"]})`,
    strong: theme.tokens["color.surface.strong"],
    accent: theme.tokens["color.accent"],
    radius: theme.tokens["shape.radius.window"],
  };
}

/** Preview colours are applied through CSSOM: the shell CSP forbids inline style attributes. */
function ThemePreview({ theme }: { theme: Theme }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const values = previewValues(theme);
    element.style.setProperty("--preview-wall", values.wall);
    element.style.setProperty("--preview-strong", values.strong);
    element.style.setProperty("--preview-accent", values.accent);
    element.style.setProperty("--preview-radius", values.radius);
  }, [theme]);
  return <div ref={ref} className="theme-card__preview">
    <span className="theme-card__window"><i /><i /><i /></span>
    <span className="theme-card__dock"><i /><i /><i /></span>
  </div>;
}

function download(theme: Theme): void {
  const blob = new Blob([exportThemePackage(theme)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `${theme.id}.json`;
  anchor.click();
  URL.revokeObjectURL(url);
}

/** Theme gallery: apply, install, export and remove theme packages. */
export function ThemeManager() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const settings = useShellPreferences();
  const themes = useThemeRegistry();
  const { theme: active } = useTheme();
  const activeName = getTheme(active.id)?.name ?? active.name;
  const fileRef = useRef<HTMLInputElement>(null);
  const shell = useOptionalShell();
  const fullAccessAllowed = useFullAccessThemes(shell?.live);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pendingTheme, setPendingTheme] = useState<string | null>(null);
  const [consent, setConsent] = useState<{
    theme: Theme;
    codeHash: string | null;
    prepared?: PreparedThemeInstall;
    apply?: string;
  } | null>(null);

  const activity = (action: string) => {
    if (shell?.live) reportActivity(shell.live.request, action);
  };

  const applyNow = (id: string) => {
    if (settings.preferences?.[settings.scope].shellTheme === id) return;
    settings.saveTheme(settings.scope, id);
    setPendingTheme(id);
  };

  const finishInstall = (prepared: PreparedThemeInstall, execution: "sandbox" | "trusted") => {
    const theme = installTheme(prepared.data, "manual", {
      execution,
      ...(prepared.codeHash ? { codeHash: prepared.codeHash } : {}),
    });
    activity(`theme install ${theme.id} ${execution}`);
    setError(null);
    applyNow(theme.id);
  };

  const apply = (id: string) => {
    const theme = themes.find((item) => item.id === id);
    if (theme?.code && !isThemeTrusted(theme) && (theme.runtime ?? "shell") !== "sandbox") {
      setConsent({ theme, codeHash: theme.codeHash ?? null, apply: id });
      return;
    }
    applyNow(id);
  };

  // Ask for the reload only after the preference was persisted, exactly like
  // the appearance page does: a stored theme can need a full first paint.
  useEffect(() => {
    if (!pendingTheme || settings.preferences?.[settings.scope].shellTheme !== pendingTheme) return;
    setPendingTheme(null);
    showToast({
      id: "theme-reload",
      title: t("theme.reloadTitle"),
      message: t("theme.reloadBody"),
      variant: "info",
      duration: null,
      buttons: [
        { label: t("reload.now"), variant: "primary", onClick: () => window.location.reload() },
        { label: t("reload.later"), onClick: () => dismissToast("theme-reload") },
      ],
    });
  }, [pendingTheme, settings.preferences, settings.scope, t]);

  const remove = (id: string) => {
    if (active.id === id) {
      settings.saveTheme("device", null);
      settings.saveTheme("user", DEFAULT_THEME_ID);
    }
    uninstallTheme(id);
  };

  const install = async (file: File) => {
    setBusy(true);
    try {
      const prepared = await prepareThemeFile(file);
      const runtime = prepared.theme.runtime ?? "shell";
      if (!prepared.theme.code || runtime === "sandbox") {
        finishInstall(prepared, "sandbox");
      } else {
        setConsent({
          theme: prepared.theme,
          codeHash: prepared.codeHash,
          prepared,
        });
      }
    } catch (failure) {
      setError(
        failure instanceof ThemeSafetyError
          ? failure.report.issues.map((issue) => issue.message).join(" ")
          : t("themes.invalid")
      );
    } finally {
      setBusy(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  const chooseExecution = (execution: "sandbox" | "trusted") => {
    if (!consent) return;
    const current = consent;
    setConsent(null);
    if (current.prepared) {
      finishInstall(current.prepared, execution);
      return;
    }
    if (execution === "trusted") {
      void trustTheme(current.theme.id, current.codeHash ?? undefined)
        .then(() => {
          activity(`theme trust ${current.theme.id}`);
          setError(null);
          if (current.apply) applyNow(current.apply);
        })
        .catch(() => setError(t("themes.invalid")));
    } else {
      revokeThemeTrust(current.theme.id);
      activity(`theme revoke ${current.theme.id}`);
      if (current.apply) applyNow(current.apply);
    }
  };

  const requestTrust = (theme: Theme) => {
    setConsent({ theme, codeHash: theme.codeHash ?? null });
  };

  const revoke = (id: string) => {
    revokeThemeTrust(id);
    activity(`theme revoke ${id}`);
  };

  return <section className="theme-manager">
    <SettingsHeading title={t("themes.title")} help={t("themes.help")} back="/settings/display" />
    <p className="theme-security-notice">{fullAccessAllowed ? t("themes.securityNotice") : t("themes.fullAccessDisabled")}</p>

    <h3 className="section-title">{t("themes.general")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy">
          <strong>{t("preferences.scope")}</strong>
          <p>{t("preferences.precedence")}</p>
        </div>
        <div className="visual">
          <RumahlSelect
            label={t("preferences.scope")}
            value={settings.scope}
            onChange={(value) => settings.setScope(value === "device" ? "device" : "user")}
            options={[
              { value: "user", label: t("preferences.user") },
              { value: "device", label: t("preferences.device") },
            ]}
          />
        </div>
      </div>
      {active.parameters.length > 0 ? <div className="setting">
        <div className="copy">
          <strong>{t("themes.settingsFor", { name: activeName })}</strong>
          <p>{t("themes.settingsHelp")}</p>
        </div>
        <div className="visual">
          <Button size="sm" onClick={() => navigate("/settings/display/themes/settings")}>{t("themes.openSettings")}</Button>
        </div>
      </div> : null}
      <div className="setting">
        <div className="copy">
          <strong>{t("themes.install")}</strong>
          <p>{t("themes.installHelp")}</p>
        </div>
        <div className="visual">
          <input ref={fileRef} type="file" hidden accept="application/json,.json,application/zip,.zip"
            onChange={(event) => { const file = event.target.files?.[0]; if (file) void install(file); }} />
          <Button onClick={() => fileRef.current?.click()} disabled={busy}>{busy ? t("themes.installing") : t("themes.installButton")}</Button>
        </div>
      </div>
    </div>
    {error ? <p role="alert" className="settings-footnote">{error}</p> : null}

    <h3 className="section-title">{t("themes.installedThemes")}</h3>
    <div className="theme-grid">
      {themes.map((theme) => {
        const isActive = active.id === theme.id;
        const scopeSelected = settings.preferences?.[settings.scope].shellTheme === theme.id;
        const origin = theme.origin === "store" ? t("themes.fromStore") : theme.origin === "builtin" ? t("themes.builtIn") : t("themes.manual");
        const runtime = theme.runtime ?? "shell";
        const trusted = isThemeTrusted(theme);
        const badge = theme.code
          ? trusted
            ? t("themes.fullAccessBadge")
            : runtime === "shell"
              ? t("themes.fullAccessRequired")
              : t("themes.sandboxedBadge")
          : null;
        return <article key={theme.id} className="theme-card" data-active={isActive ? "true" : undefined}>
          {theme.preview
            ? <img className="theme-card__image" src={theme.preview} alt="" />
            : <ThemePreview theme={theme} />}
          <div className="theme-card__meta">
            <div className="theme-card__identity">
              <strong>{theme.name}</strong>
              {badge ? <span className={`theme-card__badge${trusted || runtime === "shell" ? " theme-card__badge--danger" : ""}`}>{badge}</span> : null}
              <small>
                {origin}
                {theme.modes.includes("dark") && !theme.modes.includes("light") ? ` · ${t("themes.darkOnly")}` : ""}
                {scopeSelected ? ` · ${t("themes.selected")}` : ""}
              </small>
            </div>
            <div className="theme-card__actions">
              <Button size="sm" variant={isActive ? "primary" : "secondary"} onClick={() => apply(theme.id)}>
                {isActive ? t("themes.active") : t("themes.apply")}
              </Button>
              {theme.code && trusted
                ? <Button size="sm" variant="ghost" onClick={() => revoke(theme.id)}>{t("themes.revokeFullAccess")}</Button>
                : theme.code && runtime !== "sandbox" && fullAccessAllowed
                  ? <Button size="sm" variant="ghost" onClick={() => requestTrust(theme)}>{t("themes.allowFullAccess")}</Button>
                  : null}
              {isInstalledTheme(theme.id) ? <Button size="sm" variant="ghost" onClick={() => remove(theme.id)}>{t("themes.uninstall")}</Button> : null}
              <Button size="sm" variant="ghost" onClick={() => download(theme)}>{t("themes.export")}</Button>
            </div>

          </div>
        </article>;
      })}
    </div>
    {consent ? <ThemeConsent
      theme={consent.theme}
      codeHash={consent.codeHash}
      allowFullAccess={fullAccessAllowed}
      onCancel={() => setConsent(null)}
      onChoose={chooseExecution}
    /> : null}
  </section>;
}
