# Compatible tracks implementation plan

Status: implemented and integrated on the feature branch. Final validation and remaining platform limitations are recorded in the pull request.

1. Add behavioral Rust tests for enabled unknown checks, key modes/aliases, inclusive BPM and half/double matching, energy/genre/tags, full-library eligibility and deterministic reasons/order. Observe RED against a minimal API stub.
2. Implement a focused library compatible module and a typed Tauri command using the existing key helpers and persisted library measurements. Query an atomic read transaction.
3. Add React behavioral tests for saved mode defaults, controls, late preview invalidation, errors/unknowns, audition and atomic ordinary-playlist saving. Observe RED before implementing.
4. Build the dense branded workspace using existing style classes; expose standalone typed compatible API without changing the shared Bridge.
5. Run focused Rust/React tests and typecheck, then project suites and relevant lint/build checks through the integration owner. Render a screenshot after the owner wires navigation; record evidence separately from native/music acceptance.

## Save race correction

Extract the existing prepared-track save transaction from pasted-tracklist saving into a shared `PreparedPlaylistSaveRequest` API. Check expected library revision, selected-track ready/nonarchived state and expected playlist revision under one SQLite immediate transaction, then create and populate an ordinary playlist atomically. Keep the tracklist row-count limit in its own API. Expose compatible saving through a dedicated Tauri command and preserve rank order. Add an optional persistent-player preview callback to prevent duplicate detail/waveform/player loading.

Observed failing regressions for stale measurements after preview, a selected track returning to Incoming, generic frontend saves bypassing native rejection and ignored preview delegation. Targeted native and React tests cover the corrected behavior.
