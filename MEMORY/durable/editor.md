# Editor memory

Operational notes for `wiimaker-editor` and shared `wiimaker-scene` mutate UX.

Canonical rules: `.cursor/rules/wiimaker-editor.mdc`.

## Gotchas

- After mutate helpers, always set dirty + `rehydrate()` before relying on World/viewport.
- Inspector sliders: push undo from `undo_baseline` once per gesture (`begin_inspector_gesture`), not every frame; sync baseline when pointer released / after discrete mutates.
- Discrete edits (add/remove/rename/duplicate/paste/components): `push_undo()` then mutate, then `sync_baseline()`.
- Viewport blit is letterboxed via uniform scale `min(avail/VIEW, 1)`; map picks with `pointer_to_scene` using the **image response rect**, not the full CentralPanel.
- Sprites are **center-origin** in `render_world` (dest = translation − half size×scale). Disc radius uses `radius * max(sx, sy)`. Hit-tests in `wiimaker_scene::pick` must match that.
- Viewport drag translate: push one undo snapshot at drag **start** only (not per frame).
- After open-scene: set `scene`/`scene_path`, `dirty = false`, clear selection + undo stack, then `rehydrate()`. Do not rewrite `game.toml` just to preview another scene.
- Dirty switch uses `pending_open` + egui "Unsaved changes" modal (Save / Discard / Cancel).

## Decisions

- `UndoStack` lives in `wiimaker-scene` (snapshot of `Scene`, depth 50).
- Duplicate/paste share `insert_entity_clone` (+16,+16, `unique_entity_name`).
- Hierarchy Duplicate control is the `⧉` button; shortcuts Cmd/Ctrl+D/C/V/Z/Shift+Z/Y.
- P0 viewport pick/drag lives in `wiimaker-scene` (`pick.rs`: `pointer_to_scene`, `pick_entity_at`) + editor `handle_viewport_input`. Topmost = highest component z, then later entity index.
- Translate gizmo P0 = drag-on-entity (no separate handle).
- Scene discovery: `wiimaker_scene::list_scenes(game_dir)` → relative `*.scene.json` under `scenes/` plus `default_scene` if outside that dir.
- Opening a non-default scene is editor-only preview; "Set as default scene" persists via `save_project`.

## Open follow-ups

- Optional: clear-color editor UI with undo.
- Optional: richer gizmo handles / multi-select.
