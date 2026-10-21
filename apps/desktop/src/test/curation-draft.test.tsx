import {act,renderHook,waitFor} from "@testing-library/react";
import {describe,expect,it,vi} from "vitest";
import {useCurationDraft} from "../use-curation-draft";
import {fakeBridge} from "./bridge";
import type {CurationSnapshot,CurationReceipt} from "../curation-types";
const snapshot:CurationSnapshot={taxonomy_revision:1,library_revision:1,mixed:[],tracks:[{track_id:"a",title:"Borrowed Light",desired_revision:0,acknowledged_revision:0,desired:{categories:{},user_rating:null,notes:""},committed:{categories:{},user_rating:null,notes:""},state:"saved",error:null,active_publication_id:null,native_stale:false,can_retry:false,can_discard:false,measured:{key:"8A",bpm:128,energy:6,duration_seconds:240}}]};
describe("durable Curate revision queue",()=>{
 it("reconciles durable changes after discarding a draft whose receipt was lost",async()=>{const bridge=fakeBridge([]),saved=structuredClone(snapshot);saved.tracks[0].desired_revision=1;saved.tracks[0].acknowledged_revision=1;saved.tracks[0].desired.notes="Already durable";saved.tracks[0].committed.notes="Already durable";bridge.updateCuration=vi.fn(async()=>{throw Error("Receipt interrupted");});bridge.curationSnapshot=vi.fn(async()=>saved);const {result}=renderHook(()=>useCurationDraft(bridge,snapshot));act(()=>result.current.editNotes("Already durable"));await waitFor(()=>expect(result.current.error).toBe("Receipt interrupted"));await act(async()=>{await result.current.discardUnsaved();});await waitFor(()=>expect(result.current.tracks[0].desired_revision).toBe(1));expect(result.current.tracks[0].desired.notes).toBe("Already durable");expect(result.current.dirty).toBe(false);await expect(result.current.flush()).resolves.toBeUndefined();});
 it("accepts an unchanged native revision for an idempotent choice already present",async()=>{
  const bridge=fakeBridge([]),initial=structuredClone(snapshot);initial.tracks[0].desired.categories.mood=["warm"];
  bridge.curationSnapshot=vi.fn(async()=>initial);bridge.updateCuration=vi.fn(async request=>({request_id:request.request_id,accepted:[{track_id:"a",revision:0}],library_revision:1}));
  const {result}=renderHook(()=>useCurationDraft(bridge,initial));act(()=>result.current.edit({groups:[{kind:"add",group_id:"mood",value_ids:["warm"]}],notes:{kind:"keep"},user_rating:{kind:"keep"}}));
  await act(async()=>{await result.current.flush();});expect(result.current.error).toBeNull();expect(result.current.tracks[0].desired.categories.mood).toEqual(["warm"]);
 });
 it("flushes the latest notes and includes input arriving during an older receipt",async()=>{
  const bridge=fakeBridge([]);let first!:(receipt:CurationReceipt)=>void;
  bridge.updateCuration=vi.fn().mockImplementationOnce(()=>new Promise(resolve=>{first=resolve;})).mockImplementation(async request=>({request_id:request.request_id,accepted:[{track_id:"a",revision:2}],library_revision:3}));
  bridge.curationSnapshot=vi.fn(async()=>snapshot);
  const {result}=renderHook(()=>useCurationDraft(bridge,snapshot));
  act(()=>result.current.editNotes("Long blend"));const done=vi.fn();let flushing!:Promise<void>;
  act(()=>{flushing=result.current.flush().then(done);});await waitFor(()=>expect(bridge.updateCuration).toHaveBeenCalledTimes(1));
  act(()=>result.current.editNotes("Long blend. After the break."));
  await act(async()=>{first({request_id:vi.mocked(bridge.updateCuration).mock.calls[0][0].request_id,accepted:[{track_id:"a",revision:1}],library_revision:2});await flushing;});
  expect(done).toHaveBeenCalledTimes(1);expect(bridge.updateCuration).toHaveBeenLastCalledWith(expect.objectContaining({tracks:[{track_id:"a",revision:1}],patch:expect.objectContaining({notes:{kind:"set",value:"Long blend. After the break."}})}));
  expect(result.current.tracks[0].desired.notes).toBe("Long blend. After the break.");
 });
 it("retries an unacknowledged request with the same idempotency identity",async()=>{
  const bridge=fakeBridge([]);bridge.curationSnapshot=vi.fn(async()=>snapshot);
  bridge.updateCuration=vi.fn().mockRejectedValueOnce(Error("Receipt interrupted")).mockImplementation(async request=>({request_id:request.request_id,accepted:[{track_id:"a",revision:1}],library_revision:2}));
  const {result}=renderHook(()=>useCurationDraft(bridge,snapshot));act(()=>result.current.edit({groups:[],notes:{kind:"keep"},user_rating:{kind:"set",value:4}}));
  await waitFor(()=>expect(result.current.error).toBe("Receipt interrupted"));await expect(result.current.flush()).rejects.toThrow("Receipt interrupted");
  await act(async()=>{await result.current.retry();});
  expect(vi.mocked(bridge.updateCuration).mock.calls[1][0]).toEqual(vi.mocked(bridge.updateCuration).mock.calls[0][0]);
  expect(result.current.tracks[0].desired.user_rating).toBe(4);
 });
});
