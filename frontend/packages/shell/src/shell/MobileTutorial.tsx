import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { GridIcon, HomeIcon, SlidersIcon, WindowStackIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShell } from "./ShellContext";

const TOUR_KEY = "rumahl.mobile.tour.v1";
export const MOBILE_TOUR_EVENT = "rumahl:mobile-tour";

export function openMobileTour() {
  if (typeof window !== "undefined") window.dispatchEvent(new Event(MOBILE_TOUR_EVENT));
}

function tourSeen(): boolean {
  if (typeof localStorage === "undefined") return true;
  try { return localStorage.getItem(TOUR_KEY) === "done"; } catch { return true; }
}

function rememberTour() {
  try { localStorage.setItem(TOUR_KEY, "done"); } catch { /* private mode */ }
}

function TourVisual({ step }: { step: number }) {
  if (step === 0) return <span className="mobile-tour__glyph"><GridIcon aria-hidden="true" /></span>;
  if (step === 1) return <span className="mobile-tour__glyph"><HomeIcon aria-hidden="true" /><WindowStackIcon aria-hidden="true" /></span>;
  if (step === 2) return <span className="mobile-tour__glyph mobile-tour__glyph--dock"><span /><span /><span /><span /></span>;
  return <span className="mobile-tour__glyph"><SlidersIcon aria-hidden="true" /></span>;
}

/** A short, visible first-run explanation of the phone-specific shell. */
export function MobileTutorial({ phone }: { phone: boolean }) {
  const { t } = useI18n();
  const { mode } = useShell();
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState(0);
  const steps = [1, 2, 3, 4] as const;
  const titles = [t("tour.step1.title"), t("tour.step2.title"), t("tour.step3.title"), t("tour.step4.title")];
  const bodies = [t("tour.step1.body"), t("tour.step2.body"), t("tour.step3.body"), t("tour.step4.body")];

  useEffect(() => {
    if (!phone || (mode !== "launcher" && mode !== "mobile") || tourSeen()) return;
    const timer = window.setTimeout(() => { setStep(0); setOpen(true); }, 1200);
    return () => window.clearTimeout(timer);
  }, [phone, mode]);

  useEffect(() => {
    const replay = () => { setStep(0); setOpen(true); };
    window.addEventListener(MOBILE_TOUR_EVENT, replay);
    return () => window.removeEventListener(MOBILE_TOUR_EVENT, replay);
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") finish();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function finish() {
    rememberTour();
    setOpen(false);
  }

  if (!open || !phone || (mode !== "launcher" && mode !== "mobile") || typeof document === "undefined") return null;
  const isLast = step === steps.length - 1;

  return createPortal(
    <div className="mobile-tour-overlay">
      <section className="mobile-tour" role="dialog" aria-modal="true" aria-label={t("tour.eyebrow")}>
        <header className="mobile-tour__head">
          <span className="mobile-tour__eyebrow">{t("tour.eyebrow")}</span>
          <button type="button" className="mobile-tour__skip" onClick={finish}>{t("tour.skip")}</button>
        </header>
        <div className="mobile-tour__visual" data-step={step}>
          <TourVisual step={step} />
        </div>
        <div className="mobile-tour__body" aria-live="polite">
          <span className="mobile-tour__count">{step + 1} / {steps.length}</span>
          <h2>{titles[step]}</h2>
          <p>{bodies[step]}</p>
        </div>
        <footer className="mobile-tour__footer">
          <div className="mobile-tour__dots" aria-hidden="true">
            {steps.map((item) => <span key={item} data-active={item === steps[step] ? "true" : undefined} />)}
          </div>
          <div className="mobile-tour__actions">
            {step > 0 ? <button type="button" className="mobile-tour__back" onClick={() => setStep((value) => value - 1)}>{t("tour.back")}</button> : null}
            <button type="button" className="mobile-tour__next" onClick={() => { if (isLast) finish(); else setStep((value) => value + 1); }}>
              {isLast ? t("tour.start") : t("tour.next")}
            </button>
          </div>
        </footer>
      </section>
    </div>,
    document.body
  );
}
