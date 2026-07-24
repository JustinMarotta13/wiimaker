# Editor memory

Operational notes for `wiimaker-editor` and shared `wiimaker-scene` mutate UX.

Canonical rules: `.cursor/rules/wiimaker-editor.mdc`.

## Gotchas

- After mutate helpers, always set dirty + `rehydrate()` before relying on World/viewport.
- Inspector sliders: push undo from `undo_baseline` once per gesture (`begin_inspector_gesture`), not every frame; sync baseline when pointer released / after discrete mutates.
- Discrete edits (add/remove/rename/duplicate/paste/components): `push_undo()` then mutate, then `sync_baseline()`.

## Decisions

- `UndoStack` lives in `wiimaker-scene` (snapshot of `Scene`, depth 50).
- Duplicate/paste share `insert_entity_clone` (+16,+16, `unique_entity_name`).
- Hierarchy Duplicate control is the `⧉` button; shortcuts Cmd/Ctrl+D/C/V/Z/Shift+Z/Y.

## Open follow-ups

- Viewport picking / gizmos (other agents).
- Optional: clear-color editor UI with undo.
