use super::*;

#[test]
fn decide_action_edges() {
    // playing rising edge → Play (spatial flag forwarded)
    assert_eq!(
        decide_action(true, false, false),
        AudioAction::Play { spatial: false }
    );
    assert_eq!(
        decide_action(true, false, true),
        AudioAction::Play { spatial: true }
    );
    // held playing (already active) → nothing (no re-trigger every frame)
    assert_eq!(decide_action(true, true, false), AudioAction::None);
    // stop edge
    assert_eq!(decide_action(false, true, false), AudioAction::Stop);
    // idle
    assert_eq!(decide_action(false, false, false), AudioAction::None);
}

#[test]
fn world_clear_stales_handles_so_sweep_catches_old_voice_and_stop_all_drains() {
    let audio = AudioSystem::new(vec![voltex_audio::AudioClip::new(
        vec![0.0; 8],
        44100,
        1,
    )])
    .expect("audio system");
    let mut world = World::new();
    let mut voices = AudioVoiceTracker::default();

    let looping_src = || {
        let mut s = AudioSource::new(0);
        s.looping = true;
        s.playing = true;
        s
    };
    let e = world.spawn();
    world.add(e, voltex_ecs::Transform::new());
    world.add(e, looping_src());
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 1);

    // v1.0.120: clear()는 슬롯 generation을 올린다 — 같은 id가 재발급돼도
    // 옛 핸들과 구분되므로 sweep이 이전 voice를 orphan으로 잡는다.
    world.clear();
    let e2 = world.spawn();
    assert_eq!(e2.id, e.id, "slot is reused");
    assert_ne!(e, e2, "but the handle is distinguishable (no ABA)");
    world.add(e2, voltex_ecs::Transform::new());
    world.add(e2, looping_src());
    audio_sync_system(&mut world, &audio, None, &mut voices);
    // 이전 voice는 sweep으로 stop+해제, 새 voice만 추적.
    assert_eq!(voices.len(), 1);
    assert!(voices.voices.contains_key(&e2));
    assert!(!voices.voices.contains_key(&e));

    // load_scene 경로가 쓰는 일괄 해제.
    voices.stop_all(&audio);
    assert!(voices.is_empty());
}

#[test]
fn replay_after_natural_finish_produces_new_edge() {
    // 클립 자연 종료 후 상태: playing=true(사용자 미변경), active=true
    // (믹서 종료 피드백이 ECS로 오지 않음). play()가 active를 리셋해야
    // 다음 sync가 Play 엣지를 본다.
    let mut src = AudioSource::new(0);
    src.playing = true;
    src.active = true;
    src.play();
    assert_eq!(
        decide_action(src.playing, src.active, src.spatial),
        AudioAction::Play { spatial: false }
    );
}

#[test]
fn poll_audio_hot_reload_swaps_changed_watched_files_only() {
    use std::time::Duration;
    let dir = std::env::temp_dir();
    let watched_path = dir.join(format!("voltex_audio_hr_{}.wav", std::process::id()));
    let unwatched_path = dir.join(format!("voltex_audio_hr_un_{}.wav", std::process::id()));
    let wav = voltex_audio::generate_wav_bytes(&[0.0, 0.1, -0.1], 44100);
    std::fs::write(&watched_path, &wav).unwrap();
    std::fs::write(&unwatched_path, &wav).unwrap();

    let audio = AudioSystem::new(vec![voltex_audio::AudioClip::new(
        vec![0.0; 8],
        44100,
        1,
    )])
    .expect("audio system");
    let mut watcher = voltex_asset::FileWatcher::new(Duration::from_millis(0));
    watcher.watch(watched_path.clone());
    watcher.watch(unwatched_path.clone());
    let watched: std::collections::HashMap<std::path::PathBuf, usize> =
        [(watched_path.clone(), 0usize)].into_iter().collect();

    // First poll establishes the mtime baseline — no outcomes.
    assert!(poll_audio_hot_reload(&mut watcher, &watched, &audio).is_empty());

    // Touch both files; only the mapped one produces an outcome.
    std::thread::sleep(Duration::from_millis(20));
    let wav2 = voltex_audio::generate_wav_bytes(&[0.5, -0.5], 22050);
    std::fs::write(&watched_path, &wav2).unwrap();
    std::fs::write(&unwatched_path, &wav2).unwrap();
    let outcomes = poll_audio_hot_reload(&mut watcher, &watched, &audio);
    assert_eq!(outcomes.len(), 1, "only the mapped path is reloaded");
    assert_eq!(outcomes[0].0, watched_path);
    assert!(outcomes[0].1.is_ok(), "valid wav swaps Ok: {:?}", outcomes[0].1);

    let _ = std::fs::remove_file(&watched_path);
    let _ = std::fs::remove_file(&unwatched_path);
}

