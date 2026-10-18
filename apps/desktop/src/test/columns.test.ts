import {it,expect} from "vitest";
import {columnLabel,metadataText} from "../columns";
it("labels custom metadata as text, including names shared with object properties",()=>{
  expect(columnLabel("tag.label")).toBe("Label");
  expect(columnLabel("tag.constructor")).toBe("Constructor");
  expect(columnLabel("tag.custom:mood")).toBe("Custom:mood");
  expect(metadataText("format.bit_rate","320000")).toBe("320 kbps");
  expect(metadataText("tag.rating",null)).toBe("—");
});
