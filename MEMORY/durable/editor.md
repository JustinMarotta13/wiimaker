# Editor memory

Operational notes for `wiimaker-editor` and shared `wiimaker-scene` mutate UX.

Canonical rules: `.cursor/rules/wiimaker-editor.mdc`.

## Gotchas

- Viewport blit is letterboxed via uniform scale `min(avail/VIEW, 1)`; map picks with `pointer_to_scene` using the **image response rect**, not the full CentralPanel.
- Sprites are **center-origin** in `render_world` (dest = translation − half size×scale). Disc radius uses `radius * max(sx, sy)`. Hit-tests in `wiimaker_scene::pick` must match that.
- After mutate helpers, always set dirty + rehydrate() before relying on World/viewport.

## Decisions

- P0 viewport pick/drag lives in `wiimaker-scene` (`pick.rs`: `pointer_to_scene`, `pick_entity_at`) + editor `handle_viewport_input`. Topmost = highest component z, then later entity index.
- Translate gizmo P0 = drag-on-entity (no separate handle). Undo: `TODO(undo): push snapshot at drag start` in editor until UndoStack lands.

## Open follow-ups

- Wire undo snapshot at viewport drag start once p0-1 UndoStack merges.
- Optional: richer gizmo handles / multi-select.