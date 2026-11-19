import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { useNavigate } from "react-router";
import { useI18n } from "../../i18n";
import type { MessageKey } from "../../i18n/locales/en";
import { useDisabledApps } from "../../preferences/disabled-apps";
import { useShellPreferences } from "../../preferences/ShellPreferences";
import { useShell } from "../../shell/ShellContext";
import {
  DEFAULT_PET_POSITION,
  clampPetPosition,
  getPetPosition,
  savePetPosition
} from "./pet-position";
import { setActivePet, updatePet, useZoo } from "./pet-store";
import { activePet, type PetZoo } from "./pet-zoo";
import { PetAvatar } from "./PetAvatar";
import { Pet3D } from "./Pet3D";
import { PetVitals } from "./PetVitals";
import { PET_ACTION_IDS, type PetBuiltinAction, type PetExtensionIcon } from "./pet-actions";
import { effectiveSleeping, usePetLife, type PetMood } from "./pet-life";
import {
  invokePetAction,
  publishPetInteraction,
  publishPetState,
  subscribePetCommands,
  usePetActions,
  type PetActionExtension
} from "./pet-extensions";

type ReactionKind = PetBuiltinAction | "custom";
type IdleMood = "look" | "stretch" | "doze" | "perk";

const REACTION_LINES: Record<PetBuiltinAction, readonly MessageKey[]> = {
  pet: ["pets.reaction.pet1", "pets.reaction.pet2", "pets.reaction.pet3"],
  play: ["pets.reaction.play1", "pets.reaction.play2", "pets.reaction.play3"],
  feed: ["pets.reaction.feed1", "pets.reaction.feed2", "pets.reaction.feed3"],
  nap: ["pets.reaction.nap1", "pets.reaction.nap2", "pets.reaction.nap3"],
  dance: ["pets.reaction.dance1", "pets.reaction.dance2", "pets.reaction.dance3"],
  talk: ["pets.reaction.talk1", "pets.reaction.talk2", "pets.reaction.talk3"],
  bath: ["pets.reaction.bath1", "pets.reaction.bath2", "pets.reaction.bath3"],
  boop: ["pets.reaction.boop1", "pets.reaction.boop2", "pets.reaction.boop3"],
  spin: ["pets.reaction.spin1", "pets.reaction.spin2", "pets.reaction.spin3"],
  stretch: ["pets.reaction.stretch1", "pets.reaction.stretch2", "pets.reaction.stretch3"],
  sing: ["pets.reaction.sing1", "pets.reaction.sing2", "pets.reaction.sing3"],
  peek: ["pets.reaction.peek1", "pets.reaction.peek2", "pets.reaction.peek3"],
  meditate: ["pets.reaction.meditate1", "pets.reaction.meditate2", "pets.reaction.meditate3"]
};

const IDLE_LINES: readonly MessageKey[] = [
  "pets.idle.look1",
  "pets.idle.look2",
  "pets.idle.stretch1",
  "pets.idle.doze1"
];

const LIFE_LINES: Partial<Record<PetMood, readonly MessageKey[]>> = {
  hungry: ["pets.life.hungry1", "pets.life.hungry2"],
  sleepy: ["pets.life.sleepy1", "pets.life.sleepy2"],
  sleeping: ["pets.life.sleeping1"],
  lonely: ["pets.life.lonely1"],
  cozy: ["pets.life.cozy1"],
  playful: ["pets.life.playful1"],
  curious: ["pets.idle.look1", "pets.idle.look2"]
};

interface DragState {
  pointerId: number;
  startX: number;
  startY: number;
  originX: number;
  originY: number;
  moved: boolean;
}

function pick<T>(values: readonly T[]): T {
  return values[Math.floor(Math.random() * values.length)]!;
}

function idleDelay(energy: "calm" | "balanced" | "playful", quiet: boolean): number {
  const base = energy === "calm" ? 26_000 : energy === "playful" ? 9_000 : 16_000;
  const width = base * (quiet ? 1.1 : 0.55);
  return base + Math.floor(Math.random() * width);
}

