import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { App } from "../App";
import { demoSnapshot } from "../demo/snapshot";
import { parseWorkspace } from "./Workspace";
describe("workspace persistence",()=>{
 it("rejects external routes, invalid geometry and duplicate folder memberships",()=>{
  const w={version:1,windows:[{location:"/app/files",rect:{x:20,y:30,width:600,height:400},placement:"left",minimized:false}],folders:[]};
  expect(parseWorkspace(w).windows[0]?.placement).toBe("left");
  expect(()=>parseWorkspace({...w,windows:[{...w.windows[0],location:"//evil.test"}]})).toThrow();
  expect(()=>parseWorkspace({...w,windows:[{...w.windows[0],rect:{x:NaN,y:0,width:1,height:1}}]})).toThrow();
  expect(()=>parseWorkspace({...w,folders:[{id:"one",name:"One",apps:["files"]},{id:"two",name:"Two",apps:["files"]}]})).toThrow();
 });
 it("restores a saved window while preserving an explicit deep link",async()=>{
  localStorage.setItem("rumahl.demo.workspace",JSON.stringify({version:1,windows:[{location:"/app/files",rect:{x:20,y:30,width:600,height:400},placement:"left",minimized:false}],folders:[]}));
  render(<App snapshot={demoSnapshot} initialLocation="/app/app-manager"/>);
  expect(await screen.findByRole("region",{name:"Files"})).toBeInTheDocument();
  expect(screen.getByRole("region",{name:"App manager"})).toBeInTheDocument();
 });
 it("creates a folder without importing app code and retains it across a reload",async()=>{
  const rendered=render(<App snapshot={demoSnapshot} initialLocation="/settings/workspace"/>);
  fireEvent.change(screen.getByLabelText("Folder name"),{target:{value:"Tools"}});
  fireEvent.click(screen.getByRole("checkbox",{name:"Files"}));
  fireEvent.click(screen.getByRole("button",{name:"Save folder"}));
  await waitFor(()=>expect(JSON.parse(localStorage.getItem("rumahl.demo.workspace")!).folders[0].name).toBe("Tools"));
  rendered.unmount();render(<App snapshot={demoSnapshot}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Tools"}));
  expect(screen.getByRole("link",{name:"Files"})).toBeInTheDocument();
 });
});
