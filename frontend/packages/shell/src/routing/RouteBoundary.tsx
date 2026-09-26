import { Component, type PropsWithChildren } from "react";
import { useI18n } from "../i18n";

type BoundaryProps = PropsWithChildren<{ message: string; location: string }>;
class Boundary extends Component<BoundaryProps, { failed: boolean }> {
  override state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  override componentDidUpdate(previous: BoundaryProps) {
    // Retry a failed view after navigation without remounting healthy layouts.
    if (previous.location !== this.props.location && this.state.failed) {
      this.setState({ failed: false });
    }
  }
  override render() {
    return this.state.failed ? <p role="alert">{this.props.message}</p> : this.props.children;
  }
}
export function RouteBoundary({ children, location }: PropsWithChildren<{ location: string }>) {
  const { t } = useI18n();
  return <Boundary location={location} message={t("route.error")}>{children}</Boundary>;
}