#[test]
fn poll_audio_hot_reload_reports_decode_failure_without_panicking() {
    use std::time::Duration;
    let path = std::env::temp_dir()
        .join(format!("voltex_audio_hr_bad_{}.wav", std::process::id()));
    std::fs::write(&path, voltex_audio::generate_wav_bytes(&[0.0], 44100)).unwrap();

    let audio = AudioSystem::new(Vec::new()).expect("audio system");
    let mut watcher = voltex_asset::FileWatcher::new(Duration::from_millis(0));
    watcher.watch(path.clone());
    let watched: std::collections::HashMap<std::path::PathBuf, usize> =
        [(path.clone(), 0usize)].into_iter().collect();
    let _ = poll_audio_hot_reload(&mut watcher, &watched, &audio); // baseline

    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(&path, b"RIFFcorrupt").unwrap();
    let outcomes = poll_audio_hot_reload(&mut watcher, &watched, &audio);
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].1.is_err(), "corrupt file reports Err, old clip kept");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sync_sets_active_on_play_and_clears_on_stop() {
    // AudioSystem::new spawns a background thread but needs no device to
    // accept fire-and-forget commands, so this runs headless.
    let audio = AudioSystem::new(Vec::new()).expect("audio system");
    let mut world = World::new();
    let mut voices = AudioVoiceTracker::default();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::ZERO));
    let mut src = AudioSource::new(0u32);
    src.playing = true;
    world.add(e, src);

    // First sync: rising edge → active becomes true.
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert!(world.get::<AudioSource>(e).unwrap().active, "play edge sets active");

    // Second sync with still-playing: no change, stays active (idempotent).
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert!(world.get::<AudioSource>(e).unwrap().active);

    // Toggle off → stop edge clears active.
    world.get_mut::<AudioSource>(e).unwrap().playing = false;
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert!(!world.get::<AudioSource>(e).unwrap().active, "stop edge clears active");
}

#[test]
fn non_spatial_source_without_transform_is_handled() {
    let audio = AudioSystem::new(Vec::new()).expect("audio system");
    let mut world = World::new();
    let mut voices = AudioVoiceTracker::default();
    let e = world.spawn();
    // No Transform — a non-spatial UI/music source.
    let mut src = AudioSource::new(3u32);
    src.playing = true;
    world.add(e, src);
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert!(
        world.get::<AudioSource>(e).unwrap().active,
        "non-spatial source without Transform should still start"
    );
}

#[test]
fn play_edge_tracks_voice_and_stop_edge_untracks() {
    let audio = AudioSystem::new(Vec::new()).expect("audio system");
    let mut world = World::new();
    let mut voices = AudioVoiceTracker::default();
    let e = world.spawn();
    let mut src = AudioSource::new(0u32);
    src.looping = true;
    src.playing = true;
    world.add(e, src);

    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 1, "play edge must register the issued voice");

    world.get_mut::<AudioSource>(e).unwrap().playing = false;
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 0, "stop edge must release the tracked voice");
}

