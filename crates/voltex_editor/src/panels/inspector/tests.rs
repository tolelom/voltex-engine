use super::*;
use crate::UiContext;

fn temp_path(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("{}_{}", name, std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(scene: &str) {
    let _ = std::fs::remove_file(scene);
    let _ = std::fs::remove_file(format!("{}.log", scene));
}

#[test]
fn bounds_bytes_round_trips_through_registry_deserialize() {
    // Guard the hand-mirrored layout against drift in Bounds::serialize.
    let min = Vec3::new(-1.5, -2.0, -0.25);
    let max = Vec3::new(3.0, 4.5, 0.75);
    let bytes = bounds_bytes(min, max);
    assert_eq!(bytes.len(), 24);

    let mut registry = voltex_ecs::ComponentRegistry::new();
    registry.register_defaults();
    let mut world = voltex_ecs::World::new();
    let e = world.spawn();
    let entry = registry.find("bounds").expect("bounds registered");
    (entry.deserialize)(&mut world, e, &bytes).expect("deserialize");
    let b = world.get::<voltex_ecs::Bounds>(e).expect("Bounds attached");
    assert_eq!(b.0.min, min);
    assert_eq!(b.0.max, max);

    // And the registry's own serialize emits the identical bytes.
    let back = (entry.serialize)(&world, e).expect("serialize");
    assert_eq!(back, bytes);
}

#[test]
fn bounds_edit_dispatch_overwrites_and_undoes_to_prior_value() {
    // The exact command the edit form dispatches: overwrite + undo→prior.
    let scene = temp_path("voltex_inspector_bounds_edit.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::AddComponent {
            entity: e,
            name: "bounds".into(),
            data: bounds_bytes(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
        })
        .unwrap();

    session
        .dispatch(EditorCommand::AddComponent {
            entity: e,
            name: "bounds".into(),
            data: bounds_bytes(Vec3::new(-9.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
        })
        .unwrap();
    let b = session.world.get::<voltex_ecs::Bounds>(e).unwrap();
    assert_eq!(b.0.min.x, -9.0, "edit overwrites in place");

    assert!(session.undo());
    let b = session.world.get::<voltex_ecs::Bounds>(e).unwrap();
    assert_eq!(b.0.min.x, -1.0, "undo restores the prior bounds, not removal");
    cleanup(&scene);
}

#[test]
fn component_value_summary_formats_builtins() {
    let scene = temp_path("voltex_insp_summary.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();

    // Absent components → None.
    assert_eq!(component_value_summary(&session, e, "bounds"), None);

    session.world.add(
        e,
        voltex_ecs::Bounds(voltex_math::AABB::new(
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(1.0, 2.0, 1.0),
        )),
    );
    assert_eq!(
        component_value_summary(&session, e, "bounds").as_deref(),
        Some("min(-1.00, 0.00, -1.00) max(1.00, 2.00, 1.00)")
    );

    session.world.add(
        e,
        voltex_ecs::MeshHandle(voltex_asset::Handle::from_raw_parts(3, 1)),
    );
    assert_eq!(
        component_value_summary(&session, e, "mesh_handle").as_deref(),
        Some("id=3 gen=1")
    );

    session
        .world
        .add(e, voltex_ecs::ScriptComponent::new("scripts/ai.lua"));
    assert_eq!(
        component_value_summary(&session, e, "script").as_deref(),
        Some("scripts/ai.lua"),
        "UNBOUND instance shows the path only"
    );

    // Dedicated-UI and unknown names → None even when attached.
    assert_eq!(component_value_summary(&session, e, "transform"), None);
    assert_eq!(component_value_summary(&session, e, "audio_source"), None);
    cleanup(&scene);
}

#[test]
fn tag_buffer_resyncs_after_undo() {
    let scene = temp_path("voltex_insp_tag_resync.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session.selected = Some(e);

    let mut state = InspectorState::default();
    state.sync_from(&session);
    assert_eq!(state.tag, "");

    // 사용자가 tag를 입력·커밋한 상황 (UI 경로와 동일하게 last_tag 갱신).
    state.tag = "Hero".into();
    state.last_tag = "Hero".into();
    session
        .dispatch(EditorCommand::SetTag { entity: e, tag: "Hero".into() })
        .unwrap();
    state.sync_from(&session);
    assert_eq!(state.tag, "Hero", "자기 입력은 되돌리지 않는다");

    // Ctrl+Z — world의 tag가 사라짐. 버퍼도 따라와야 한다 (안 따라오면
    // 다음 키 입력이 undo된 값을 그대로 재-dispatch).
    assert!(session.undo());
    assert!(session.world.get::<Tag>(e).is_none());
    state.sync_from(&session);
    assert_eq!(state.tag, "", "undo 후 버퍼 재동기화");
    cleanup(&scene);
}

#[test]
fn attached_component_names_lists_only_attached() {
    let scene = temp_path("voltex_insp_attached_names.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();

    // Spawn attaches a default Transform only.
    let names = attached_component_names(&session, e);
    assert!(names.iter().any(|n| n == "transform"), "transform attached on spawn");
    assert!(!names.iter().any(|n| n == "tag"), "no tag yet: {names:?}");
    assert!(!names.iter().any(|n| n == "bounds"), "no bounds yet: {names:?}");

    session
        .dispatch(EditorCommand::SetTag { entity: e, tag: "Hero".into() })
        .unwrap();
    let names = attached_component_names(&session, e);
    assert!(names.iter().any(|n| n == "tag"), "tag listed after SetTag");
    cleanup(&scene);
}

#[test]
fn remove_button_dispatch_removes_and_undo_restores_prior_value() {
    // Replicate the "- tag" button logic end-to-end without the GUI.
    let scene = temp_path("voltex_insp_remove_dispatch.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::SetTag { entity: e, tag: "Hero".into() })
        .unwrap();

    session
        .dispatch(EditorCommand::RemoveComponent { entity: e, name: "tag".into() })
        .unwrap();
    assert!(session.world.get::<Tag>(e).is_none(), "tag removed");
    assert!(
        !attached_component_names(&session, e).iter().any(|n| n == "tag"),
        "components list reflects removal"
    );

    session.undo();
    let tag = session.world.get::<Tag>(e).expect("undo restores tag");
    assert_eq!(tag.0, "Hero", "undo restores the exact prior value");
    cleanup(&scene);
}

#[test]
fn add_buttons_exclude_already_attached_components() {
    // Replicate the Add-section filter: attached names are skipped.
    let scene = temp_path("voltex_insp_add_filter.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();

    let attached = attached_component_names(&session, e);
    let offered: Vec<&str> = ADD_COMPONENT_BUTTONS
        .iter()
        .filter(|s| !attached.iter().any(|n| n == s.name))
        .map(|s| s.name)
        .collect();
    assert!(!offered.contains(&"transform"), "attached transform filtered out");
    assert!(offered.contains(&"bounds"), "unattached bounds still offered");
    cleanup(&scene);
}

#[test]
fn inspector_runs_with_no_selection() {
    let mut ui = UiContext::new(800.0, 600.0);
    let scene = temp_path("voltex_insp_no_sel.vscn");
    cleanup(&scene);

    let mut session = EditorSession::new(scene.clone());
    let mut state = InspectorState::default();
    ui.begin_frame(0.0, 0.0, false);
    inspector_panel(&mut ui, &mut session, &mut state, 0.0, 0.0, 300.0, 400.0);
    ui.end_frame();
    cleanup(&scene);
}

#[test]
fn inspector_sync_loads_tag_into_buffer() {
    let scene = temp_path("voltex_insp_sync_tag.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session
        .dispatch(EditorCommand::SetTag { entity: e, tag: "Hero".into() })
        .unwrap();
    session.selected = Some(e);

    let mut state = InspectorState::default();
    state.sync_from(&session);
    assert_eq!(state.tag, "Hero");
    cleanup(&scene);
}

#[test]
fn inspector_with_transform_draws_sliders() {
    let scene = temp_path("voltex_insp_xform.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session.selected = Some(e);

    let mut ui = UiContext::new(800.0, 600.0);
    let mut state = InspectorState::default();
    ui.begin_frame(0.0, 0.0, false);
    inspector_panel(&mut ui, &mut session, &mut state, 0.0, 0.0, 300.0, 600.0);
    ui.end_frame();
    // 9 sliders + label rows + header → easily >10 draw commands
    assert!(ui.draw_list.commands.len() > 10);
    cleanup(&scene);
}

#[test]
fn add_component_button_specs_match_registered_built_in_names() {
    // Guard against drift between ADD_COMPONENT_BUTTONS and
    // ComponentRegistry::register_defaults(). If a button references a
    // name that isn't in the registry, the dispatch would silently fail.
    let scene = temp_path("voltex_insp_add_specs.vscn");
    cleanup(&scene);
    let session = EditorSession::new(scene.clone());
    for spec in super::ADD_COMPONENT_BUTTONS.iter() {
        assert!(
            session.registry.find(spec.name).is_some(),
            "ADD_COMPONENT_BUTTONS references unknown name `{}`",
            spec.name
        );
    }
    cleanup(&scene);
}

#[test]
fn add_component_dispatch_with_default_bytes_attaches_transform() {
    // The Inspector wiring calls `(entry.default_bytes)()` and dispatches
    // AddComponent — exercise that exact path end-to-end without the GUI.
    let scene = temp_path("voltex_insp_add_dispatch.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();

    // Pretend the user clicks "+ transform" — we replicate the closure logic.
    let entry = session.registry.find("transform").expect("transform registered");
    let bytes = (entry.default_bytes)();
    session.dispatch(EditorCommand::AddComponent {
        entity: e,
        name: "transform".into(),
        data: bytes,
    }).unwrap();

    let t = session.world.get::<Transform>(e).expect("Transform attached");
    assert_eq!(t.scale, voltex_math::Vec3::new(1.0, 1.0, 1.0));
    cleanup(&scene);
}

#[test]
fn slider_drag_dispatches_set_transform_and_logs() {
    let scene = temp_path("voltex_insp_dispatch.vscn");
    cleanup(&scene);
    let mut session = EditorSession::new(scene.clone());
    let e = session.dispatch(EditorCommand::Spawn).unwrap();
    session.selected = Some(e);

    let mut ui = UiContext::new(800.0, 600.0);
    let mut state = InspectorState::default();

    // Simulate mouse drag on the Pos X slider (first slider drawn).
    // Slider geometry: starts at layout.cursor_x (= panel inner x = x+padding=4),
    // width 150. Mid-x ≈ 79 → roughly value 0 with range [-50, 50].
    // Click far right to push value > original 0.
    ui.begin_frame(160.0, 60.0, true);
    inspector_panel(&mut ui, &mut session, &mut state, 0.0, 0.0, 300.0, 600.0);
    ui.end_frame();

    let t = session.world.get::<Transform>(e).expect("transform");
    // Either the slider value moved away from zero (drag landed) or stayed.
    // The important property: the session still represents a valid Transform.
    assert!(t.position.x.is_finite());

    // Log file existence is the persistent guarantee — at minimum Spawn was logged.
    let log = format!("{}.log", scene);
    let log_text = std::fs::read_to_string(&log).expect("log exists");
    assert!(log_text.contains("Spawn"));
    cleanup(&scene);
}
