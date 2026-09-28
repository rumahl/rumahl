import { useState } from "react";
import { useWorkspace, type AppFolder } from "./Workspace";
import { useShell } from "../shell/ShellContext";
import { useShellPreferences } from "./ShellPreferences";
import { useI18n } from "../i18n";
import { useAppCatalog } from "../apps/AppCatalog";
import { firstPartyApps } from "../apps/registry";
export function WorkspaceSettings() {
 const {t}=useI18n(), workspace=useWorkspace(), settings=useShellPreferences(), {state}=useShell(), catalog=useAppCatalog();
 const [name,setName]=useState(""), [editing,setEditing]=useState<string|null>(null), [selected,setSelected]=useState<string[]>([]);
 const folders=workspace.effective.folders;
 const apps=[...firstPartyApps.map(a=>({id:a.id,title:t(a.title)})),{id:"settings",title:t("nav.settings")},...catalog.apps.filter(a=>!firstPartyApps.some(f=>f.id===a.id))];
 const updateFolders=async(next:AppFolder[])=>workspace.save(settings.scope,{...workspace.effective,folders:next});
 return <section className="workspace-settings"><h1>{t("workspace.title")}</h1><p>{t("workspace.help")}</p>
 <label>{t("preferences.scope")} <select value={settings.scope} onChange={e=>settings.setScope(e.target.value==="user"?"user":"device")}><option value="user">{t("preferences.user")}</option><option value="device">{t("preferences.device")}</option></select></label>
 <fieldset disabled={!workspace.ready||workspace.busy}><legend>{t("workspace.layout")}</legend>
 <button onClick={()=>void workspace.save(settings.scope,{...workspace.effective,windows:state.windows.filter(w=>w.location&&!w.streamId).slice(0,32).map(w=>({location:w.location!,rect:w.rect??{x:36,y:24,width:760,height:540},placement:w.placement??"floating",minimized:w.minimized}))})}>{t("workspace.save")}</button>
 <button onClick={workspace.requestRestore}>{t("workspace.restore")}</button>
 {settings.scope==="device"?<button onClick={()=>void workspace.save("device",null)}>{t("preferences.inherit")}</button>:null}
 </fieldset>
 <h2>{t("workspace.folders")}</h2>
 <ul>{folders.map(f=><li key={f.id}><span>{f.name}</span> <button onClick={()=>{setEditing(f.id);setName(f.name);setSelected(f.apps);}}>{t("workspace.edit")}</button> <button disabled={workspace.busy} onClick={()=>void updateFolders(folders.filter(v=>v.id!==f.id))}>{t("workspace.remove")}</button></li>)}</ul>
 <form onSubmit={e=>{e.preventDefault();void updateFolders([...folders.filter(f=>f.id!==editing).map(f=>({...f,apps:f.apps.filter(id=>!selected.includes(id))})),{id:editing??crypto.randomUUID(),name:name.trim(),apps:selected}]).then(ok=>{if(ok){setName("");setSelected([]);setEditing(null);}});}}>
 <fieldset disabled={!workspace.ready||workspace.busy}><legend>{t(editing?"workspace.edit":"workspace.create")}</legend>
 <label>{t("workspace.name")} <input required maxLength={64} value={name} onChange={e=>setName(e.target.value)}/></label>
 <div className="folder-members">{apps.map(a=><label key={a.id}><input type="checkbox" checked={selected.includes(a.id)} onChange={e=>setSelected(e.target.checked?[...selected,a.id]:selected.filter(id=>id!==a.id))}/>{a.title}</label>)}</div>
 <button type="submit">{t("workspace.saveFolder")}</button>{editing?<button type="button" onClick={()=>{setEditing(null);setName("");setSelected([]);}}>{t("workspace.cancel")}</button>:null}
 </fieldset></form><p role="status">{workspace.error?t(`preferences.${workspace.error==="conflict"?"conflict":"unavailable"}`):workspace.busy?t("preferences.saving"):workspace.ready?t("preferences.synced"):t("preferences.loading")}</p>
 </section>;
}