#[test]
fn despawned_looping_source_voice_is_swept() {
    // The dogfood friction #9 case: a looping BGM entity is despawned
    // without ever toggling playing=false — no Stop edge fires, but the
    // tracker must notice the dead entity and stop its orphan voice.
    let audio = AudioSystem::new(Vec::new()).expect("audio system");
    let mut world = World::new();
    let mut voices = AudioVoiceTracker::default();
    let e = world.spawn();
    let mut src = AudioSource::new(0u32);
    src.looping = true;
    src.playing = true;
    world.add(e, src);

    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 1);

    world.despawn(e);
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 0, "dead entity's voice must be stopped + untracked");
}

#[test]
fn world_clear_sweeps_all_tracked_voices() {
    let audio = AudioSystem::new(Vec::new()).expect("audio system");
    let mut world = World::new();
    let mut voices = AudioVoiceTracker::default();
    for clip in 0..3u32 {
        let e = world.spawn();
        let mut src = AudioSource::new(clip);
        src.looping = true;
        src.playing = true;
        world.add(e, src);
    }
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 3);

    world.clear();
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 0, "world.clear() must orphan-sweep every voice");
}

#[test]
fn removed_audio_source_component_is_swept() {
    let audio = AudioSystem::new(Vec::new()).expect("audio system");
    let mut world = World::new();
    let mut voices = AudioVoiceTracker::default();
    let e = world.spawn();
    let mut src = AudioSource::new(0u32);
    src.looping = true;
    src.playing = true;
    world.add(e, src);

    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 1);

    world.remove::<AudioSource>(e);
    audio_sync_system(&mut world, &audio, None, &mut voices);
    assert_eq!(voices.len(), 0, "entity alive but source removed → voice swept");
}

#[test]
fn audio_source_serialize_round_trip() {
    let mut world = World::new();
    let e = world.spawn();
    let mut src = AudioSource::new(7u32).spatial().looping();
    src.volume = 0.42;
    src.playing = true;
    src.played_once = true; // transient — must NOT survive
    src.active = true; //       transient — must NOT survive
    world.add(e, src);

    let bytes = serialize_audio_source(&world, e).expect("serialize");
    assert_eq!(bytes.len(), AUDIO_SOURCE_BYTES);

    let mut world2 = World::new();
    let e2 = world2.spawn();
    deserialize_audio_source(&mut world2, e2, &bytes).expect("deserialize");

    let got = world2.get::<AudioSource>(e2).expect("round-tripped source");
    assert_eq!(got.clip_id.0, 7);
    assert!((got.volume - 0.42).abs() < 1e-6);
    assert!(got.spatial);
    assert!(got.looping);
    assert!(got.playing);
    assert!(!got.played_once, "transient played_once must reset");
    assert!(!got.active, "transient active must reset");
}

#[test]
fn audio_source_deserialize_rejects_truncated() {
    let mut world = World::new();
    let e = world.spawn();
    assert!(deserialize_audio_source(&mut world, e, &[0u8; 4]).is_err());
}

#[test]
fn register_audio_source_round_trips_through_registry() {
    // End-to-end: registering via register_raw makes the component show up
    // in a JSON scene and load back.
    let mut registry = voltex_ecs::ComponentRegistry::new();
    registry.register_defaults();
    register_audio_source(&mut registry);

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::default());
    world.add(e, AudioSource::new(5u32).looping());

    let json = voltex_ecs::serialize_scene_json(&world, &registry);
    assert!(json.contains(AUDIO_SOURCE_COMPONENT), "scene JSON must list the audio source");

    let mut world2 = World::new();
    voltex_ecs::deserialize_scene_json(&mut world2, &json, &registry).expect("load scene");
    let (_, src) = world2.query::<AudioSource>().next().expect("audio source survived round-trip");
    assert_eq!(src.clip_id.0, 5);
    assert!(src.looping);
}