/**
 * The pet lives on the shell itself, not inside an app iframe. It sits above
 * the desktop/launcher surfaces but below the window layer, so an open app
 * naturally covers it.
 *
 * The active zoo pet is shown. Drag to move, click to pet, hover for the action
 * grid, or let it idle. Positions, quiet mode and the active pet are stored per
 * account on this device. Other shell programs can register extra actions and
 * dispatch built-in interactions through `pet-extensions.ts`.
 */
export function PetOverlay() {
  const { t } = useI18n();
  const { mode } = useShell();
  const { preferences } = useShellPreferences();
  const disabled = useDisabledApps();
  const navigate = useNavigate();
  const owner = preferences?.ownerId ?? null;
  const zoo = useZoo(owner);
  const pet = activePet(zoo)?.config ?? null;
  const petId = zoo?.activeId ?? null;
  const extensions = usePetActions();
  const { life, interact } = usePetLife(owner, petId, pet);
  const sleeping = effectiveSleeping(life, pet);
  const lifeMood: PetMood | null = life ? (sleeping && life.mood !== "sleeping" ? "sleeping" : life.mood) : null;
  const displayLife = life && lifeMood ? { ...life, mood: lifeMood } : life;

  const layerRef = useRef<HTMLDivElement>(null);
  const companionRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const position = useRef(DEFAULT_PET_POSITION);
  const drag = useRef<DragState | null>(null);
  const suppressClick = useRef(false);
  const reactionTimer = useRef<number | null>(null);
  const menuTimer = useRef<number | null>(null);
  const idleTimer = useRef<number | null>(null);
  const idleClearTimer = useRef<number | null>(null);
  const nonce = useRef(0);
  const reactionRef = useRef<ReactionKind | null>(null);
  const [dragging, setDragging] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [reaction, setReaction] = useState<{ kind: ReactionKind; nonce: number } | null>(null);
  const [speech, setSpeech] = useState<string | null>(null);
  const [mood, setMood] = useState<IdleMood | null>(null);

  const showMenu = useCallback(() => {
    setMenuOpen(true);
    if (menuTimer.current) window.clearTimeout(menuTimer.current);
    menuTimer.current = window.setTimeout(() => setMenuOpen(false), 9000);
  }, []);

  const reactTo = useCallback((kind: ReactionKind, options: { source?: string; speech?: string } = {}) => {
    const current = pet;
    if (!current || !petId) return;
    reactionRef.current = kind;
    nonce.current += 1;
    setMood(null);
    interact(kind);
    setReaction({ kind, nonce: nonce.current });
    const lines = kind === "custom" ? null : REACTION_LINES[kind];
    setSpeech(options.speech ?? (lines ? t(pick(lines)) : current.name));
    showMenu();
    if (reactionTimer.current) window.clearTimeout(reactionTimer.current);
    reactionTimer.current = window.setTimeout(() => {
      reactionRef.current = null;
      setReaction(null);
      setSpeech(null);
    }, 2200);
    publishPetInteraction({ action: kind, petId, ...(options.source ? { source: options.source } : {}) });
  }, [interact, pet, petId, showMenu, t]);

  useEffect(() => subscribePetCommands((command) => reactTo(command.action, command.source ? { source: command.source } : {})), [reactTo]);

  useEffect(() => {
    const next = getPetPosition(owner, petId ?? undefined);
    position.current = next;
    const companion = companionRef.current;
    if (!companion) return;
    companion.style.setProperty("--pet-x", String(next.x));
    companion.style.setProperty("--pet-y", String(next.y));
  }, [owner, petId, mode]);

  useEffect(() => {
    if (!pet || !petId) {
      publishPetState(null);
      return;
    }
    publishPetState({
      id: petId,
      name: pet.name,
      quiet: pet.quiet,
      render: pet.render,
      temperament: pet.energy,
      mood: lifeMood ?? "curious",
      vitality: life?.energy ?? 0,
      hunger: life?.hunger ?? 0,
      bond: life?.bond ?? 0,
      sleeping
    });
    return () => publishPetState(null);
  }, [pet?.name, pet?.quiet, pet?.render, pet?.energy, life, lifeMood, sleeping, petId]);

  // Idle life: calm/playful temperaments pick a different rhythm, and quiet
  // pets only doze or look around without speaking or spawning particles.
  useEffect(() => {
    if (!pet || !petId) return;
    let cancelled = false;
    const schedule = () => {
      idleTimer.current = window.setTimeout(() => {
        if (cancelled) return;
        if (!reactionRef.current) {
          const options: IdleMood[] = sleeping
            ? ["doze"]
            : pet.quiet
              ? ["doze", "look"]
              : pet.energy === "playful"
                ? ["perk", "stretch", "look"]
                : pet.energy === "calm"
                  ? ["doze", "look", "stretch"]
                  : ["look", "stretch", "doze"];
          setMood(pick(options));
          if (!pet.quiet) {
            const lines = (lifeMood && LIFE_LINES[lifeMood]) || IDLE_LINES;
            setSpeech(t(pick(lines)));
          }
          idleClearTimer.current = window.setTimeout(() => {
            if (!cancelled) {
              setMood(null);
              setSpeech(null);
            }
          }, 3_600 + Math.floor(Math.random() * 1_800));
        }
        schedule();
      }, idleDelay(pet.energy, pet.quiet));
    };
    schedule();
    return () => {
      cancelled = true;
      if (idleTimer.current) window.clearTimeout(idleTimer.current);
      if (idleClearTimer.current) window.clearTimeout(idleClearTimer.current);
    };
  }, [petId, pet?.quiet, pet?.energy, lifeMood, sleeping, t]);

  useEffect(() => () => {
    if (reactionTimer.current) window.clearTimeout(reactionTimer.current);
    if (menuTimer.current) window.clearTimeout(menuTimer.current);
    if (idleTimer.current) window.clearTimeout(idleTimer.current);
    if (idleClearTimer.current) window.clearTimeout(idleClearTimer.current);
  }, []);

  if (!pet || disabled.includes("pets")) return null;
  if (mode !== "desktop" && mode !== "launcher") return null;
  if (pet.placement !== "both" && pet.placement !== mode) return null;

  const trackGaze = (event: ReactPointerEvent<HTMLDivElement>) => {
    const companion = companionRef.current;
    if (!companion) return;
    const rect = companion.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    const x = Math.max(-1, Math.min(1, ((event.clientX - rect.left) / rect.width) * 2 - 1));
    const y = Math.max(-1, Math.min(1, ((event.clientY - rect.top) / rect.height) * 2 - 1));
    companion.style.setProperty("--pet-gaze-x", x.toFixed(3));
    companion.style.setProperty("--pet-gaze-y", y.toFixed(3));
  };

  const resetGaze = () => {
    companionRef.current?.style.setProperty("--pet-gaze-x", "0");
    companionRef.current?.style.setProperty("--pet-gaze-y", "0");
  };

  const applyPosition = (value: { x: number; y: number }) => {
    const companion = companionRef.current;
    if (!companion) return;
    companion.style.setProperty("--pet-x", String(value.x));
    companion.style.setProperty("--pet-y", String(value.y));
  };

  const beginDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    try {
      event.currentTarget.setPointerCapture(event.pointerId);
    } catch {
      // Pointer capture is an optimisation; dragging still works without it.
    }
    drag.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: position.current.x,
      originY: position.current.y,
      moved: false
    };
  };

  const moveDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const active = drag.current;
    if (!active || active.pointerId !== event.pointerId) return;
    const dx = event.clientX - active.startX;
    const dy = event.clientY - active.startY;
    if (!active.moved && Math.abs(dx) + Math.abs(dy) < 5) return;
    active.moved = true;
    setDragging(true);
    const rect = layerRef.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0 || rect.height <= 0) return;
    const next = clampPetPosition({
      x: active.originX + dx / rect.width,
      y: active.originY + dy / rect.height
    });
    position.current = next;
    applyPosition(next);
  };

  const endDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const active = drag.current;
    if (!active || active.pointerId !== event.pointerId) return;
    drag.current = null;
    try {
      event.currentTarget.releasePointerCapture(event.pointerId);
    } catch {
      // Ignore: the pointer may already be released.
    }
    if (active.moved) {
      savePetPosition(owner, position.current, petId ?? undefined);
      suppressClick.current = true;
      window.setTimeout(() => {
        suppressClick.current = false;
      }, 0);
    }
    setDragging(false);
  };

  const cancelDrag = () => {
    drag.current = null;
    setDragging(false);
  };

  const activatePet = () => {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    reactTo("pet");
  };

  const openStudio = () => {
    showMenu();
    navigate("/app/pets");
  };

  const switchPet = (id: string) => {
    if (!owner || id === petId) return;
    setActivePet(owner, id);
    setReaction(null);
    setSpeech(null);
    setMood(null);
    reactionRef.current = null;
    showMenu();
  };

  const toggleQuiet = () => {
    if (!owner || !petId) return;
    updatePet(owner, petId, { ...pet, quiet: !pet.quiet });
    if (!pet.quiet) {
      setSpeech(null);
      setReaction(null);
      reactionRef.current = null;
    }
    showMenu();
  };

  const runExtension = (action: PetActionExtension) => {
    reactTo("custom", { speech: action.label });
    invokePetAction(action.id);
  };

  const reactionEyes =
    reaction?.kind === "pet" || reaction?.kind === "feed" || reaction?.kind === "boop" || reaction?.kind === "bath"
      ? "happy"
      : reaction?.kind === "play" || reaction?.kind === "spin" || reaction?.kind === "dance" || reaction?.kind === "sing"
        ? "sparkle"
        : reaction?.kind === "nap" || reaction?.kind === "meditate" || reaction?.kind === "stretch"
          ? "sleepy"
          : reaction?.kind === "peek"
            ? "sparkle"
            : reaction?.kind === "custom"
              ? "sparkle"
              : mood === "doze" || sleeping || life?.mood === "sleepy"
                ? "sleepy"
                : lifeMood === "cozy" || lifeMood === "playful"
                  ? "happy"
                  : pet.eyes;

  const avatarPet = { ...pet, eyes: reactionEyes };
  const avatar = pet.render === "3d"
    ? <Pet3D pet={avatarPet} size={pet.size} />
    : <PetAvatar pet={avatarPet} size={pet.size} />;

  return (
    <div ref={layerRef} className={`pet-overlay pet-overlay--${mode}`} data-placement={pet.placement}>
      <div
        ref={companionRef}
        className={`pet-overlay__companion pet-body-color-${pet.bodyColor} pet-accent-color-${pet.accentColor} pet-material-${pet.material}${menuOpen ? " is-open" : ""}${reaction ? ` is-reacting is-reacting-${reaction.kind}` : ""}${dragging ? " is-dragging" : ""}${pet.quiet ? " is-quiet" : ""}${mood ? ` is-idle-${mood}` : ""}${lifeMood ? ` is-life-${lifeMood}` : ""}`}
        data-energy={pet.energy}
        data-render={pet.render}
        data-mood={lifeMood ?? undefined}
        onPointerMove={trackGaze}
        onPointerLeave={resetGaze}
        onPointerCancel={resetGaze}
      >
        <button
          ref={buttonRef}
          type="button"
          className="pet-overlay__button"
          aria-label={t("pets.interact", { name: pet.name })}
          title={pet.name}
          draggable={false}
          onDragStart={(event) => event.preventDefault()}
          onPointerDown={beginDrag}
          onPointerMove={moveDrag}
          onPointerUp={endDrag}
          onPointerCancel={cancelDrag}
          onLostPointerCapture={cancelDrag}
          onClick={activatePet}
        >
          <span key={reaction ? `${reaction.kind}-${reaction.nonce}` : `idle-${mood ?? "none"}`} className="pet-overlay__bob">
            {avatar}
          </span>
        </button>
        {sleeping ? <span className="pet-overlay__sleep" aria-hidden="true">z z</span> : null}
        <div className="pet-overlay__menu">
          <div className="pet-overlay__status">
            <span className="pet-overlay__name" role="status" aria-live="polite">
              {speech ?? pet.name}
            </span>
            {pet.quiet ? <span className="pet-overlay__quiet" title={t("pets.quiet")}>{t("pets.quietBadge")}</span> : null}
          </div>
          <PetVitals life={displayLife} compact />
          {zoo && zoo.pets.length > 1 ? (
            <ZooSwitcher zoo={zoo} onSelect={switchPet} />
          ) : null}
          <div className="pet-overlay__actions" role="toolbar" aria-label={t("pets.actions")}>
            {PET_ACTION_IDS.map((action) => (
              <ActionButton key={action} label={t(`pets.action.${action}` as MessageKey)} onClick={() => reactTo(action)}>
                <ActionIcon action={action} />
              </ActionButton>
            ))}
            {extensions.map((action) => (
              <ActionButton key={action.id} label={action.label} onClick={() => runExtension(action)}>
                <ExtensionIcon icon={action.icon} />
              </ActionButton>
            ))}
            <ActionButton label={pet.quiet ? t("pets.quietOff") : t("pets.quietOn")} onClick={toggleQuiet}>
              <QuietIcon quiet={pet.quiet} />
            </ActionButton>
            <ActionButton label={t("pets.action.edit")} onClick={openStudio}>
              <EditIcon />
            </ActionButton>
          </div>
        </div>

        {reaction ? <PetParticles key={reaction.nonce} kind={reaction.kind} quiet={pet.quiet} /> : null}
      </div>
    </div>
  );
}

