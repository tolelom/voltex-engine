# editor_demo

Full immediate-mode editor — dock layout + 3D viewport + Hierarchy +
Inspector + Asset Browser + Play/Stop mode + Lua scripting.

## What it demonstrates

- `voltex_editor::DockTree` — binary-split docking layout.
- `voltex_editor::UiContext` + widgets (text/button/slider/checkbox/text_input
  /scroll_panel).
- 3D viewport via `ViewportTexture` + `ViewportRenderer` + `OrbitCamera`.
- Sphere picking in the viewport (left-click → `pick_entity`).
- Translate/Rotate/Scale gizmos (`voltex_editor::gizmo`).
- `EditorSession` + `EditorCommand` dispatch — every mutation is undoable.
- `panels::hierarchy` (tree view) + `panels::inspector` (Transform/Tag/components).
- Asset browser walking the filesystem.
- `EditorMode::Play` lifecycle — switches simulation on (physics + scripts).
- Lua script attachment via `ScriptComponent`.

## Run

```bash
cargo run -p editor_demo --release
```

## Key controls

| Action | Input |
|---|---|
| Orbit viewport | RMB drag |
| Pan viewport | MMB drag |
| Zoom | Scroll |
| Select entity | LMB on a sphere in the viewport |
| Resize dock | Drag handle |
| Undo / Redo | toolbar buttons |
| Spawn / Save / Load | toolbar buttons |
| Play / Stop | toolbar buttons (top-right) |

## Next step

→ [`runtime_demo`](../runtime_demo) — distills the runtime to a four-line
`main()` for game projects.
