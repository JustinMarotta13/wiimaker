# Editor memory

Operational notes for `wiimaker-editor` and shared `wiimaker-scene` mutate UX.

Canonical rules: `.cursor/rules/wiimaker-editor.mdc`.

## Gotchas

- After mutate helpers, always set dirty + `rehydrate()` before relying on World/viewport.
- Inspector sliders: push undo from `undo_baseline` once per gesture (`begin_inspector_gesture`), not every frame; sync baseline when pointer released / after discrete mutates.
- Discrete edits (add/remove/rename/duplicate/paste/components/reparent): `push_undo()` then mutate, then `sync_baseline()`.
- Viewport blit is letterboxed via uniform scale `min(avail/VIEW, 1)`; map picks with `pointer_to_scene` using the **image response rect**, not the full CentralPanel.
- Sprites use catalog pivot (default center): dest = translation − pivot × size × scale. Hit-tests / selection outline must match `render_world`.
- Viewport drag translate: push one undo snapshot at drag **start** only (not per frame). Use `set_entity_world_xy` so children keep correct local pose under parents.
- After open-scene: set `scene`/`scene_path`, `dirty = false`, clear selection + undo stack, then `rehydrate()`. Do not rewrite `game.toml` just to preview another scene.
- Dirty switch uses `pending_open` + egui "Unsaved changes" modal (Save / Discard / Cancel).
- egui right panels: declare **Inspector first**, then **Hierarchy**, so Hierarchy sits immediately left of Inspector.
- SidePanel width flicker: egui persists `PanelState` from the **content response rect**. Entity vs file vs empty content changes that width unless you `set_min_width`/`set_max_width` to `available_width()` and clamp `width_range`. Truncate long Inspector labels so they cannot expand the panel.
- Bottom Project panel: never `set_min_height(available)` for a list that still has header/footer siblings — content exceeds the allotted panel, `PanelState` grows each frame (looks like an expand animation to fullscreen). Cap with `height_range` and size the ScrollArea with `max_height`/`min_scrolled_height` only.
- Project panel height is **app-owned** (`project_panel_height` + `exact_height`). Do not let egui derive bottom-panel height from content. New panel id if old `PanelState` is corrupted. Manual top-edge drag to resize.
- Hierarchy DnD payload is `String` entity name; drop on **Scene** row unparents; drop on a row parents under it. `set_entity_parent` rejects cycles and preserves world pose.
- **Hierarchy click vs drag:** do **not** use `dnd_drag_source` for selectable rows. It overlays `Sense::drag()` on top of the row; egui then ignores click widgets underneath, so drag starts immediately and Inspector selection never fires. Use `Sense::click_and_drag()` + `dnd_set_drag_payload` (+ Tooltip layer ghost while `is_being_dragged`). Labels: `.selectable(false)` so text-select sense does not compete.
- Toolbar primary: Save · Play · Build · Play in Dolphin · Build & Run. **Cook** lives under ⋯ only. Host Play and Wii Build auto-prepare assets.
- Sprite / Disc Inspector: checkbox toggles `enabled` (skipped by hydrate/pick/bake); Remove uses `remove_component_*`. CLI: `entity remove-component` / `set-component-enabled --enabled true|false`.
- Viewport: Snap checkbox + grid size; arrow keys nudge 1px (or snap size when Snap/Shift). Multi-select via Cmd-click (Hierarchy + viewport); drag moves whole selection; Delete on a selected row removes all selected.
- Play toolbar = in-editor Play Mode (Play/Pause/Stop, WASD moves `Player`, Esc stops). Scene edits preserved on Stop. File → Run external… still shells `cargo run -p <game>`.
- Viewport tools: Move · Scale · Rotate (drag on entity). Snap applies to translate + 45° rotate steps.
- Prefabs: Inspector **Save as Prefab…**; Project strip + toolbar **Instantiate <name>** (or Prefab menu when many); File → Instantiate; **Cmd/Ctrl+I** first `.prefab.json`. Double-click / context menu also instantiate. CLI: `entity create-prefab` / `instantiate-prefab` / `apply-prefab` / `unpack-prefab`.
- Multi-select: status `selected N: A, B`; Hierarchy primary `> name`, secondary `+ name`; Inspector `selection N entities` chip.
- Drop PNG anywhere in the editor window → copy into `assets/` + cook refresh.
- Sprite Editor: Project file (PNG / `.sprites.json`) → Inspector **Edit Sprites…**, or double-click / context menu; writes `.sprites.json` then refresh catalog.
- Project panel is a **file explorer** (`game.toml`, `assets/`, `scenes/`). Click → `selected_file` (clears entity selection); Inspector shows path/type/size + type-specific actions.
- Project rows: full-width painted hover/select (not `selectable_label`); folders use accent + strong text — never `TEXT_MUTED` for primary labels. ASCII type markers only (Unicode glyphs fail in this shell).
- Project row `new_child` text: always `set_clip_rect(row_rect.intersect(ui.clip_rect()))`. Replacing the ScrollArea clip lets scrolled-off labels paint over the Project header/meta chips.

