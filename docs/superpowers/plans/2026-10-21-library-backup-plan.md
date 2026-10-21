# Local library backup implementation plan

Status: implemented and integrated on the feature branch. Final validation and remaining platform limitations are recorded in the pull request.

1. Add authored fixture tests for coherent backup/settings, open-lease refusal,
   changed sources/resources, untrusted bundles and restart-only restore. Observe
   RED before implementation.
2. Implement fixed-layout directory manifest, strict budgets and SQLite checks,
   native SQLite backup, publication preflight and dependency inventory.
3. Stage validated restore requests. Apply under original lease before Library
   opens, preserving a recoverable pre-restore bundle and retry evidence.
4. Add typed Tauri commands and scoped React backup view. Test preview, explicit
   staging, restart copy and error handling. Integrator registers startup/commands
   and renders the view.
5. Run targeted Rust/React checks. Integrator runs final workspace gates and
   rendered UI verification. Preserve automated release versions.
