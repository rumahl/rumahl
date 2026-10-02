import { Component, type PropsWithChildren, type ReactNode } from "react";
import { useI18n } from "../i18n";
import { Button, buttonClass } from "../components/Button";
import { ShellLink } from "./ShellLink";
import "./route-failure.css";

type BoundaryProps = PropsWithChildren<{ fallback: (retry: () => void) => ReactNode; location: string }>;
class Boundary extends Component<BoundaryProps, { failed: boolean }> {
  override state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  override componentDidUpdate(previous: BoundaryProps) {
    // Retry a failed view after navigation without remounting healthy layouts.
    if (previous.location !== this.props.location && this.state.failed) this.setState({ failed: false });
  }
  private retry = () => this.setState({ failed: false });
  override render() {
    return this.state.failed ? this.props.fallback(this.retry) : this.props.children;
  }
}
export function RouteBoundary({ children, location }: PropsWithChildren<{ location: string }>) {
  const { t } = useI18n();
  return <Boundary location={location} fallback={retry => <section className="route-failure" aria-label={t("route.errorTitle")}>
    <div className="route-failure__content">
      <div className="route-failure__icon" aria-hidden="true">
        <svg viewBox="0 0 48 48" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="7" y="9" width="34" height="30" rx="7" /><path d="M7 18h34M13 13.5h.01M17 13.5h.01M24 24v6M24 34h.01" />
        </svg>
      </div>
      <div role="alert"><h2>{t("route.errorTitle")}</h2><p>{t("route.error")}</p></div>
      <div className="route-failure__actions">
        <Button onClick={retry}>{t("route.retry")}</Button>
        <ShellLink to="/apps" className={buttonClass("ghost")}>{t("route.backToApps")}</ShellLink>
      </div>
      <small>{t("route.errorHint")}</small>
    </div>
  </section>}>{children}</Boundary>;
}
