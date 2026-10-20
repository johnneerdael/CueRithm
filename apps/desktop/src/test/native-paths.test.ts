import {describe,expect,it} from "vitest";
import {pathKey,pathText} from "../native-paths";

describe("native path labels",()=>{
 it("keeps text paths and valid UTF-8 runs while escaping undecodable bytes",()=>{
  expect(pathText("/music/café.wav")).toBe("/music/café.wav");
  expect(pathText({unix:[47,99,97,102,0xc3,0xa9,45,0xff,46,119,97,118]})).toBe("/café-\\xff.wav");
  expect(pathText({unix:[0xc0,0xaf]})).toBe("\\xc0\\xaf");
  expect(pathText({unix:[47,0xef,0xbb,0xbf,0xff]})).toBe("/\ufeff\\xff");
 });
 it("preserves UTF-16 pairs and visibly escapes unpaired Windows units",()=>{
  expect(pathText({windows:[0x43,0x3a,0x5c,0xd83c,0xdfb5,0x2d,0xd800,0x2e,0x77,0x61,0x76]})).toBe("C:\\🎵-\\ud800.wav");
 });
 it("distinguishes raw paths from literal escape characters for list identity",()=>{
  expect(pathKey({unix:[0xff]})).not.toBe(pathKey("\\xff"));
  expect(pathKey({unix:[0xff]})).not.toBe(pathKey({unix:[0xfe]}));
 });
});
