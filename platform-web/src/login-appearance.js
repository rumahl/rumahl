/* Restore only the last device presentation, never a user's private workspace. */
(() => {
  try {
    const saved = JSON.parse(localStorage.getItem("rumahl.login.appearance.v1") || "null");
    if (!saved || saved.version !== 1 || !saved.tokens || typeof saved.tokens !== "object") return;
    const root = document.documentElement;
    const defaults = getComputedStyle(root);
    for (const [id, value] of Object.entries(saved.tokens)) {
      if (!/^[a-z]+(?:\.[a-z]+)+$/.test(id) || typeof value !== "string" || value.length > 2048) continue;
      const property = `--rumahl-ui-${id.replaceAll(".", "-")}`;
      // Only the public token contract emitted by the server can be restored.
      if (id !== "texture.wallpaper" && !defaults.getPropertyValue(property).trim()) continue;
      if (/url\s*\(/i.test(value)) {
        // Wallpapers must be bundled assets on this origin, never remote URLs.
        if (id !== "texture.wallpaper" || !/^url\(["']?\/(?:assets|src\/assets)\/[a-zA-Z0-9_./-]+\.(?:jpg|jpeg|png|webp)["']?\)$/.test(value) || value.includes("..")) continue;
      }
      if (/[{};<>\\]/.test(value) || /(?:image-set|var|env)\s*\(/i.test(value)) continue;
      root.style.setProperty(property, value);
    }
    if (typeof saved.theme === "string" && /^[a-z0-9.-]{1,128}$/.test(saved.theme)) root.dataset.theme = saved.theme;
    if (saved.scheme === "dark" || saved.scheme === "light") {
      root.dataset.scheme = saved.scheme;
      root.style.colorScheme = saved.scheme;
    }
    if (saved.material === "solid" || saved.material === "translucent") root.dataset.material = saved.material;
  } catch {
    // Corrupt or unavailable storage falls back to the server's standard theme.
  }
})();

// Clock is decoration; account selection and authentication work without JS.
(() => {
  function start() {
    const time = document.getElementById("login-time");
    const date = document.getElementById("login-date");
    if (!time || !date) return;
    const locale = document.documentElement.lang || navigator.language;
    const timeFormat = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" });
    const dateFormat = new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" });
    function update() {
      const now = new Date();
      time.textContent = timeFormat.format(now);
      time.dateTime = now.toISOString();
      date.textContent = dateFormat.format(now);
    }
    update();
    setInterval(update, 1000 * 30);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start, { once: true });
  else start();
})();

// Progressive enhancement: native account links remain usable without JS.
(() => {
  let lifecycle;
  function start() {
    lifecycle?.abort();
    lifecycle = new AbortController();
    const { signal } = lifecycle;
    const track = document.getElementById("login-accounts");
    if (!track || track.dataset.enhanced) return;
    const cards = [...track.querySelectorAll(".account")];
    const controls = track.parentElement.querySelector(".slider-controls");
    if (!cards.length || !controls) return;
    const previous = controls.querySelector('[data-step="-1"]');
    const next = controls.querySelector('[data-step="1"]');
    const position = controls.querySelector(".slider-position");
    const reduced = matchMedia("(prefers-reduced-motion: reduce)");
    let current = 0;
    let frame = 0;
    function update() {
      frame = 0;
      const rect = track.getBoundingClientRect();
      const center = rect.left + rect.width / 2;
      current = cards.reduce((nearest, card, index) => {
        const a = card.getBoundingClientRect();
        const b = cards[nearest].getBoundingClientRect();
        return Math.abs(a.left + a.width / 2 - center) < Math.abs(b.left + b.width / 2 - center) ? index : nearest;
      }, 0);
      cards.forEach((card, index) => card.classList.toggle("is-current", index === current));
      previous.disabled = current === 0;
      next.disabled = current === cards.length - 1;
      position.textContent = `${current + 1} / ${cards.length}`;
    }
    function go(index, focus = false) {
      const card = cards[Math.max(0, Math.min(cards.length - 1, index))];
      if (focus) card.focus({ preventScroll: true });
      const target = card.getBoundingClientRect();
      const rect = track.getBoundingClientRect();
      track.scrollBy({ left: target.left + target.width / 2 - rect.left - rect.width / 2, behavior: reduced.matches ? "instant" : "smooth" });
    }
    previous.addEventListener("click", () => go(current - 1));
    next.addEventListener("click", () => go(current + 1));
    track.addEventListener("keydown", event => {
      const index = cards.indexOf(document.activeElement);
      if (index < 0) return;
      const target = { ArrowLeft: index - 1, ArrowRight: index + 1, Home: 0, End: cards.length - 1 }[event.key];
      if (target === undefined) return;
      event.preventDefault();
      go(target, true);
    });
    track.addEventListener("focusin", event => {
      const index = cards.indexOf(event.target);
      if (index >= 0) go(index);
    });
    function schedule() { if (!frame) frame = requestAnimationFrame(update); }
    track.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", () => { go(current); schedule(); }, { signal });
    signal.addEventListener("abort", () => cancelAnimationFrame(frame), { once: true });
    track.dataset.enhanced = "true";
    controls.hidden = cards.length < 2;
    update();
  }
  document.addEventListener("login:view", start);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start, { once: true });
  else start();
})();

// Same-origin login views and submissions use the existing server authentication.
(() => {
  let pending;
  let revision = 0;
  let lastProfile;
  let submitting = false;
  let transition;
  async function replaceView(current, next, commit = () => {}) {
    clearPasswords();
    const update = () => {
      commit();
      current.replaceWith(document.importNode(next, true));
      document.dispatchEvent(new Event("login:view"));
    };
    if (!document.startViewTransition || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) { update(); return; }
    next.dataset.transitioned = "true";
    transition?.skipTransition();
    document.documentElement.dataset.loginTransition = "true";
    const active = document.startViewTransition(update);
    transition = active;
    active.ready.catch(() => {});
    active.finished.catch(() => {}).finally(() => {
      if (transition === active) delete document.documentElement.dataset.loginTransition;
    });
    await active.updateCallbackDone;
  }
  function clearPasswords() {
    document.querySelectorAll('.login-profiles input[type="password"]').forEach(input => { input.value = ""; });
  }
  async function navigate(url, push) {
    pending?.abort();
    const request = new AbortController();
    pending = request;
    const version = ++revision;
    const current = document.querySelector(".login-profiles");
    if (!current) { location.assign(url.href); return; }
    current.setAttribute("aria-busy", "true");
    try {
      const response = await fetch(url.href, { signal: request.signal, credentials: "same-origin", cache: "no-store", redirect: "error" });
      if (!response.ok || !response.headers.get("content-type")?.includes("text/html")) throw new Error("Login view unavailable");
      const parsed = new DOMParser().parseFromString(await response.text(), "text/html");
      const next = parsed.querySelector(".login-profiles");
      if (!next) throw new Error("Missing login view");
      if (version !== revision) return;
      // Do not retain credentials in detached DOM, browser history or caches.
      await replaceView(current, next, () => {
        if (version !== revision) throw new Error("Superseded navigation");
        if (push) history.pushState(null, "", url.href);
      });
      if (version !== revision) return;
      const view = document.querySelector(".login-profiles");
      const profile = [...view.querySelectorAll(".account")].find(link => link.href === lastProfile);
      const focus = view.querySelector("input[autofocus]") || profile || view.querySelector(".account, input:not([type=hidden]), .login-action");
      focus?.focus({ preventScroll: true });
    } catch (error) {
      if (request.signal.aborted || version !== revision) return;
      // Preserve the native navigation fallback on a network/server failure.
      location.assign(url.href);
    } finally {
      if (version === revision) current.removeAttribute("aria-busy");
    }
  }
  document.addEventListener("click", event => {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const link = event.target.closest?.('.login-profiles a.account, .login-profiles a.login-action');
    if (!link || link.target || link.hasAttribute("download")) return;
    const url = new URL(link.href);
    if (url.origin !== location.origin || url.pathname !== "/login") return;
    event.preventDefault();
    if (submitting) return;
    if (link.classList.contains("account")) lastProfile = link.href;
    void navigate(url, true);
  });
  window.addEventListener("popstate", () => {
    if (location.pathname === "/login") { submitting = false; void navigate(new URL(location.href), false); }
  });
  document.addEventListener("submit", event => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !form.matches("[data-login-form]") || new URL(form.action).pathname !== "/login") {
      ++revision; pending?.abort(); return;
    }
    event.preventDefault();
    if (submitting) return;
    pending?.abort();
    pending = new AbortController();
    const request = pending;
    const version = ++revision;
    const de = document.documentElement.lang === "de";
    const button = form.querySelector('button[type="submit"]');
    const status = document.createElement("p");
    status.className = "login-status";
    status.setAttribute("role", "status");
    form.querySelector(".login-status")?.remove();
    form.append(status);
    const message = text => { status.textContent = text; };
    if (!navigator.onLine) {
      message(de ? "Du bist offline. Bitte verbinde dich und versuche es erneut." : "You’re offline. Reconnect and try again.");
      return;
    }
    const body = new URLSearchParams();
    for (const [key, value] of new FormData(form)) if (typeof value === "string") body.append(key, value);
    submitting = true;
    form.setAttribute("aria-busy", "true");
    if (button) button.disabled = true;
    message(de ? "Anmeldung läuft …" : "Signing in …");
    let leaving = false;
    const timeout = setTimeout(() => request.abort(), 30000);
    void (async () => {
      try {
        const response = await fetch(form.action, {
          method: "POST", body, signal: request.signal, credentials: "same-origin", cache: "no-store", redirect: "error",
          headers: { Accept: "application/json", "Content-Type": "application/x-www-form-urlencoded" }
        });
        body.delete("password");
        if (version !== revision) return;
        if (response.ok && response.headers.get("content-type")?.includes("application/json")) {
          const payload = await response.json();
          const target = typeof payload.next === "string" ? new URL(payload.next, location.origin) : null;
          if (!target || target.origin !== location.origin || !payload.next.startsWith("/") || payload.next.startsWith("//") || /^\/(login|logout|api|oauth2|offline)(\/|$)/.test(target.pathname)) throw new Error("Invalid destination");
          clearPasswords();
          message(de ? "Dein Arbeitsbereich wird geöffnet …" : "Opening your workspace …");
          leaving = true;
          location.replace(target.href);
          return;
        }
        if (response.status === 401 && response.headers.get("content-type")?.includes("text/html")) {
          const next = new DOMParser().parseFromString(await response.text(), "text/html").querySelector(".login-profiles");
          const current = document.querySelector(".login-profiles");
          if (next && current && version === revision) {
            await replaceView(current, next, () => {
              if (version !== revision) throw new Error("Superseded sign-in");
            });
            document.querySelector('.login-profiles input[type="password"]')?.focus();
            return;
          }
        }
        message(response.status === 429
          ? (de ? "Zu viele Anmeldeversuche. Bitte warte kurz." : "Too many sign-in attempts. Please wait a moment.")
          : (de ? "Anmeldung derzeit nicht möglich. Bitte versuche es erneut." : "Sign-in is unavailable. Please try again."));
      } catch {
        if (version === revision) message(de ? "Keine Antwort vom Server. Bitte versuche es erneut." : "No response from the server. Please try again.");
      } finally {
        clearTimeout(timeout);
        body.delete("password");
        if (version === revision && !leaving) {
          submitting = false;
          form.removeAttribute("aria-busy");
          if (button) button.disabled = false;
        }
      }
    })();
  }, true);
  window.addEventListener("pageshow", () => {
    submitting = false;
    document.querySelectorAll('[data-login-form]').forEach(form => {
      form.removeAttribute("aria-busy");
      const button = form.querySelector('button[type="submit"]');
      if (button) button.disabled = false;
    });
  });
  window.addEventListener("pagehide", () => { ++revision; pending?.abort(); clearPasswords(); });
})();
