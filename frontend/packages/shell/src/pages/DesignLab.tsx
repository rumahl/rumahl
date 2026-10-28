import { RumahlModal, RumahlDialog } from "../components/RumahlDialog";
import {
  RumahlSearch,
  RumahlStepper,
  RumahlBadge,
  RumahlNotice,
  RumahlTabs,
} from "../components/RumahlOsControls";
import { RumahlButtonGroup } from "../components/RumahlButtonGroup";
import {
  RumahlAvatar,
  RumahlProfileCard,
  RumahlProfileSwitch,
  RumahlPassword,
  RumahlAvatarChange,
} from "../components/RumahlProfile";
import { RumahlCalendar, RumahlDatePicker } from "../components/RumahlCalendar";
import { RumahlTooltip } from "../components/RumahlTooltip";
import { RumahlProgress } from "../components/RumahlProgress";
import { useState } from "react";
import { Button } from "../components/Button";
import {
  RumahlFieldGroup,
  RumahlMultiSelect,
  RumahlSwitch,
  RumahlRange,
  RumahlFileInput,
  RumahlInputGroup,
  RumahlInput,
  RumahlTextArea,
  RumahlCheckbox,
  RumahlRadio,
} from "../components/RumahlInputs";
import { RumahlSelect } from "../components/RumahlSelect";
import { RumahlColorPicker } from "../components/RumahlColorPicker";