function ZooSwitcher({ zoo, onSelect }: { zoo: PetZoo; onSelect: (id: string) => void }) {
  const { t } = useI18n();
  return (
    <div className="pet-overlay__zoo" role="group" aria-label={t("pets.zoo")}>
      {zoo.pets.map((pet) => (
        <button
          key={pet.id}
          type="button"
          className={`pet-overlay__zoo-pet${pet.id === zoo.activeId ? " is-active" : ""}`}
          aria-label={t("pets.switchPet", { name: pet.config.name })}
          aria-pressed={pet.id === zoo.activeId}
          title={pet.config.name}
          onClick={() => onSelect(pet.id)}
        >
          <PetAvatar pet={pet.config} size="xs" />
        </button>
      ))}
    </div>
  );
}

function ActionButton({
  label,
  onClick,
  children
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button type="button" className="pet-overlay__action" aria-label={label} title={label} onClick={onClick}>
      {children}
    </button>
  );
}

function PetParticles({ kind, quiet }: { kind: ReactionKind; quiet: boolean }) {
  if (kind === "talk") return null;
  return (
    <span className={`pet-overlay__particles pet-overlay__particles--${kind}${quiet ? " is-quiet" : ""}`} aria-hidden="true">
      <i />
      <i />
      <i />
      <i />
      {kind === "feed" ? <CookieIcon className="pet-overlay__cookie" /> : null}
    </span>
  );
}

