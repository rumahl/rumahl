import { useEffect, useState } from "react";
import { useShell } from "../shell/ShellContext";
import { useI18n } from "../i18n";
interface Entry { id:string; parent:string; name:string; directory:boolean; size:number }
export function FilesApp(){
 const {live}=useShell(),{t}=useI18n();
 const [trail,setTrail]=useState<{id:string;name:string}[]>([{id:"root",name:""}]),[entries,setEntries]=useState<Entry[]>([]),[revision,setRevision]=useState(0),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[loading,setLoading]=useState(true);
 const [name,setName]=useState(""),[editing,setEditing]=useState<Entry|null>(null),[removing,setRemoving]=useState<Entry|null>(null),[moving,setMoving]=useState<Entry|null>(null);
 const [hydrated,setHydrated]=useState(false);
 useEffect(()=>setHydrated(true),[]);
 const available=hydrated&&!!live;
 const parent=trail.at(-1)!.id;
 function failure(status:number){return status===409?"conflict":status===413?"limit":status===404?"missing":"unavailable";}
 useEffect(()=>{const controller=new AbortController();setLoading(true);setError(null);setEntries([]);
   if(!live){setLoading(false);return;}
   const request = live.request;
   void request(`/api/v1/files?parent=${parent}`,{signal:controller.signal,credentials:"same-origin",cache:"no-store"}).then(async r=>{if(!r.ok)throw Error(failure(r.status));const value=await r.json() as Entry[];if(!Array.isArray(value)||value.length>1000)throw Error("unavailable");if(!controller.signal.aborted)setEntries(value);}).catch(e=>{if(!controller.signal.aborted)setError(e instanceof Error?e.message:"unavailable");}).finally(()=>{if(!controller.signal.aborted)setLoading(false);});
   return()=>controller.abort();
 },[live,parent,revision]);
 async function mutate(method:string,query:Record<string,string>,body?:File){
  if(!live||busy)return false;setBusy(true);setError(null);
  try{const request=live.request;const response=await request(`/api/v1/files?${new URLSearchParams(query)}`,{method,credentials:"same-origin",headers:{"Content-Type":"application/octet-stream"},...(body?{body}:{})});if(!response.ok)throw Error(failure(response.status));setRevision(v=>v+1);return true;}
  catch(e){setError(e instanceof Error?e.message:"unavailable");return false;}finally{setBusy(false);}
 }
 return <section className="files-app"><h1>{t("files.title")}</h1><p>{t("files.help")}</p>
 {hydrated&&!live?<p role="status">{t("files.demo")}</p>:null}
 <nav aria-label={t("files.path")}>{trail.map((part,index)=><button key={part.id} disabled={busy} onClick={()=>{setTrail(trail.slice(0,index+1));setEditing(null);setRemoving(null);}}>{index===0?t("files.root"):part.name}</button>)}</nav>
 <fieldset disabled={!available||busy||loading}><legend>{t("files.actions")}</legend>
 <label>{t("files.upload")} <input type="file" onChange={e=>{const file=e.target.files?.[0];e.target.value="";if(file){if(file.size>16*1024*1024)setError("limit");else void mutate("POST",{parent,name:file.name,directory:"false"},file);}}}/></label>
 <form onSubmit={e=>{e.preventDefault();void (editing?mutate("PUT",{id:editing.id,parent:editing.parent,name}):mutate("POST",{parent,name,directory:"true"})).then(ok=>{if(ok){setName("");setEditing(null);}});}}><label>{t(editing?"files.rename":"files.newFolder")} <input required maxLength={255} value={name} onChange={e=>setName(e.target.value)}/></label><button type="submit">{t("files.save")}</button>{editing?<button type="button" onClick={()=>{setEditing(null);setName("");}}>{t("workspace.cancel")}</button>:null}</form>
 {moving?<p>{t("files.moving")}: {moving.name} <button onClick={()=>void mutate("PUT",{id:moving.id,parent,name:moving.name}).then(ok=>{if(ok)setMoving(null);})}>{t("files.moveHere")}</button><button onClick={()=>setMoving(null)}>{t("workspace.cancel")}</button></p>:null}
 </fieldset>
 {busy?<p role="status">{t("files.working")}</p>:null}
 {error?<p role="alert">{t(`files.${["conflict","limit","missing"].includes(error)?error as "conflict"|"limit"|"missing":"unavailable"}`)}</p>:null}
 <button disabled={busy} onClick={()=>setRevision(v=>v+1)}>{t("files.refresh")}</button>
 {loading?<p role="status">{t("files.loading")}</p>:entries.length===0?<p>{t("files.empty")}</p>:<ul className="file-list">{entries.map(file=><li key={file.id}>
 {file.directory?<button onClick={()=>{setTrail([...trail,{id:file.id,name:file.name}]);setEditing(null);setRemoving(null);}}>{file.name}/</button>:<a href={`/api/v1/files/content?id=${file.id}`} download={file.name}>{file.name}</a>}
 <span>{file.directory?t("files.folder"):`${file.size.toLocaleString()} B`}</span>
 <button disabled={busy} onClick={()=>{setEditing(file);setName(file.name);}}>{t("files.rename")}</button><button disabled={busy} onClick={()=>setMoving(file)}>{t("files.move")}</button><button disabled={busy} onClick={()=>setRemoving(file)}>{t("files.delete")}</button>
 </li>)}</ul>}
 {removing?<div role="alertdialog" aria-label={t("files.confirmDelete")}><p>{t("files.confirmDelete")}: {removing.name}</p><button disabled={busy} onClick={()=>void mutate("DELETE",{id:removing.id}).then(ok=>{if(ok)setRemoving(null);})}>{t("files.delete")}</button><button onClick={()=>setRemoving(null)}>{t("workspace.cancel")}</button></div>:null}
 </section>;
}
