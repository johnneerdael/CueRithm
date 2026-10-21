# Manual musical correction implementation

Status: implemented and integrated on the feature branch. Final validation and remaining platform limitations are recorded in the pull request.

1. Add behavioral tests for guarded manual publication, validation, preserved measured analysis, restart Undo, ownership and A/B/A rejection.
2. Add schema 24 preview/history/template/ownership persistence and a focused library API; reuse core publication and existing sync schema.
3. Test named template previews, preservation, custom anchors and atomic batch apply.
4. Add typed Tauri boundary, player editor and reusable template controls with scoped styles; test explicit preview/save and failure retention.
5. Run focused Rust tests, React tests, TypeScript and formatting; inspect rendered UI and document limits.
