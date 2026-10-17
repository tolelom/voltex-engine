use super::*;
use crate::ImageStats;

/// Count pixels whose RGB differs by more than `tol` between two frames.
fn diff_pixels(a: &[u8], b: &[u8], tol: i32) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(pa, pb)| {
            (pa[0] as i32 - pb[0] as i32).abs() > tol
                || (pa[1] as i32 - pb[1] as i32).abs() > tol
                || (pa[2] as i32 - pb[2] as i32).abs() > tol
        })
        .count()
}

#[test]
#[ignore = "requires a GPU adapter"]
fn load_skinned_gltf_builds_mesh_and_pose() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    use voltex_renderer::gltf::{GltfData, GltfMesh, GltfNode, GltfSkin};
    use voltex_renderer::MeshVertex;
    use voltex_renderer_ecs::{load_skinned_gltf, RenderAssets};

    let node = |t: [f32; 3], children: Vec<usize>| GltfNode {
        name: None,
        children,
        translation: t,
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
        mesh: None,
        skin: None,
    };
    let data = GltfData {
        meshes: vec![GltfMesh {
            vertices: vec![MeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                tangent: [1.0, 0.0, 0.0, 1.0],
            }],
            indices: vec![0],
            name: None,
            material: None,
            joints: Some(vec![[1u16, 0, 0, 0]]),
            weights: Some(vec![[1.0f32, 0.0, 0.0, 0.0]]),
        }],
        nodes: vec![node([0.0, 0.0, 0.0], vec![1]), node([0.0, 1.0, 0.0], vec![])],
        skins: vec![GltfSkin {
            name: None,
            joints: vec![0, 1],
            inverse_bind_matrices: vec![IDENTITY4, IDENTITY4],
            skeleton: None,
        }],
        animations: vec![],
    };

    let mut assets = RenderAssets::new();
    let (handle, pose) =
        load_skinned_gltf(&gpu.device, &mut assets, &data).expect("skinned glTF loads");
    assert_eq!(pose.skeleton.joint_count(), 2, "two joints");
    assert!(assets.skinned_mesh(handle).is_some(), "skinned mesh uploaded");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn runtime_basic_renderer_skins_and_animates() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: no GPU adapter ({e})");
            return;
        }
    };
    let (w, h) = (256, 256);
    // Drive the skinned strip through BasicRenderer (the runtime path) at
    // rest vs animated — the GPU-skinned geometry must differ.
    let rest = render_skinned_runtime(&gpu, w, h, 0.0);
    let posed = render_skinned_runtime(&gpu, w, h, 1.0);
    assert!(
        !ImageStats::analyze(&rest).looks_blank(),
        "runtime skinned strip should render"
    );
    let changed = diff_pixels(&rest, &posed, 16);
    assert!(
        changed > 150,
        "animating the runtime skinned entity should change pixels, got {changed}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn runtime_skinned_pbr_path_renders_and_animates() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: no GPU adapter ({e})");
            return;
        }
    };
    let (w, h) = (256, 256);
    // The PBR skinned pipeline must compile + render, deform with the pose,
    // and shade differently from the colored skinned path.
    let pbr_rest = render_skinned_runtime_mode(&gpu, w, h, 0.0, true);
    let pbr_posed = render_skinned_runtime_mode(&gpu, w, h, 1.0, true);
    crate::write_visual_record("skinned_pbr.png", w, h, &pbr_posed);

    assert!(
        !ImageStats::analyze(&pbr_rest).looks_blank(),
        "PBR skinned strip should render"
    );
    assert!(
        diff_pixels(&pbr_rest, &pbr_posed, 16) > 150,
        "PBR skinned entity should deform with the pose"
    );
    // PBR shading should differ from the colored skinned path on the same pose.
    let colored_posed = render_skinned_runtime_mode(&gpu, w, h, 1.0, false);
    assert!(
        diff_pixels(&colored_posed, &pbr_posed, 16) > 100,
        "PBR skinned shading should differ from the colored skinned path"
    );
}

/// Count pixels that got darker (in luminance) in `a` vs `b` by more than
/// `tol` — i.e. where a shadow fell.
fn darkened_pixels(a: &[u8], b: &[u8], tol: i32) -> usize {
    let lum = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(pa, pb)| lum(pb) - lum(pa) > tol)
        .count()
}

#[test]
#[ignore = "requires a GPU adapter"]
fn skinned_mesh_casts_shadow() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: no GPU adapter ({e})");
            return;
        }
    };
    let (w, h) = (512, 512);

    // Strip present: shadows on vs off. The strip's own pixels don't receive
    // shadow (the skinned shader ignores the shadow map), so the darkening is
    // the ground shadow cast BY the skinned strip.
    let on = render_skinned_shadow_scene(&gpu, w, h, true, false);
    let off = render_skinned_shadow_scene(&gpu, w, h, false, false);
    let cast = darkened_pixels(&on, &off, 40);

    // Control: strip hidden → no caster. Any residual darkening (flat-ground
    // self-shadow) must be far smaller than the strip's cast shadow.
    let on_ctrl = render_skinned_shadow_scene(&gpu, w, h, true, true);
    let off_ctrl = render_skinned_shadow_scene(&gpu, w, h, false, true);
    let baseline = darkened_pixels(&on_ctrl, &off_ctrl, 40);

    crate::write_visual_record("skinned_shadow.png", w, h, &on);

    assert!(
        cast > 500,
        "skinned strip should cast a visible ground shadow, got {cast} darkened px"
    );
    assert!(
        cast > baseline * 4,
        "skinned cast shadow ({cast}) should dominate flat-ground baseline ({baseline})"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn skinned_bend_deforms_geometry() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: no GPU adapter ({e})");
            return;
        }
    };
    let (w, h) = (256, 256);

    // Rest pose must draw the strip (not a blank frame).
    let rest = render_skinned_rest(&gpu, w, h);
    let stats = ImageStats::analyze(&rest);
    assert!(!stats.looks_blank(), "rest skinned strip should not be blank");

    // Bent pose must differ from the rest pose — the GPU palette deformed
    // the top half of the strip.
    let bent = render_skinned_bent(&gpu, w, h, std::f32::consts::FRAC_PI_3);
    let changed = diff_pixels(&rest, &bent, 16);
    assert!(
        changed > 200,
        "bending the joint should change many pixels, got {changed}"
    );
}