function ActionIcon({ action }: { action: PetBuiltinAction }) {
  switch (action) {
    case "pet": return <HeartIcon />;
    case "play": return <SparkIcon />;
    case "feed": return <CookieIcon />;
    case "nap": return <MoonIcon />;
    case "dance": return <MusicIcon />;
    case "talk": return <TalkIcon />;
    case "bath": return <BubbleIcon />;
    case "boop": return <PawIcon />;
    case "spin": return <OrbitIcon />;
    case "stretch": return <StretchIcon />;
    case "sing": return <SingIcon />;
    case "peek": return <EyeIcon />;
    case "meditate": return <LotusIcon />;
  }
}

function ExtensionIcon({ icon }: { icon?: PetExtensionIcon | undefined }) {
  switch (icon) {
    case "heart": return <HeartIcon />;
    case "bolt": return <BoltIcon />;
    case "leaf": return <LotusIcon />;
    case "music": return <MusicIcon />;
    case "chat": return <TalkIcon />;
    case "moon": return <MoonIcon />;
    case "cookie": return <CookieIcon />;
    case "wave": return <WaveIcon />;
    case "orbit": return <OrbitIcon />;
    case "pulse": return <PulseIcon />;
    case "spark":
    case "star":
    default:
      return <SparkIcon />;
  }
}

function HeartIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 20.2S4.8 15.8 4.8 10.4A4.1 4.1 0 0 1 12 7.5a4.1 4.1 0 0 1 7.2 2.9c0 5.4-7.2 9.8-7.2 9.8Z" fill="currentColor" />
    </svg>
  );
}

function SparkIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 3.5 14 9.5 20.5 12 14 14.5 12 20.5 10 14.5 3.5 12 10 9.5Z" fill="currentColor" />
    </svg>
  );
}

function CookieIcon({ className = "" }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="12" cy="12" r="8.2" fill="#d9aa72" />
      <circle cx="9" cy="9" r="1.3" fill="#6b4425" />
      <circle cx="14.5" cy="8" r="1" fill="#6b4425" />
      <circle cx="16" cy="13" r="1.4" fill="#6b4425" />
      <circle cx="10.5" cy="15.5" r="1" fill="#6b4425" />
    </svg>
  );
}

function MoonIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M20 14.5A8.5 8.5 0 1 1 9.5 4 6.6 6.6 0 0 0 20 14.5Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
    </svg>
  );
}

function MusicIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M9 18V6l10-2v11" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
      <ellipse cx="6.5" cy="18" rx="2.5" ry="2" fill="currentColor" />
      <ellipse cx="16.5" cy="15" rx="2.5" ry="2" fill="currentColor" />
    </svg>
  );
}

function TalkIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M4 5.5h16v10H9l-5 4v-4Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
      <path d="M8 9h8M8 12h5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

function BubbleIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="8" cy="10" r="4.2" fill="none" stroke="currentColor" strokeWidth="1.7" />
      <circle cx="15.5" cy="8" r="3.2" fill="none" stroke="currentColor" strokeWidth="1.7" />
      <circle cx="16" cy="15.5" r="4.6" fill="none" stroke="currentColor" strokeWidth="1.7" />
    </svg>
  );
}

function PawIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <ellipse cx="12" cy="15.5" rx="5.2" ry="4.4" fill="currentColor" />
      <ellipse cx="5.6" cy="10.5" rx="2.2" ry="2.8" fill="currentColor" />
      <ellipse cx="9.4" cy="7.2" rx="2" ry="2.7" fill="currentColor" />
      <ellipse cx="14.6" cy="7.2" rx="2" ry="2.7" fill="currentColor" />
      <ellipse cx="18.4" cy="10.5" rx="2.2" ry="2.8" fill="currentColor" />
    </svg>
  );
}

function OrbitIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <g fill="none" stroke="currentColor" strokeWidth="1.8">
        <circle cx="12" cy="12" r="3.1" />
        <ellipse cx="12" cy="12" rx="9" ry="4.2" transform="rotate(-28 12 12)" />
      </g>
    </svg>
  );
}

function StretchIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 3v18M3 12h18M5.5 5.5l2.6 2.6M18.5 5.5l-2.6 2.6M5.5 18.5l2.6-2.6M18.5 18.5l-2.6-2.6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function SingIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M8 18V7l9-2v10" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
      <ellipse cx="5.5" cy="18" rx="2.6" ry="2" fill="currentColor" />
      <ellipse cx="14.5" cy="15" rx="2.6" ry="2" fill="currentColor" />
      <path d="M20 7h2M20 11h2" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

function EyeIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M2.5 12S6 6.5 12 6.5 21.5 12 21.5 12 18 17.5 12 17.5 2.5 12 2.5 12Z" fill="none" stroke="currentColor" strokeWidth="1.8" />
      <circle cx="12" cy="12" r="3" fill="currentColor" />
    </svg>
  );
}

function LotusIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 20c-4.5 0-8-2.5-9-6 3-.4 5.3.5 6.8 1.7C9 12 9.6 8.6 12 5c2.4 3.6 3 7 2.2 10.7C15.7 14.5 18 13.6 21 14c-1 3.5-4.5 6-9 6Z" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
    </svg>
  );
}

function BoltIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M13.5 2.5 5 13h5.5L10 21.5 19 10h-5.5Z" fill="currentColor" />
    </svg>
  );
}

function WaveIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M4 12c2-5 4-5 6 0s4 5 6 0 2.5-4 4-2" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
    </svg>
  );
}

function PulseIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M2.5 12h4l2-5 3.5 11 2.5-7 2 1h5" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function QuietIcon({ quiet }: { quiet: boolean }) {
  return quiet ? (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M5 9v6h4l5 4V5L9 9Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
      <path d="m18 9 4 6M22 9l-4 6" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  ) : (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M5 9v6h4l5 4V5L9 9Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
      <path d="M17 9.5a4 4 0 0 1 0 5M19.5 7a7.5 7.5 0 0 1 0 10" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  );
}

function EditIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="m5 19 3.8-.8L19 8a2.1 2.1 0 0 0-3-3L5.8 15.2Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
      <path d="m13.5 6.5 4 4" fill="none" stroke="currentColor" strokeWidth="1.8" />
    </svg>
  );
}