/** Deliberately absent from navigation and the app registry. */
export function DesignLab() {
  const [modal, setModal] = useState(false);
  const [dialog, setDialog] = useState(false);
  const [folder, setFolder] = useState("New folder");
  const [folderError, setFolderError] = useState("");
  const [outcome, setOutcome] = useState("");
  const [location, setLocation] = useState("Home");
  const [groups, setGroups] = useState<string[]>(["users"]);
  const [permissions, setPermissions] = useState<string[]>(["read"]);
  const [host, setHost] = useState("rumahl.local");
  const [files, setFiles] = useState("");
  const [search, setSearch] = useState("");
  const [volume, setVolume] = useState(50);
  const [tab, setTab] = useState("general");
  const [profile, setProfile] = useState("personal");
  const [avatarName, setAvatarName] = useState("");
  const profiles = [
    { id: "personal", name: "Alex Morgan", detail: "Personal · Demo" },
    { id: "work", name: "Alex Morgan", detail: "Work · Demo" },
    { id: "guest", name: "Guest", detail: "Guest · Demo" },
  ];
  const [date, setDate] = useState("2026-09-30");
  const [text, setText] = useState("rumahl");
  const [choice, setChoice] = useState("auto");
  const [checked, setChecked] = useState(true);
  const [radio, setRadio] = useState("a");
  const [toggle, setToggle] = useState(true);
  const [range, setRange] = useState(45);
  const [color, setColor] = useState("#6989d9");
  const [formats, setFormats] = useState<string[]>(["Bold"]);
  const [segment, setSegment] = useState("Auto");
  return (
    <section className="settings-content display-settings design-lab">
      <header className="settings-page-heading">
        <h1>design.rl</h1>
        <p>Component playground · lokale Testwerte / local test values</p>
      </header>
      <fieldset>
        <legend>Profile &amp; account · Demo</legend>
        <div className="design-lab__grid">
          <RumahlProfileCard
            profile={profiles.find((p) => p.id === profile)!}
            status="Local preview"
          />
          <RumahlProfileSwitch
            profiles={profiles}
            value={profile}
            onChange={(id) => {
              setProfile(id);
              setAvatarName("");
            }}
          />
          <div className="design-lab__buttons">
            <RumahlAvatar name="Alex Morgan" size="sm" />
            <RumahlAvatar name="Alex Morgan" status="online" />
            <RumahlAvatar name="Guest" size="lg" status="away" />
          </div>
          <RumahlPassword
            label="Account password"
            autoComplete="new-password"
            placeholder="Password"
          />
          <RumahlPassword
            label="Disabled password"
            disabled
            defaultValue="example"
          />
        </div>
        <RumahlAvatarChange
          key={profile}
          name={profiles.find((p) => p.id === profile)!.name}
          onChange={(file) => setAvatarName(file?.name ?? "")}
        />
        <p role="status">{avatarName}</p>
      </fieldset>
      <fieldset>
        <legend>Text &amp; validation</legend>
        <div className="design-lab__grid">
          <label>
            Text
            <RumahlInput
              value={text}
              onChange={(e) => setText(e.target.value)}
            />
          </label>
          <label>
            Search
            <RumahlInput type="search" placeholder="Search…" />
          </label>
          <label>
            Number
            <RumahlInput type="number" defaultValue={12} min={0} max={100} />
          </label>
          <label>
            Password
            <RumahlInput type="password" defaultValue="example" />
          </label>
          <label>
            Disabled
            <RumahlInput disabled value="Disabled" />
          </label>
          <label>
            Read only
            <RumahlInput readOnly value="Read only" />
          </label>
          <label>
            Invalid
            <RumahlInput
              error="This value is not allowed."
              defaultValue="invalid"
            />
          </label>
          <label>
            Success
            <RumahlInput
              status="success"
              aria-describedby="design-success"
              defaultValue="alex"
            />
            <small id="design-success">Username available</small>
          </label>
          <label>
            Website
            <RumahlInputGroup prefix="https://" placeholder="example.com" />
          </label>
          <label>
            Storage limit
            <RumahlInputGroup
              type="number"
              min={0}
              defaultValue={256}
              suffix="MB"
            />
          </label>
          <label>
            Search with action
            <RumahlInputGroup
              type="search"
              placeholder="Search…"
              suffix={
                <Button size="sm" onClick={() => setText("Search")}>
                  Search
                </Button>
              }
            />
          </label>
          <label>
            Notes
            <RumahlTextArea placeholder="Notes…" />
          </label>
        </div>
      </fieldset>
      <fieldset>
        <legend>OS settings &amp; groups · local preview</legend>
        <div className="design-lab__grid">
          <RumahlMultiSelect
            label="User groups"
            name="groups"
            value={groups}
            onChange={setGroups}
            status={groups.length ? "success" : "error"}
            description={
              groups.length
                ? "Group selection ready to save"
                : "Select at least one group"
            }
            options={[
              { value: "users", label: "Users" },
              { value: "admins", label: "Administrators" },
              { value: "developers", label: "Developers" },
              { value: "guests", label: "Guests", disabled: true },
            ]}
          />
          <RumahlMultiSelect
            label="Filesystem permissions"
            value={permissions}
            onChange={setPermissions}
            description="Permissions preview for the selected folder"
            options={[
              { value: "read", label: "Read files" },
              { value: "write", label: "Write files" },
              { value: "execute", label: "Execute applications" },
            ]}
          />
          <RumahlFieldGroup
            label="Network"
            status={
              /^[a-z0-9]+([.-][a-z0-9]+)*$/i.test(host) ? "success" : "error"
            }
            description={
              /^[a-z0-9]+([.-][a-z0-9]+)*$/i.test(host)
                ? "Hostname format is valid (local check)"
                : "Use letters and numbers separated by dots or hyphens"
            }
          >
            <label>
              Hostname
              <RumahlInput
                value={host}
                onChange={(event) => setHost(event.target.value)}
                spellCheck={false}
              />
            </label>
            <label>
              Port
              <RumahlInputGroup
                type="number"
                min={1}
                max={65535}
                defaultValue={8080}
                prefix="TCP"
              />
            </label>
          </RumahlFieldGroup>
          <RumahlFieldGroup label="Storage">
            <label>
              Home directory
              <RumahlInputGroup
                prefix="~/"
                defaultValue="Documents"
                spellCheck={false}
              />
            </label>
            <label>
              Quota
              <RumahlInputGroup
                type="number"
                min={1}
                defaultValue={20}
                suffix="GiB"
              />
            </label>
          </RumahlFieldGroup>
          <RumahlFieldGroup label="Schedule">
            <label>
              Start time
              <RumahlInput type="time" defaultValue="09:00" />
            </label>
            <label>
              Maintenance window
              <RumahlInput
                type="datetime-local"
                defaultValue="2026-10-09T02:00"
              />
            </label>
            <label>
              Retention
              <RumahlInputGroup
                type="number"
                min={1}
                defaultValue={30}
                suffix="days"
              />
            </label>
          </RumahlFieldGroup>
          <RumahlFieldGroup label="Contact">
            <label>
              Email
              <RumahlInput
                type="email"
                placeholder="alex@example.com"
                autoComplete="email"
              />
            </label>
            <label>
              Phone
              <RumahlInput type="tel" placeholder="+49 …" autoComplete="tel" />
            </label>
            <label>
              Server URL
              <RumahlInput type="url" placeholder="https://server.local" />
            </label>
          </RumahlFieldGroup>
          <RumahlSwitch
            label="Allow notifications"
            checked={toggle}
            onChange={(event) => setToggle(event.target.checked)}
            status={toggle ? "success" : "neutral"}
          />
          <RumahlRange
            label={`Display brightness · ${range}%`}
            min={0}
            max={100}
            value={range}
            onChange={(event) => setRange(Number(event.target.value))}
          />
          <div>
            <RumahlFileInput
              label="Import configuration"
              accept=".json,.toml"
              onChange={(event) =>
                setFiles(event.target.files?.[0]?.name ?? "")
              }
              status={files ? "success" : "neutral"}
              aria-describedby="design-file"
            />
            <small id="design-file" role="status">
              {files
                ? `${files} selected · local preview`
                : "Choose a JSON or TOML file"}
            </small>
          </div>
        </div>
      </fieldset>
      <fieldset>
        <legend>Glass validation · same material across controls</legend>
        <div className="design-lab__grid">
          <label>
            Invalid path
            <RumahlInputGroup
              prefix="~/"
              error="This folder does not exist."
              defaultValue="missing-folder"
            />
          </label>
          <label>
            Quota accepted
            <RumahlInputGroup
              type="number"
              suffix="GiB"
              status="success"
              defaultValue={20}
            />
          </label>
          <label>
            Invalid notes
            <RumahlTextArea
              error="Please add a description before saving."
              defaultValue=""
            />
          </label>
          <label>
            Notes accepted
            <RumahlTextArea
              status="success"
              defaultValue="Configuration ready"
            />
          </label>
          <RumahlSelect
            label="Unavailable device"
            error="Reconnect the device to continue."
            value="offline"
            onChange={() => {}}
            options={[{ value: "offline", label: "Device unavailable" }]}
          />
          <RumahlSelect
            label="Connected device"
            status="success"
            value="online"
            onChange={() => {}}
            options={[{ value: "online", label: "Device connected" }]}
          />
          <RumahlPassword
            label="Password error"
            error="This password is too short."
            defaultValue="example"
          />
          <RumahlPassword
            label="Password accepted"
            status="success"
            defaultValue="example"
          />
          <RumahlCheckbox label="Required consent missing" status="error" />
          <RumahlCheckbox
            label="Consent accepted"
            status="success"
            defaultChecked
          />
        </div>
      </fieldset>
      <fieldset>
        <legend>Selection</legend>
        <div className="design-lab__grid">
          <RumahlSelect
            label="Quality"
            value={choice}
            onChange={setChoice}
            options={[
              { value: "auto", label: "Automatic" },
              { value: "high", label: "High quality" },
              { value: "low", label: "Low power" },
            ]}
          />
          <RumahlSelect
            label="Disabled selection"
            value="auto"
            disabled
            onChange={() => {}}
            options={[{ value: "auto", label: "Disabled" }]}
          />
          <RumahlCheckbox
            label="Checkbox"
            checked={checked}
            onChange={(e) => setChecked(e.target.checked)}
          />
          <RumahlCheckbox label="Disabled checkbox" checked disabled />
          <RumahlRadio
            label="Option A"
            name="design-radio"
            checked={radio === "a"}
            onChange={() => setRadio("a")}
          />
          <RumahlRadio
            label="Option B"
            name="design-radio"
            checked={radio === "b"}
            onChange={() => setRadio("b")}
          />
          <label className="appearance-switch">
            Switch
            <input
              type="checkbox"
              role="switch"
              checked={toggle}
              onChange={(e) => setToggle(e.target.checked)}
            />
          </label>
          <RumahlButtonGroup
            label="Quality segments"
            value={segment}
            onChange={setSegment}
            options={["Auto", "High", "Low"].map((value) => ({
              value,
              label: value,
            }))}
          />
          <RumahlButtonGroup
            label="Text formatting"
            multiple
            value={formats}
            onChange={setFormats}
            options={["Bold", "Italic", "Underline"].map((value) => ({
              value,
              label: value,
            }))}
          />
          <RumahlButtonGroup
            label="Disabled group"
            disabled
            value="Auto"
            onChange={() => {}}
            options={[
              { value: "Auto", label: "Auto" },
              { value: "Manual", label: "Manual" },
            ]}
          />
          <label>
            Range · {range}
            <input
              type="range"
              min={0}
              max={100}
              value={range}
              onChange={(e) => setRange(Number(e.target.value))}
            />
          </label>
          <label>
            File
            <input type="file" />
          </label>
        </div>
      </fieldset>
      <fieldset>
        <legend>Calendar &amp; date</legend>
        <div className="design-lab__grid">
          <RumahlCalendar value={date} onChange={setDate} />
          <div>
            <RumahlDatePicker label="Date" value={date} onChange={setDate} />
            <p>{date}</p>
            <RumahlDatePicker
              label="Disabled date"
              value={date}
              onChange={setDate}
              disabled
            />
          </div>
        </div>
      </fieldset>
      <fieldset>
        <legend>Feedback</legend>
        <div className="design-lab__grid">
          <RumahlTooltip content="Tooltip · hover or keyboard focus">
            <Button>Tooltip</Button>
          </RumahlTooltip>
          <RumahlProgress label="Progress" value={range} />
          <RumahlProgress label="Loading" />
        </div>
      </fieldset>
      <fieldset>
        <legend>Color</legend>
        <RumahlColorPicker label="Accent" value={color} onChange={setColor} />
      </fieldset>
      <fieldset>
        <legend>OS controls · Demo</legend>
        <div className="design-lab__grid">
          <RumahlSearch
            label="Search settings"
            clearLabel="Clear search"
            value={search}
            onChange={setSearch}
          />
          <RumahlStepper
            label="Volume"
            decreaseLabel="Decrease volume"
            increaseLabel="Increase volume"
            value={volume}
            onChange={setVolume}
            step={5}
          />
          <div className="design-lab__buttons">
            <RumahlBadge tone="success">Connected</RumahlBadge>
            <RumahlBadge tone="warning">Update available</RumahlBadge>
            <RumahlBadge tone="danger">Offline</RumahlBadge>
            <RumahlBadge>Idle</RumahlBadge>
          </div>
        </div>
        <RumahlTabs
          label="System settings preview"
          value={tab}
          onChange={setTab}
          tabs={[
            {
              id: "general",
              label: "General",
              content: (
                <RumahlNotice title="System up to date">
                  No updates pending. This is a local preview.
                </RumahlNotice>
              ),
            },
            {
              id: "storage",
              label: "Storage",
              content: (
                <RumahlNotice
                  tone="warning"
                  title="Storage almost full"
                  action={
                    <Button onClick={() => setTab("general")}>Review</Button>
                  }
                >
                  Example warning with an embedded action.
                </RumahlNotice>
              ),
            },
            {
              id: "network",
              label: "Network",
              content: (
                <RumahlNotice tone="danger" title="Connection interrupted">
                  Check the network connection and try again.
                </RumahlNotice>
              ),
            },
            {
              id: "unavailable",
              label: "Unavailable",
              content: null,
              disabled: true,
            },
          ]}
        />
      </fieldset>
      <fieldset>
        <legend>OS windows &amp; dialogs</legend>
        <div className="design-lab__buttons">
          <Button
            onClick={() => {
              setFolderError("");
              setModal(true);
            }}
          >
            New folder…
          </Button>
          <Button onClick={() => setDialog(true)}>Remove folder…</Button>
          <RumahlButtonGroup
            mode="actions"
            label="File navigation"
            onChange={setLocation}
            options={[
              { value: "Back", label: "← Back" },
              { value: "Forward", label: "Forward →" },
              { value: "Home", label: "⌂ Root" },
            ]}
          />
        </div>
        <p role="status">{outcome || `Navigation preview: ${location}`}</p>
      </fieldset>
      <RumahlModal
        open={modal}
        onClose={() => setModal(false)}
        title="New folder"
        application="Files · Local preview"
        closeLabel="Close window"
        description="Choose a name for your folder. This is a local preview."
        actions={
          <>
            <Button onClick={() => setModal(false)}>Cancel</Button>
            <Button
              variant="primary"
              onClick={() => {
                if (
                  !folder.trim() ||
                  /[/\\]/.test(folder) ||
                  [".", ".."].includes(folder.trim())
                ) {
                  setFolderError(
                    "Use a folder name without slashes; it cannot be empty, . or ..",
                  );
                  return;
                }
                setOutcome(`Folder “${folder.trim()}” created in preview`);
                setModal(false);
              }}
            >
              Create folder
            </Button>
          </>
        }
      >
        <label>
          Folder name
          <RumahlInput
            data-autofocus
            value={folder}
            error={folderError}
            onChange={(event) => {
              setFolder(event.target.value);
              setFolderError("");
            }}
          />
        </label>
      </RumahlModal>
      <RumahlDialog
        open={dialog}
        onClose={() => setDialog(false)}
        onConfirm={() => {
          setOutcome("Folder removed in preview");
          setDialog(false);
        }}
        title="Remove this folder?"
        application="Files · Local preview"
        description="The folder and its contents will be removed. This demo does not change any files."
        confirmLabel="Remove folder"
        cancelLabel="Cancel"
        destructive
      />
      <fieldset>
        <legend>Buttons</legend>
        <div className="design-lab__buttons">
          {(["primary", "secondary", "ghost", "danger"] as const).map(
            (variant) => (
              <Button
                key={variant}
                variant={variant}
                onClick={() => setText(variant)}
              >
                {variant}
              </Button>
            ),
          )}
          <Button disabled>Disabled</Button>
          <Button size="sm">Small</Button>
        </div>
      </fieldset>
    </section>
  );
}
