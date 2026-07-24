# Editor memory

Operational notes for `wiimaker-editor` and shared `wiimaker-scene` mutate UX.

Canonical rules: `.cursor/rules/wiimaker-editor.mdc`.

## Gotchas

- After open-scene: set `scene`/`scene_path`, `dirty = false`, `selected = None`, then `rehydrate()`. Do not rewrite `game.toml` just to preview another scene.
- Dirty switch uses `pending_open` + egui "Unsaved changes" modal (Save / Discard / Cancel).

## Decisions

- Scene discovery: `wiimaker_scene::list_scenes(game_dir)` → relative `*.scene.json` under `scenes/` plus `default_scene` if outside that dir.
- Opening a non-default scene is editor-only preview; "Set as default scene" persists via `save_project`.

## Open follow-ups

<!-- Next concrete editor tasks. -->
