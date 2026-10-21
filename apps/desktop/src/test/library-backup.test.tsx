import {render,screen,waitFor} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {describe,it,expect,vi} from "vitest";
import LibraryBackup from "../views/LibraryBackup";
import type {LibraryBackupBridge,LibraryBackupPreview} from "../backup-types";
const preview:LibraryBackupPreview={path:"/generated/backup",database_sha256:"abc",created:1792540800000,tracks:12,missing_sources:0,schema:15,music_included:false};
function bridge():LibraryBackupBridge{return {chooseFolder:vi.fn().mockResolvedValue(preview.path),create:vi.fn().mockResolvedValue(preview),inspect:vi.fn().mockResolvedValue(preview),stage:vi.fn().mockResolvedValue({id:"fixture",created:preview.created,database_sha256:"abc"}),pending:vi.fn().mockResolvedValue(null),retry:vi.fn().mockResolvedValue({id:"fixture",created:0,database_sha256:"abc"}),cancel:vi.fn().mockResolvedValue(undefined)};}
describe("library backups",()=>{
 it("requires reviewed confirmation and explains restart and music exclusion",async()=>{
  const b=bridge(),user=userEvent.setup();render(<LibraryBackup bridge={b}/>);
  expect(screen.getByText(/music files are excluded/i)).toBeVisible();
  await user.click(screen.getByRole("button",{name:"Inspect backup folder"}));
  expect(await screen.findByText("12 tracks")).toBeVisible();expect(b.stage).not.toHaveBeenCalled();
  expect(screen.getByRole("button",{name:"Restore on next launch"})).toBeDisabled();
  await user.click(screen.getByRole("checkbox"));
  await user.click(screen.getByRole("button",{name:"Restore on next launch"}));
  await waitFor(()=>expect(b.stage).toHaveBeenCalledWith(preview.path,"abc"));
  expect(await screen.findByText(/close and reopen CueRithm/i)).toBeVisible();
 });
 it("retains actionable errors and does not offer restore after a failed inspection",async()=>{
  const b=bridge();b.inspect=vi.fn().mockRejectedValue(new Error("A source changed. Create a fresh backup."));
  render(<LibraryBackup bridge={b}/>);await userEvent.click(screen.getByRole("button",{name:"Inspect backup folder"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("A source changed");
  expect(screen.queryByRole("button",{name:"Restore on next launch"})).not.toBeInTheDocument();
 });
 it("shows and cancels a restore staged in an earlier session",async()=>{
  const b=bridge();b.pending=vi.fn().mockResolvedValue({id:"pending",created:0,database_sha256:"abc"});
  render(<LibraryBackup bridge={b}/>);await userEvent.click(await screen.findByRole("button",{name:"Cancel scheduled restore"}));
  await waitFor(()=>expect(b.cancel).toHaveBeenCalled());
  expect(screen.queryByRole("button",{name:"Cancel scheduled restore"})).not.toBeInTheDocument();
 });
 it("keeps a blocked restore visible with its reason and an explicit cancel",async()=>{
  const b=bridge();b.pending=vi.fn().mockResolvedValue({id:"blocked",created:0,database_sha256:"abc",blocked_reason:"A source file changed after review."});
  render(<LibraryBackup bridge={b}/>);
  expect(await screen.findByText("Restore blocked")).toBeVisible();
  expect(screen.getByRole("alert")).toHaveTextContent("A source file changed after review.");
  expect(screen.getByRole("button",{name:"Retry on next launch"})).toBeVisible();
  await userEvent.click(screen.getByRole("button",{name:"Cancel scheduled restore"}));
  await waitFor(()=>expect(b.cancel).toHaveBeenCalled());
 });
 it("only clears the blocked state after an explicit successful retry",async()=>{
  const b=bridge();b.pending=vi.fn().mockResolvedValue({id:"blocked",created:0,database_sha256:"abc",blocked_reason:"Restore files before retrying."});
  render(<LibraryBackup bridge={b}/>);
  await userEvent.click(await screen.findByRole("button",{name:"Retry on next launch"}));
  expect(await screen.findByText("Restore scheduled")).toBeVisible();
  expect(b.retry).toHaveBeenCalledTimes(1);
  expect(screen.queryByText("Restore blocked")).not.toBeInTheDocument();
 });
});
