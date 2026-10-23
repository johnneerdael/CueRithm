import {afterEach,beforeEach,describe,expect,it,vi} from "vitest";
import {readSyncPreferences,syncPreferencesKey} from "../sync-preferences";

beforeEach(()=>{localStorage.removeItem(syncPreferencesKey);});
afterEach(()=>{vi.restoreAllMocks();});

describe("sync choice storage",()=>{
 it("retains valid choices while dropping unknown fields and malformed option values",()=>{
  localStorage.setItem(syncPreferencesKey,JSON.stringify({version:1,software:"engine_dj",previewId:"never-restore",profiles:{engine_dj:{path:"/Engine",scope:"changed",playlistId:"set",stagedPayload:{apply:true},options:{engine_copy_audio:true,materialize_smartlists:true,nearest_color:false,skip_beatgrids:"true",beatgrid_lock:"lock",key_notation:"camelot",usb_cue_destination:"both",usb_cdj_compatibility:"detect",field_mappings:{comment:"genre",key:42,unknown:"artist"},unknown:true}},djay_pro:{path:"bad\0path",scope:false,playlistId:[]},unknown:{path:"/Other"}}}));
  const saved=readSyncPreferences();
  expect(saved.software).toBe("engine_dj");
  expect(saved.profiles.engine_dj).toMatchObject({path:"/Engine",scope:"changed",playlistId:"set",options:{engine_copy_audio:true,materialize_smartlists:true,nearest_color:false,skip_beatgrids:false,beatgrid_lock:"lock",key_notation:"camelot",usb_cue_destination:"both",usb_cdj_compatibility:"detect",field_mappings:{comment:"genre"}}});
  expect(Object.keys(saved.profiles)).toEqual(["virtual_dj","engine_dj","djay_pro","serato4","pioneer_usb","mixxx"]);
  expect(saved.destination).toBe("engine_dj");
  expect(Object.keys(saved).sort()).toEqual(["destination","profiles","software","version"]);
  expect(Object.keys(saved.profiles.engine_dj).sort()).toEqual(["environment","options","path","playlistId","scope","sourceRoot"]);
  expect(saved.profiles.engine_dj.options).not.toHaveProperty("unknown");
  expect(saved.profiles.engine_dj.options.field_mappings).toEqual({comment:"genre"});
  expect(saved.profiles.djay_pro).toMatchObject({path:"",scope:"full",playlistId:""});
 });
 it("creates independent default option mappings for every target and read",()=>{
  const first=readSyncPreferences();
  first.profiles.virtual_dj.options.field_mappings.comment="artist";
  expect(first.profiles.engine_dj.options.field_mappings).toEqual({});
  expect(readSyncPreferences().profiles.virtual_dj.options.field_mappings).toEqual({});
 });
 it("defaults Mixxx to Add only with playlists, independently on every read",()=>{
  const first=readSyncPreferences();
  expect(first.profiles.mixxx.options.mixxx).toEqual({library:"update",existing_tracks:"keep",container:"playlists",replace_metadata:true,replace_key:true,replace_cues:true});
  first.profiles.mixxx.options.mixxx.existing_tracks="replace";
  expect(readSyncPreferences().profiles.mixxx.options.mixxx.existing_tracks).toBe("keep");
  expect(first.profiles.virtual_dj.options.mixxx.existing_tracks).toBe("keep");
 });
 it("keeps valid saved Mixxx choices, repairs malformed ones, and never restores a Wine environment for Mixxx",()=>{
  localStorage.setItem(syncPreferencesKey,JSON.stringify({version:1,software:"mixxx",profiles:{mixxx:{path:"/Mixxx",environment:{kind:"windows_compatibility",prefix:"/p",installation:null,mappings:[]},options:{mixxx:{library:"overwrite",existing_tracks:"replace",container:"crates",replace_metadata:false,replace_key:"yes",replace_cues:false,unknown:1}}}}}));
  const saved=readSyncPreferences();
  expect(saved.software).toBe("mixxx");
  expect(saved.profiles.mixxx.path).toBe("/Mixxx");
  expect(saved.profiles.mixxx.environment).toEqual({kind:"native"});
  expect(saved.profiles.mixxx.options.mixxx).toEqual({library:"overwrite",existing_tracks:"replace",container:"crates",replace_metadata:false,replace_key:true,replace_cues:false});
  localStorage.setItem(syncPreferencesKey,JSON.stringify({version:1,software:"mixxx",profiles:{mixxx:{options:{mixxx:{library:"everything",existing_tracks:"everything",container:7}}}}}));
  expect(readSyncPreferences().profiles.mixxx.options.mixxx).toEqual({library:"update",existing_tracks:"keep",container:"playlists",replace_metadata:true,replace_key:true,replace_cues:true});
  localStorage.setItem(syncPreferencesKey,JSON.stringify({version:1,software:"mixxx",profiles:{mixxx:{options:{mixxx:"not an object"}}}}));
  expect(readSyncPreferences().profiles.mixxx.options.mixxx.existing_tracks).toBe("keep");
 });
 it("falls back when browser storage is unavailable",()=>{
  vi.spyOn(localStorage,"getItem").mockImplementation(()=>{throw new Error("Storage unavailable");});
  expect(readSyncPreferences()).toMatchObject({software:"virtual_dj",profiles:{virtual_dj:{path:"",scope:"full",playlistId:"",options:{field_mappings:{}}}}});
 });
});
