import { GroupFooter } from "@rumahl/ui/components";
import { Button } from "./Button";

export function SettingsReset({ label, description, disabled, onReset }: {
  label: string;
  description: string;
  disabled: boolean;
  onReset: () => void;
}) {
  return <GroupFooter description={description}>
    <Button size="sm" disabled={disabled} onClick={onReset}>{label}</Button>
  </GroupFooter>;
}
