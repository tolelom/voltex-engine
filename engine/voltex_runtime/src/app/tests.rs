//! Unit tests for [`RuntimeApp`] that run without a window or GPU.

use super::*;
use std::cell::Cell;
use std::rc::Rc;
use voltex_ecs::Transform;
use voltex_math::Vec3;

#[test]
fn new_runs_register_components_and_scene_builder_in_order() {
    let order = Rc::new(Cell::new(Vec::<&'static str>::new()));
    let o1 = order.clone();
    let o2 = order.clone();
    let cfg = RuntimeConfig::new()
        .with_registry_setup(move |_reg| {
            let mut v = o1.take();
            v.push("registry");
            o1.set(v);
        })
        .with_scene_builder(move |world, _reg| {
            let mut v = o2.take();
            v.push("scene");
            o2.set(v);
            let e = world.spawn();
            world.add(e, Transform::from_position(Vec3::new(7.0, 0.0, 0.0)));
        });

    let app = RuntimeApp::new(cfg).expect("RuntimeApp::new");
    let order = order.take();
    assert_eq!(order, vec!["registry", "scene"]);
    assert_eq!(app.world.entity_count(), 1);
}

#[test]
fn with_audio_files_decodes_and_watches_with_stable_indices() {
    let dir = std::env::temp_dir();
    let good = dir.join(format!("voltex_waf_good_{}.wav", std::process::id()));
    let bad = dir.join(format!("voltex_waf_bad_{}.wav", std::process::id()));
    std::fs::write(&good, voltex_audio::generate_wav_bytes(&[0.1, -0.1], 44100)).unwrap();
    std::fs::write(&bad, b"RIFFcorrupt").unwrap();

    // One pre-loaded clip (index 0), then files: bad → placeholder at 1,
    // good keeps its position at 2 (index stability is the contract).
    let cfg = RuntimeConfig::new()
        .with_audio(vec![voltex_audio::AudioClip::new(vec![0.0; 4], 44100, 1)])
        .with_audio_files(vec![bad.clone(), good.clone()]);
    let app = RuntimeApp::new(cfg).expect("decode failure must not abort boot");

    assert!(app.audio.is_some(), "audio system built");
    assert_eq!(app.watched_audio_clips.len(), 2, "both files watched");
    assert_eq!(app.watched_audio_clips.get(&bad), Some(&1));
    assert_eq!(app.watched_audio_clips.get(&good), Some(&2));

    // with_audio_files mapping is seeded into the AudioPath resolver —
    // a scene AudioPath for the same file reuses the clip (friction #5).
    assert_eq!(app.audio_clip_cache.get(&good), Some(2));
    assert_eq!(app.audio_clip_cache.get(&bad), Some(1), "placeholder id seeded too");

    let _ = std::fs::remove_file(&good);
    let _ = std::fs::remove_file(&bad);
}

#[test]
fn with_audio_files_alone_starts_indices_at_zero() {
    let path = std::env::temp_dir()
        .join(format!("voltex_waf_solo_{}.wav", std::process::id()));
    std::fs::write(&path, voltex_audio::generate_wav_bytes(&[0.2], 22050)).unwrap();

    let cfg = RuntimeConfig::new().with_audio_files(vec![path.clone()]);
    let app = RuntimeApp::new(cfg).expect("RuntimeApp::new");
    assert_eq!(app.watched_audio_clips.get(&path), Some(&0));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn tick_systems_only_runs_user_systems_in_order() {
    let log = Rc::new(Cell::new(Vec::<u32>::new()));
    let l1 = log.clone();
    let l2 = log.clone();
    let cfg = RuntimeConfig::new()
        .disable_physics()
        .disable_scripts()
        .with_system_setup(move |app| {
            let la = l1.clone();
            app.add_system(move |_w, _dt| {
                let mut v = la.take();
                v.push(1);
                la.set(v);
            });
            let lb = l2.clone();
            app.add_system(move |_w, _dt| {
                let mut v = lb.take();
                v.push(2);
                lb.set(v);
            });
        });

    let mut app = RuntimeApp::new(cfg).expect("RuntimeApp::new");
    assert_eq!(app.user_system_count(), 2);
    app.tick_systems_only(0.016);
    app.tick_systems_only(0.016);
    let recorded = log.take();
    assert_eq!(recorded, vec![1, 2, 1, 2]);
}

#[test]
fn tick_systems_only_propagates_transforms() {
    use voltex_ecs::{add_child, WorldTransform};
    let cfg = RuntimeConfig::new().disable_physics().disable_scripts();
    let mut app = RuntimeApp::new(cfg).unwrap();

    let parent = app.world.spawn();
    app.world.add(parent, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));
    let child = app.world.spawn();
    app.world.add(child, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));
    add_child(&mut app.world, parent, child);

    app.tick_systems_only(0.016);

    let wt = app.world.get::<WorldTransform>(child).expect("child WorldTransform");
    let translation = wt.0.cols[3];
    assert!((translation[0] - 11.0).abs() < 1e-5);
    assert!((translation[1] - 2.0).abs() < 1e-5);
    assert!((translation[2] - 3.0).abs() < 1e-5);
}

#[test]
fn initial_scene_missing_path_is_silent() {
    let cfg = RuntimeConfig::new().with_initial_scene("/no/such/file.vscn");
    // Must not return an error — missing initial scene degrades gracefully so
    // a fresh project can boot before any scene is saved.
    let app = RuntimeApp::new(cfg).expect("RuntimeApp::new should not fail on missing scene");
    assert_eq!(app.world.entity_count(), 0);
}

// ── v1.0.18: hook + logger installed in new(), not run() ─────────────

#[test]
fn new_with_crash_log_disabled_still_succeeds() {
    // Asserts that opting out of the crash hook does not break the
    // `new` path. With v1.0.18 the hook install moved from `run` to
    // `new`, so the opt-out branch needed to remain wired.
    let cfg = RuntimeConfig::new().disable_crash_log();
    let app = RuntimeApp::new(cfg).expect("new should succeed with crash log disabled");
    assert_eq!(app.world.entity_count(), 0);
}

#[test]
fn new_runs_build_scene_with_logger_initialised() {
    // The logger is initialised at the top of `new`. A build_scene
    // callback that touches the log macros must therefore work.
    // Pre-v1.0.18 the macros would have been emit-only via the
    // default fallback Info-level logger initialised lazily — still
    // fine, but this test pins the contract: macros never panic when
    // called from build_scene.
    let cfg = RuntimeConfig::new()
        .disable_crash_log() // skip global panic-hook install in test
        .with_scene_builder(|world, _reg| {
            crate::log_info!("scene builder ran with {} entities", world.entity_count());
        });
    let _app = RuntimeApp::new(cfg).expect("new should succeed");
}

// ── v1.0.1: add_input_system regression coverage ─────────────────────

#[test]
fn add_input_system_runs_after_regular_systems() {
    // Verifies the ordering invariant `tick_systems_only` documents:
    // input_systems fire *after* user_systems each frame.
    let log = Rc::new(Cell::new(Vec::<&'static str>::new()));
    let l_user = log.clone();
    let l_input = log.clone();
    let cfg = RuntimeConfig::new()
        .disable_physics()
        .disable_scripts()
        .with_system_setup(move |app| {
            let lu = l_user.clone();
            app.add_system(move |_w, _dt| {
                let mut v = lu.take();
                v.push("user");
                lu.set(v);
            });
            let li = l_input.clone();
            app.add_input_system(move |_w, _input, _dt| {
                let mut v = li.take();
                v.push("input");
                li.set(v);
            });
        });
    let mut app = RuntimeApp::new(cfg).unwrap();
    assert_eq!(app.user_system_count(), 1);
    assert_eq!(app.input_system_count(), 1);
    app.tick_systems_only(0.016);
    assert_eq!(log.take(), vec!["user", "input"]);
}

#[test]
fn action_system_reads_input_map_bindings() {
    use voltex_platform::KeyCode;

    let mut map = InputMap::new();
    map.bind_key("jump", KeyCode::Space);
    let cfg = RuntimeConfig::new()
        .disable_physics()
        .disable_scripts()
        .with_input_map(map);
    let mut app = RuntimeApp::new(cfg).unwrap();

    let jumped = Rc::new(Cell::new(false));
    let flag = jumped.clone();
    app.add_action_system(move |_w, input, map, _dt| {
        if map.just_pressed("jump", input) {
            flag.set(true);
        }
    });
    assert_eq!(app.action_system_count(), 1);

    // No key held → action does not fire.
    app.tick_systems_only(0.016);
    assert!(!jumped.get(), "jump fired with no key pressed");

    // Press the bound key → the action system observes it via the map.
    app.input.process_key(KeyCode::Space, true);
    app.tick_systems_only(0.016);
    assert!(jumped.get(), "action system should observe the bound 'jump' key");
}

#[test]
fn time_accumulates_across_ticks() {
    let cfg = RuntimeConfig::new().disable_physics().disable_scripts();
    let mut app = RuntimeApp::new(cfg).unwrap();
    assert_eq!(app.time.frame_count, 0);

    app.tick_systems_only(0.5);
    app.tick_systems_only(0.5);

    assert_eq!(app.time.frame_count, 2);
    assert_eq!(app.time.delta_secs, 0.5);
    assert!((app.time.elapsed_secs - 1.0).abs() < 1e-5, "elapsed={}", app.time.elapsed_secs);
}

#[test]
fn timed_system_receives_advancing_clock() {
    let cfg = RuntimeConfig::new().disable_physics().disable_scripts();
    let mut app = RuntimeApp::new(cfg).unwrap();

    let seen = Rc::new(Cell::new(0.0f32));
    let probe = seen.clone();
    app.add_timed_system(move |_w, time| probe.set(time.elapsed_secs));
    assert_eq!(app.timed_system_count(), 1);

    app.tick_systems_only(0.25);
    assert!((seen.get() - 0.25).abs() < 1e-5, "first frame elapsed = {}", seen.get());
    app.tick_systems_only(0.25);
    assert!((seen.get() - 0.5).abs() < 1e-5, "second frame elapsed = {}", seen.get());
}

#[test]
fn camera_config_defaults_and_builder() {
    let d = RuntimeConfig::default();
    assert_eq!(d.camera_position, Vec3::new(0.0, 10.0, 20.0));
    assert_eq!(d.camera_yaw, 0.0);
    assert!((d.camera_pitch + 0.4).abs() < 1e-6);
    let c = RuntimeConfig::new().with_camera(Vec3::new(1.0, 2.0, 3.0), 0.5, -0.2);
    assert_eq!(c.camera_position, Vec3::new(1.0, 2.0, 3.0));
    assert!((c.camera_yaw - 0.5).abs() < 1e-6);
    assert!((c.camera_pitch + 0.2).abs() < 1e-6);
}

#[test]
fn set_camera_fn_registers_and_runs_on_camera() {
    let cfg = RuntimeConfig::new().disable_physics().disable_scripts().disable_crash_log();
    let mut app = RuntimeApp::new(cfg).unwrap();
    assert!(!app.has_camera_fn());

    // Capture that the updater is actually invoked against a Camera.
    let seen = Rc::new(Cell::new(false));
    let s = seen.clone();
    app.set_camera_fn(move |cam, _world, _dt| {
        cam.position = Vec3::new(3.0, 3.0, 3.0);
        s.set(true);
    });
    assert!(app.has_camera_fn());

    // Drive the updater directly (no window needed) to prove it mutates a Camera.
    let mut fnc = app.camera_fn.take().unwrap();
    let mut cam = voltex_renderer::Camera::new(Vec3::ZERO, 1.0);
    fnc(&mut cam, &app.world, 0.016);
    assert!(seen.get(), "camera_fn should have run");
    assert_eq!(cam.position, Vec3::new(3.0, 3.0, 3.0));
}

#[test]
fn save_and_load_scene_roundtrips_entities() {
    let dir = std::env::temp_dir().join("voltex_runtime_scene_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("scene.json");
    let p = path.to_str().unwrap();

    let cfg = RuntimeConfig::new()
        .disable_physics()
        .disable_scripts()
        .disable_crash_log();
    let mut app = RuntimeApp::new(cfg).unwrap();
    let e = app.world.spawn();
    app.world
        .add(e, Transform::from_position(Vec3::new(3.0, 4.0, 5.0)));
    app.save_scene_json(p).unwrap();

    let mut app2 = RuntimeApp::new(
        RuntimeConfig::new()
            .disable_physics()
            .disable_scripts()
            .disable_crash_log(),
    )
    .unwrap();
    app2.load_scene(p).unwrap();

    assert_eq!(app2.world.entity_count(), 1);
    let pos = app2
        .world
        .query::<Transform>()
        .next()
        .map(|(_, t)| t.position)
        .expect("loaded Transform");
    assert!(
        (pos.x - 3.0).abs() < 1e-4 && (pos.y - 4.0).abs() < 1e-4 && (pos.z - 5.0).abs() < 1e-4,
        "position should roundtrip, got {pos:?}"
    );
}

#[test]
fn load_scene_leaves_pre_load_handles_stale() {
    // audio_listener (and any game-held Entity) set against the previous
    // scene must not alias an entity of the newly loaded one.
    let dir = std::env::temp_dir().join("voltex_runtime_scene_stale_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("scene.json");
    let p = path.to_str().unwrap();

    let cfg = RuntimeConfig::new()
        .disable_physics()
        .disable_scripts()
        .disable_crash_log();
    let mut app = RuntimeApp::new(cfg).unwrap();
    let listener = app.world.spawn();
    app.world.add(listener, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
    app.set_audio_listener(listener);
    app.save_scene_json(p).unwrap();

    app.load_scene(p).unwrap();

    assert_eq!(app.world.entity_count(), 1);
    assert_eq!(app.audio_listener, Some(listener));
    assert!(!app.world.is_alive(listener), "old listener handle aliased a new entity");
    assert!(app.world.get::<Transform>(listener).is_none());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn collision_fn_receives_overlapping_pairs() {
    use voltex_physics::Collider;
    let cfg = RuntimeConfig::new()
        .disable_physics()
        .disable_scripts()
        .disable_crash_log();
    let mut app = RuntimeApp::new(cfg).unwrap();

    // Two overlapping unit spheres → one contact pair.
    let a = app.world.spawn();
    app.world.add(a, Transform::from_position(Vec3::ZERO));
    app.world.add(a, Collider::Sphere { radius: 1.0 });
    let b = app.world.spawn();
    app.world.add(b, Transform::from_position(Vec3::new(0.5, 0.0, 0.0)));
    app.world.add(b, Collider::Sphere { radius: 1.0 });

    let hits = Rc::new(Cell::new(0usize));
    let h = hits.clone();
    app.set_collision_fn(move |contacts, _world, _dt| h.set(contacts.len()));
    assert!(app.has_collision_fn());

    // Trigger detection runs even though physics is disabled.
    app.tick_systems_only(0.016);
    assert!(hits.get() >= 1, "overlapping colliders should report a contact");

    // Separated → no contact.
    if let Some(t) = app.world.get_mut::<Transform>(b) {
        t.position = Vec3::new(10.0, 0.0, 0.0);
    }
    app.tick_systems_only(0.016);
    assert_eq!(hits.get(), 0, "separated colliders should report no contact");
}

#[test]
fn set_light_fn_registers_and_runs_on_light() {
    let cfg = RuntimeConfig::new().disable_physics().disable_scripts().disable_crash_log();
    let mut app = RuntimeApp::new(cfg).unwrap();
    assert!(!app.has_light_fn());
    app.set_light_fn(|light, _world, _dt| {
        light.color = [0.0, 1.0, 0.0];
    });
    assert!(app.has_light_fn());
    let mut fnc = app.light_fn.take().unwrap();
    let mut light = voltex_renderer::LightUniform::new();
    fnc(&mut light, &app.world, 0.016);
    assert_eq!(light.color, [0.0, 1.0, 0.0]);
}

#[test]
fn set_point_lights_fn_registers_and_mutates_set() {
    let cfg = RuntimeConfig::new().disable_physics().disable_scripts().disable_crash_log();
    let mut app = RuntimeApp::new(cfg).unwrap();
    assert!(!app.has_point_lights_fn());
    app.set_point_lights_fn(|lights, _world, _dt| {
        lights.clear();
        lights.push(voltex_renderer::PointLight::new(
            [1.0, 2.0, 3.0],
            [1.0, 0.5, 0.0],
            2.0,
            8.0,
        ));
    });
    assert!(app.has_point_lights_fn());
    let mut fnc = app.point_lights_fn.take().unwrap();
    fnc(&mut app.point_lights, &app.world, 0.016);
    assert_eq!(app.point_lights.len(), 1);
    assert_eq!(app.point_lights[0].position, [1.0, 2.0, 3.0]);
    assert_eq!(app.point_lights[0].range, 2.0);
    assert_eq!(app.point_lights[0].intensity, 8.0);
}

#[test]
fn set_hud_fn_registers_callback() {
    let cfg = RuntimeConfig::new()
        .disable_physics()
        .disable_scripts()
        .disable_crash_log()
        .with_hud(true);
    let mut app = RuntimeApp::new(cfg).unwrap();
    assert!(!app.has_hud_fn(), "no HUD callback before set_hud_fn");
    app.set_hud_fn(|ui, world| {
        ui.text(&format!("entities: {}", world.entity_count()));
    });
    assert!(app.has_hud_fn(), "HUD callback should be registered");
    assert!(app.config.enable_hud);
}

// ── Lua input API wiring ─────────────────────────────────────────────

#[test]
fn fill_script_input_maps_keys_and_buttons() {
    use winit::event::MouseButton;
    use winit::keyboard::KeyCode;
    let mut input = InputState::new();
    input.process_key(KeyCode::KeyW, true);
    input.process_mouse_button(MouseButton::Left, true);
    let mut si = voltex_script::ScriptInput::new();
    fill_script_input(&mut si, &input);
    assert!(si.key_pressed("W"), "W should map to pressed");
    assert!(si.key_just_pressed("W"), "W press edge should map");
    assert!(!si.key_pressed("A"), "A was not pressed");
    assert!(si.button_pressed("left"), "left button should map");
    assert!(!si.button_pressed("right"));
}

#[test]
fn scripts_loaded_from_disk_are_hot_reloaded() {
    use voltex_ecs::{ScriptComponent, Transform};
    use voltex_math::Vec3;
    let dir = std::env::temp_dir().join("voltex_runtime_hot_reload_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("mover.lua");
    let script = |z: f32| {
        format!("function on_start(id, gen) set_position(id, gen, 0.0, 0.0, {z:.1}) end")
    };
    std::fs::write(&path, script(1.0)).unwrap();

    let cfg = RuntimeConfig::new().disable_physics().disable_crash_log();
    let mut app = RuntimeApp::new(cfg).unwrap();
    app.file_watcher = voltex_asset::FileWatcher::new(std::time::Duration::ZERO);
    let e = app.world.spawn();
    app.world.add(e, Transform::from_position(Vec3::ZERO));
    app.world.add(e, ScriptComponent::new(path.to_str().unwrap().to_string()));

    app.tick_systems_only(0.016);
    assert_eq!(app.file_watcher.watched_count(), 1, "the loaded script is watched");
    assert!((app.world.get::<Transform>(e).unwrap().position.z - 1.0).abs() < 1e-6);

    // Edit the file (bump mtime explicitly: filesystems can have coarse mtimes).
    std::fs::write(&path, script(2.0)).unwrap();
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(later).unwrap();

    // Next tick sees the change and reloads; on_start re-runs the tick after.
    app.tick_systems_only(0.016);
    app.tick_systems_only(0.016);
    let z = app.world.get::<Transform>(e).unwrap().position.z;
    assert!((z - 2.0).abs() < 1e-6, "edited script should have re-run on_start, got z={z}");
}

#[test]
fn script_reads_live_input_through_tick() {
    use voltex_ecs::{ScriptComponent, Transform};
    use voltex_math::Vec3;
    let dir = std::env::temp_dir().join("voltex_runtime_input_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("input_mover.lua");
    std::fs::write(
        &path,
        "function on_start(id, gen) end
         function on_update(id, gen, dt)
             if input.is_key_pressed('W') then
                 set_position(id, gen, 0.0, 0.0, 5.0)
             end
         end",
    )
    .unwrap();

    // Scripts are enabled by default; physics off to keep the test focused.
    let cfg = RuntimeConfig::new().disable_physics().disable_crash_log();
    let mut app = RuntimeApp::new(cfg).unwrap();
    let e = app.world.spawn();
    app.world.add(e, Transform::from_position(Vec3::ZERO));
    app.world
        .add(e, ScriptComponent::new(path.to_str().unwrap().to_string()));

    // No key held → entity stays at the origin.
    app.tick_systems_only(0.016);
    assert!(app.world.get::<Transform>(e).unwrap().position.z.abs() < 1e-6);

    // Hold W → the script reads it via the `input` table and moves the entity.
    app.input
        .process_key(winit::keyboard::KeyCode::KeyW, true);
    app.tick_systems_only(0.016);
    let z = app.world.get::<Transform>(e).unwrap().position.z;
    assert!(
        (z - 5.0).abs() < 1e-6,
        "script should have observed W and moved entity, got z={z}"
    );
}

#[test]
fn input_system_receives_app_input_reference() {
    // The input system must observe the runtime's `app.input` field
    // (same reference, same state) so user code can read keyboard /
    // mouse state captured by the winit handler.
    let captured_pressed = Rc::new(Cell::new(false));
    let cap = captured_pressed.clone();
    let cfg = RuntimeConfig::new()
        .disable_physics()
        .disable_scripts()
        .with_system_setup(move |app| {
            // Pre-set a key in the app's input so the system sees it.
            app.input.process_key(
                winit::keyboard::KeyCode::KeyW,
                true,
            );
            let cc = cap.clone();
            app.add_input_system(move |_w, input, _dt| {
                cc.set(input.is_key_pressed(winit::keyboard::KeyCode::KeyW));
            });
        });
    let mut app = RuntimeApp::new(cfg).unwrap();
    app.tick_systems_only(0.016);
    assert!(
        captured_pressed.get(),
        "input system did not see the pre-set KeyW press"
    );
}
