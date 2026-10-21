---
title: Library automation
---

# Library automation

The packaged desktop executable accepts one JSON command on standard input:

```sh
cuerithm-desktop --headless library.sqlite < command.json
```

On macOS use the executable inside `CueRithm.app/Contents/MacOS/`. On Linux,
use the installed desktop executable. This mode starts no window or network
server. Close the desktop before opening the same library through automation.
Both modes use the same library ownership, settings, preservation checks and
recovery journals.

The Windows package is a GUI executable. Use a process runner that explicitly
redirects stdin/stdout and waits for termination; an interactive launch alone
does not provide console output or reliable shell waiting. For example, in
PowerShell, with the executable and database in the current directory:

```powershell
$process = Start-Process -FilePath .\cuerithm-desktop.exe -ArgumentList '--headless', 'library.sqlite' -RedirectStandardInput command.json -RedirectStandardOutput result.json -RedirectStandardError error.log -Wait -PassThru
Get-Content -Encoding utf8 result.json
$process.ExitCode
```

Quote database arguments containing spaces according to the process runner's
argument rules. Packaged Windows receipt/exit-code acceptance remains a
separate platform check.

The request limit is 1 MiB. A response contains `status` and either `result` or
`error`. Exit codes are:

| Code | Meaning |
|---|---|
| 0 | The requested operation completed. |
| 1 | Invalid request, unavailable resources or an operation error. |
| 2 | Partial result, blocked rows, preparation attention or timeout. Inspect the result. |
| 3 | This library is open in another process. |

Automation uses the installed application's bundled decoders and analysis
components. It does not accept shell commands, scripts, plugins or replacement
decoder binaries. Network preparation uses the same application-wide pacing,
optional native credentials and consent settings as the desktop.

## Status and reports

```json
{"command":"status"}
```

Status includes library counts and up to 1,000 recent jobs. Outcome reports
resolve their complete bounded scope separately; they do not reuse that recent
job limit.

```json
{"command":"report","scope":{"kind":"selected","track_ids":["TRACK_ID"]}}
```

Other report scopes are `{"kind":"all_ready"}`, `{"kind":"failed"}` and
`{"kind":"review"}`. Failed reports contain tracks requiring preparation
attention; review reports contain those with saved match-review evidence.
Selected reports accept up to 10,000 unique track IDs and query only those
tracks. Complete library reports have a 100,000-track/job bound and return an
error when it is exceeded, rather than silently truncating.

Reports contain local identity, title, artist, preparation state and job
outcomes. They exclude credentials, source payloads, platform IDs and private
notes. A report can include successfully completed and attention rows together;
check `status` and the exit code.

## Export a local M3U8 playlist

```json
{"command":"playlist","track_ids":["FIRST_TRACK_ID","SECOND_TRACK_ID"]}
```

The returned `result.playlist` is an M3U8 string in the requested order. It uses
the current local paths of ready, non-archived tracks. Save that string as UTF-8;
it is not an audio export or a cross-machine path remapping operation.

## Preview, apply and recover file organization

```json
{
  "command":"organize_preview",
  "request":{
    "scope":{"kind":"tracks","track_ids":["TRACK_ID"]},
    "settings":{"mode":"rename_in_place","pattern":"{artist} - {title}"}
  }
}
```

Read the returned counts and use the immutable preview ID to page through rows:

```json
{"command":"organize_page","preview_id":"PREVIEW_ID","offset":0,"limit":100}
```

Apply requires both the preview ID and its exact revision:

```json
{"command":"organize_apply","preview_id":"PREVIEW_ID","revision":"REVISION"}
```

Stale previews, changed sources, collisions and unsupported reference repairs
remain blocked under the same rules as the UI. Existing destination files are
never overwritten. `organize_history` lists batches; recovery uses:

```json
{"command":"organize_recover","batch_id":"BATCH_ID"}
```

An organization operation owned by unfinished Incoming preparation must resume
through that preparation job rather than through a standalone batch recovery.

## Metadata drafts

The sequence is `metadata_draft` with `track_ids`, `metadata_update` with
`draft_id`, its numeric `revision` and a typed `MetadataEditorPatch`, then
`metadata_preview` with the updated draft ID/revision. `metadata_apply` takes
the returned `preview_id`; `metadata_recover` takes a `batch_id`.

These commands call the same metadata editor service as the inspector. Keep
untouched fields absent from the patch. Native artwork uses previously imported
opaque assets; arbitrary remote URLs and raw native-frame scripts are not
accepted. Incoming edits retain their preparation owner, accepted match and
final organization rules.

## Prepare and retry the library queue

```json
{"command":"prepare_queue","paths":["music/new-track.flac"],"timeout_seconds":300}
```

This starts the configured Incoming queue, including work already queued in
the library, and enqueues the supplied paths. It follows saved tagging, cleanup,
analysis and organization settings; it does not silently turn providers on.
An empty path list resumes the existing queue. File organization runs after
accepted tagging and cleanup, before the track becomes ready.

```json
{"command":"retry_preparation","job_ids":["JOB_ID"],"timeout_seconds":300}
```

Retry preserves the job's captured rules and accepted decisions through the
normal recovery service. The worker can also process other queued library work.
Timeouts from 1 to 86,400 seconds are supported. The result follows each logical
track through replacement attempts and reports accepted work, current jobs,
unresolved tracks and individual enqueue/retry failures. A later bad input does
not hide an earlier accepted item. Attention or timeout returns exit 2; it does
not claim that the entire batch succeeded.
