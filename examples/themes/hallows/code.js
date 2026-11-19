/*
 * Hallows · theme code.
 *
 * This is a normal rumahl theme package: it only uses the SDK surface handed to
 * `activate(api)` — the shell root, a background layer behind the windows, an
 * overlay above them, the resolved tokens and the motion state. Everything the
 * theme does (blood, light failures, presence, VHS, no signal, sound) lives
 * here, not in the shell.
 */

const EVENT_DURATION = { blackout: 1300, red: 1350, flicker: 1700, glitch: 420, signal: 5200, unstable: 900 };

function pick(list) {
  return list.length ? list[Math.floor(Math.random() * list.length)] : null;
}

function word() {
  return pick(["HILFE", "SIEH MICH AN", "HINTER DIR", "DON'T LOOK", "HELP ME", "NO SIGNAL"]);
}

export default function activate(api) {
  const root = api.root;
  const background = api.background;
  const overlay = api.overlay;
  const tokens = api.state.tokens;
  const animated = api.state.animations;
  const on = (token) => tokens[token] === "1";

  const timers = new Set();
  const nodes = [];
  const listeners = [];

  const later = (fn, ms) => {
    const id = window.setTimeout(() => {
      timers.delete(id);
      fn();
    }, ms);
    timers.add(id);
    return id;
  };

  const mount = (parent, tag, className) => {
    const element = document.createElement(tag);
    element.className = className;
    element.setAttribute("aria-hidden", "true");
    parent.append(element);
    nodes.push(element);
    return element;
  };

  const listen = (target, type, handler, options) => {
    target.addEventListener(type, handler, options);
    listeners.push(() => target.removeEventListener(type, handler, options));
  };

  /* ---- frequency ------------------------------------------------------- */
  const level = () => {
    const value = Number.parseFloat(tokens["effect.dread"] ?? "0");
    return Number.isFinite(value) ? Math.min(Math.max(value, 0), 100) : 0;
  };
  // 6 h at level 1, ~3.6 min at level 100. Not a fixed timer: exponential.
  const meanMs = () => {
    const value = level();
    return value > 0 ? 6 * 60 * 60 * 1000 * Math.pow(0.01, value / 100) : 0;
  };
  const sampleDelay = () => {
    const mean = meanMs();
    if (mean <= 0) return 0;
    const random = Math.min(Math.max(Math.random(), 0.0001), 0.9999);
    return Math.min(Math.max(-Math.log(1 - random) * mean, 3000), 6 * 60 * 60 * 1000);
  };

  /* ---- blood ----------------------------------------------------------- */
  const drips = [];
  if (on("effect.blood")) {
    const field = mount(background, "div", "hallows-blood");
    field.dataset.animated = animated ? "true" : "false";
    const lanes = 18;
    for (let index = 0; index < lanes; index += 1) {
      const drip = document.createElement("i");
      drip.setAttribute("aria-hidden", "true");
      field.append(drip);
      nodes.push(drip);
      drips.push(drip);
    }
    const spawn = (drip) => {
      const duration = 5.5 + Math.random() * 6.5;
      const delay = 1.2 + Math.random() * 4.5;
      drip.style.left = `${Math.random() * 100}%`;
      drip.style.width = `${1.5 + Math.random() * 9}px`;
      drip.style.setProperty("--hallows-h", `${70 + Math.random() * 340}px`);
      drip.style.animation = "none";
      void drip.offsetWidth;
      drip.style.animation = "";
      drip.style.animationDelay = `${delay}s`;
      drip.style.animationDuration = `${duration}s`;
      drip.style.setProperty("--hallows-drop", `${(duration * 0.12).toFixed(2)}s`);
      drip.style.setProperty("--hallows-drop-delay", `${(delay + duration * 0.66).toFixed(2)}s`);
      drip.style.setProperty("--hallows-drop-delay2", `${(delay + duration * 0.82).toFixed(2)}s`);
      if (!animated) return;
      later(() => spawn(drip), (delay + duration + 1.2) * 1000);
    };
    for (const drip of drips) spawn(drip);
  }

  /* ---- no signal ------------------------------------------------------- */
  const signalLayer = mount(background, "span", "hallows-nosignal");

  /* ---- VHS surface ----------------------------------------------------- */
  const screen = mount(overlay, "div", "hallows-screen");
  if (on("effect.vhs")) {
    mount(screen, "span", "hallows-scanlines");
    mount(screen, "span", "hallows-noise");
    mount(screen, "span", "hallows-tracking");
  }
  mount(screen, "span", "hallows-bands");

  /* ---- audio ----------------------------------------------------------- */
  let audio = null;
  let armed = false;
  const unlock = () => {
    try {
      audio ??= new AudioContext();
      void audio.resume().catch(() => undefined);
    } catch {
      audio = null;
    }
  };
  const arm = () => {
    if (armed) return;
    armed = true;
    listen(window, "pointerdown", unlock, { once: true, passive: true });
    listen(window, "keydown", unlock, { once: true });
  };

  const play = (event) => {
    if (!audio || audio.state !== "running") return;
    try {
      const now = audio.currentTime;
      const heavy = event === "blackout" || event === "signal";
      const tone = audio.createOscillator();
      const gain = audio.createGain();
      tone.type = event === "glitch" ? "square" : "sawtooth";
      tone.frequency.setValueAtTime(event === "red" ? 118 : event === "glitch" ? 640 : 84, now);
      tone.frequency.exponentialRampToValueAtTime(event === "glitch" ? 180 : 30, now + 0.9);
      gain.gain.setValueAtTime(0.0001, now);
      gain.gain.exponentialRampToValueAtTime(heavy ? 0.14 : 0.09, now + 0.05);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + 1.0);
      tone.connect(gain).connect(audio.destination);
      tone.start(now);
      tone.stop(now + 1.05);

      const length = Math.floor(audio.sampleRate * 0.5);
      const buffer = audio.createBuffer(1, length, audio.sampleRate);
      const data = buffer.getChannelData(0);
      for (let index = 0; index < length; index += 1) {
        data[index] = (Math.random() * 2 - 1) * (1 - index / length) ** (event === "signal" ? 0.7 : 2);
      }
      const noise = audio.createBufferSource();
      noise.buffer = buffer;
      const filter = audio.createBiquadFilter();
      filter.type = "bandpass";
      filter.frequency.value = event === "signal" ? 2600 : event === "red" ? 900 : 1900;
      filter.Q.value = 0.8;
      const noiseGain = audio.createGain();
      noiseGain.gain.value = event === "signal" ? 0.09 : 0.07;
      noise.connect(filter).connect(noiseGain).connect(audio.destination);
      noise.start(now);
    } catch {
      /* sound is optional */
    }
  };

  /* ---- presence -------------------------------------------------------- */
  let presence = null;
  let presenceTimer = 0;
  const hidePresence = () => {
    if (presence) presence.remove();
    presence = null;
    window.clearTimeout(presenceTimer);
  };
  const dismissOnDrag = () => {
    if (document.querySelector(".window-position.is-dragging, .window-position.is-moving")) hidePresence();
  };
  if (on("effect.presence") && animated) {
    listen(window, "pointermove", dismissOnDrag, { passive: true });
    listen(window, "pointerdown", dismissOnDrag, { passive: true });
    const schedulePresence = () => {
      const delay = sampleDelay() * 1.6;
      later(() => {
        if (document.visibilityState === "visible") {
          hidePresence();
          presence = mount(background, "span", "hallows-presence");
          presence.style.left = `${10 + Math.random() * 80}%`;
          presenceTimer = window.setTimeout(() => {
            hidePresence();
            schedulePresence();
          }, 5000 + Math.random() * 4500);
        } else {
          schedulePresence();
        }
      }, delay);
    };
    schedulePresence();
  }

  /* ---- events ---------------------------------------------------------- */
  const enabled = [
    on("effect.blackout") ? "blackout" : null,
    on("effect.red") ? "red" : null,
    on("effect.flicker") ? "flicker" : null,
    on("effect.glitch") ? "glitch" : null,
    on("effect.signal") ? "signal" : null,
    on("effect.unstable") ? "unstable" : null
  ].filter(Boolean);

  const clearEvent = () => {
    delete root.dataset.dread;
    delete root.dataset.signal;
    root.style.removeProperty("--hallows-word");
  };

  if (animated && enabled.length > 0 && meanMs() > 0) {
    arm();
    const fire = () => {
      const event = pick(enabled);
      if (!event) return;
      clearEvent();
      root.dataset.dread = event;
      if (event === "signal") root.dataset.signal = "lost";
      if (event === "unstable") root.style.setProperty("--hallows-word", JSON.stringify(word()));
      play(event);
      later(clearEvent, EVENT_DURATION[event] ?? 1300);
    };
    const schedule = () => {
      later(() => {
        if (document.visibilityState === "visible") fire();
        schedule();
      }, sampleDelay());
    };
    schedule();
  }

  /* ---- ambient light --------------------------------------------------- */
  if (animated && on("effect.flicker")) {
    const stutter = () => {
      later(() => {
        root.dataset.flicker = "true";
        later(() => {
          delete root.dataset.flicker;
          stutter();
        }, 180 + Math.random() * 520);
      }, 3400 + Math.random() * 9000);
    };
    stutter();
  }

  /* ---- teardown -------------------------------------------------------- */
  return () => {
    for (const id of timers) window.clearTimeout(id);
    timers.clear();
    window.clearTimeout(presenceTimer);
    for (const remove of listeners) remove();
    for (const node of nodes) node.remove();
    clearEvent();
    delete root.dataset.flicker;
    signalLayer.remove();
  };
}
