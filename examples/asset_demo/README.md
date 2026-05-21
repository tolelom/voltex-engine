# asset_demo

`AssetLoader` background loading + `FileWatcher` hot reload.

## What it demonstrates

- `voltex_asset::AssetLoader::load` enqueueing a file on a worker thread.
- `Assets::process_loaded` draining completed loads on the main thread.
- `FileWatcher::poll_changes` detecting mtime updates.
- `AssetStorage::replace_in_place` preserving the handle across reload.

## Run

```bash
cargo run -p asset_demo
```

While the example is running, edit any of the watched files (printed at
startup) and the watcher reports the change.

## Next step

→ [`pbr_demo`](../pbr_demo) — switches from Blinn-Phong to a Cook-Torrance
PBR shader.
