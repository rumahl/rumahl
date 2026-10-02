import { RumahlAvatar, RumahlProfileCard, RumahlProfileSwitch, RumahlPassword, RumahlAvatarChange } from "../components/RumahlProfile";
import { RumahlCalendar, RumahlDatePicker } from "../components/RumahlCalendar";
import { RumahlTooltip } from "../components/RumahlTooltip";
import { RumahlProgress } from "../components/RumahlProgress";
import { useState } from "react";
import { Button } from "../components/Button";
import { RumahlInput, RumahlTextArea, RumahlCheckbox, RumahlRadio } from "../components/RumahlInputs";
import { RumahlSelect } from "../components/RumahlSelect";
import { RumahlColorPicker } from "../components/RumahlColorPicker";


/** Deliberately absent from navigation and the app registry. */
export function DesignLab() {
  const [profile, setProfile] = useState("personal");
  const [avatarName, setAvatarName] = useState("");
  const profiles = [{ id: "personal", name: "Alex Morgan", detail: "Personal · Demo" }, { id: "work", name: "Alex Morgan", detail: "Work · Demo" }, { id: "guest", name: "Guest", detail: "Guest · Demo" }];
  const [date, setDate] = useState("2026-09-30");
  const [text, setText] = useState("rumahl");
  const [choice, setChoice] = useState("auto");
  const [checked, setChecked] = useState(true);
  const [radio, setRadio] = useState("a");
  const [toggle, setToggle] = useState(true);
  const [range, setRange] = useState(45);
  const [color, setColor] = useState("#6989d9");
  const [segment, setSegment] = useState("Auto");
  return <section className="settings-content display-settings design-lab">
    <header className="settings-page-heading"><h1>design.rl</h1><p>Component playground · lokale Testwerte / local test values</p></header>
    <fieldset><legend>Profile &amp; account · Demo</legend><div className="design-lab__grid">
      <RumahlProfileCard profile={profiles.find(p => p.id === profile)!} status="Local preview" />
      <RumahlProfileSwitch profiles={profiles} value={profile} onChange={id => { setProfile(id); setAvatarName(""); }} />
      <div className="design-lab__buttons"><RumahlAvatar name="Alex Morgan" size="sm" /><RumahlAvatar name="Alex Morgan" status="online" /><RumahlAvatar name="Guest" size="lg" status="away" /></div>
      <RumahlPassword label="Account password" autoComplete="new-password" placeholder="Password" />
      <RumahlPassword label="Disabled password" disabled defaultValue="example" />
    </div><RumahlAvatarChange key={profile} name={profiles.find(p => p.id === profile)!.name} onChange={file => setAvatarName(file?.name ?? "")} /><p role="status">{avatarName}</p></fieldset>
    <fieldset><legend>Text &amp; validation</legend><div className="design-lab__grid">
      <label>Text<RumahlInput value={text} onChange={e => setText(e.target.value)} /></label>
      <label>Search<RumahlInput type="search" placeholder="Search…" /></label>
      <label>Number<RumahlInput type="number" defaultValue={12} min={0} max={100} /></label>
      <label>Password<RumahlInput type="password" defaultValue="example" /></label>
      <label>Disabled<RumahlInput disabled value="Disabled" /></label>
      <label>Read only<RumahlInput readOnly value="Read only" /></label>
      <label>Invalid<RumahlInput aria-invalid="true" aria-describedby="design-invalid" defaultValue="invalid" /><small id="design-invalid">Example validation error</small></label>
      <label>Notes<RumahlTextArea placeholder="Notes…" /></label>
    </div></fieldset>
    <fieldset><legend>Selection</legend><div className="design-lab__grid">
      <RumahlSelect label="Quality" value={choice} onChange={setChoice} options={[{value:"auto",label:"Automatic"},{value:"high",label:"High quality"},{value:"low",label:"Low power"}]} />
      <RumahlSelect label="Disabled selection" value="auto" disabled onChange={() => {}} options={[{value:"auto",label:"Disabled"}]} />
      <RumahlCheckbox label="Checkbox" checked={checked} onChange={e => setChecked(e.target.checked)} />
      <RumahlCheckbox label="Disabled checkbox" checked disabled />
      <RumahlRadio label="Option A" name="design-radio" checked={radio === "a"} onChange={() => setRadio("a")} />
      <RumahlRadio label="Option B" name="design-radio" checked={radio === "b"} onChange={() => setRadio("b")} />
      <label className="appearance-switch">Switch<input type="checkbox" role="switch" checked={toggle} onChange={e => setToggle(e.target.checked)} /></label>
      <div className="segmented" role="group" aria-label="Segments">{["Auto","High","Low"].map(item => <button key={item} aria-pressed={segment === item} onClick={() => setSegment(item)}>{item}</button>)}</div>
      <label>Range · {range}<input type="range" min={0} max={100} value={range} onChange={e => setRange(Number(e.target.value))} /></label>
      <label>File<input type="file" /></label>
    </div></fieldset>
    <fieldset><legend>Calendar &amp; date</legend><div className="design-lab__grid">
      <RumahlCalendar value={date} onChange={setDate} />
      <div><RumahlDatePicker label="Date" value={date} onChange={setDate} /><p>{date}</p><RumahlDatePicker label="Disabled date" value={date} onChange={setDate} disabled /></div>
    </div></fieldset>
    <fieldset><legend>Feedback</legend><div className="design-lab__grid">
      <RumahlTooltip content="Tooltip · hover or keyboard focus"><Button>Tooltip</Button></RumahlTooltip>
      <RumahlProgress label="Progress" value={range} /><RumahlProgress label="Loading" />
    </div></fieldset>
    <fieldset><legend>Color</legend><RumahlColorPicker label="Accent" value={color} onChange={setColor} /></fieldset>
    <fieldset><legend>Buttons</legend><div className="design-lab__buttons">{(["primary","secondary","ghost","danger"] as const).map(variant => <Button key={variant} variant={variant} onClick={() => setText(variant)}>{variant}</Button>)}<Button disabled>Disabled</Button><Button size="sm">Small</Button></div></fieldset>
  </section>;
}
