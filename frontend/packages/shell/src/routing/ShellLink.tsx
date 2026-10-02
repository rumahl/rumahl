import { Link, type LinkProps } from "react-router";
import { localPath } from "./paths";
export function ShellLink({ to, ...props }: Omit<LinkProps, "to"> & { to: string }) {
  return <Link {...props} to={localPath(to)} />;
}