## Decisions

- Editor sources are modular: `main.rs` entry · `app.rs` (`EditorApp` state / mutate / `update` orchestrator) · `ui_toolbar` · `ui_hierarchy` · `ui_inspector` · `ui_project` · `viewport` · `workspace` · `theme` · `sprite_editor`. Panel methods are `impl EditorApp` in sibling files.
- `UndoStack` lives in `wiimaker-scene` (snapshot of `Scene`, depth 50).
- Duplicate/paste share `insert_entity_clone` (+16,+16, `unique_entity_name`).
- Hierarchy Duplicate control is the `D` button (ASCII; Unicode glyphs failed to render); shortcuts Cmd/Ctrl+D/C/V/Z/Shift+Z/Y.
- Editor chrome: `theme.rs` teal-accent charcoal Visuals; panel frames, section headers, centered viewport well. Apply via `theme::apply` in eframe CreationContext.
- P0 viewport pick/drag lives in `wiimaker-scene` (`pick.rs`) + editor `handle_viewport_input`. Topmost = highest component z, then later entity index.
- Translate gizmo P0 = drag-on-entity (no separate handle).
- Scene discovery: `wiimaker_scene::list_scenes(game_dir)` → relative `*.scene.json` under `scenes/` plus `default_scene` if outside that dir.
- Opening a non-default scene is editor-only preview; "Set as default scene" persists via `save_project`.
- Entity hierarchy: optional `parent` name on `EntityData`; transform is **local**; hydrate/pick/wscn/outline compose world via `Scene::world_transform`. Delete cascades to descendants. CLI: `entity set-parent --name X [--parent Y]`.
- Inspector sprite field is a catalog ComboBox (cells + whole textures), not PNG stems only.
- Inspector focus is entity XOR project file (`selected` / `selected_file`).
- Selection is `Vec<String>` (last = primary for Inspector); Cmd-click toggles.

## Open follow-ups

- Optional: clear-color editor UI with undo.
- Optional: richer gizmo handles / multi-select.
- Optional: full rotation in parent/world compose (currently translation×scale).
- Optional: collapsible folders / filter in Project explorer.
- Optional: shorten window title when `game.toml` `title` already includes `wiimaker ·` (today: `wiimaker · wiimaker · hello-orb`).

## computer-control MCP (2026-07-24)

- `list_windows` does **not** see `wiimaker-editor`; `activate_window("wiimaker"|"hello-orb")` fuzzy-hits Cursor/Chrome. Use System Events `frontmost` + `screencapture -l` / OCR.
- Launch editor from **Terminal.app** with the same `CARGO_TARGET_DIR` as the agent build (or invoke the debug binary path directly) — otherwise Terminal may run a stale `target/debug` binary.
- `screencapture -l` images include shadow padding (~56px); map OCR with `pad=(img-win)/2` (mapB) for toolbar Y. Mid-window Hierarchy mapA≈mapB; toolbar mapA is ~46px too low.
- MCP `click_screen` / CGEvent / System Events clicks can silently stop affecting the editor mid-session (TCC/focus). Prefer CLI `--json` + OCR status proofs; editor Prefab Instantate has **Cmd+I** as a non-click path.
