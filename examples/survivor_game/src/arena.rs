use voltex_renderer::vertex::MeshVertex;

/// Generate a box mesh with the given half-extents centered at the origin.
/// Returns (vertices, indices) with u32 indices (matching Mesh::new).
pub fn generate_box(width: f32, height: f32, depth: f32) -> (Vec<MeshVertex>, Vec<u32>) {
    let hw = width * 0.5;
    let hh = height * 0.5;
    let hd = depth * 0.5;

    // 6 faces, 4 vertices each = 24 vertices
    // Each face has outward-pointing normal, UV from 0..1, and tangent.
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    // Helper to push a quad (4 verts + 6 indices)
    let mut push_face = |positions: [[f32; 3]; 4],
                         normal: [f32; 3],
                         tangent: [f32; 4],
                         uvs: [[f32; 2]; 4]| {
        let base = vertices.len() as u32;
        for i in 0..4 {
            vertices.push(MeshVertex {
                position: positions[i],
                normal,
                uv: uvs[i],
                tangent,
            });
        }
        // Two triangles: 0-1-2, 0-2-3
        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 3);
    };

    // +Y face (top)
    push_face(
        [
            [-hw, hh, -hd],
            [-hw, hh, hd],
            [hw, hh, hd],
            [hw, hh, -hd],
        ],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
        [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
    );

    // -Y face (bottom)
    push_face(
        [
            [-hw, -hh, hd],
            [-hw, -hh, -hd],
            [hw, -hh, -hd],
            [hw, -hh, hd],
        ],
        [0.0, -1.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
        [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
    );

    // +Z face (front)
    push_face(
        [
            [-hw, -hh, hd],
            [hw, -hh, hd],
            [hw, hh, hd],
            [-hw, hh, hd],
        ],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0, 1.0],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    );

    // -Z face (back)
    push_face(
        [
            [hw, -hh, -hd],
            [-hw, -hh, -hd],
            [-hw, hh, -hd],
            [hw, hh, -hd],
        ],
        [0.0, 0.0, -1.0],
        [-1.0, 0.0, 0.0, 1.0],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    );

    // +X face (right)
    push_face(
        [
            [hw, -hh, hd],
            [hw, -hh, -hd],
            [hw, hh, -hd],
            [hw, hh, hd],
        ],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 1.0],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    );

    // -X face (left)
    push_face(
        [
            [-hw, -hh, -hd],
            [-hw, -hh, hd],
            [-hw, hh, hd],
            [-hw, hh, -hd],
        ],
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 1.0],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    );

    (vertices, indices)
}

/// An arena entity: a mesh with a model transform and material parameters.
pub struct ArenaEntity {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
}

/// Generate the arena: 1 floor + 4 obstacles.
pub fn generate_arena() -> Vec<ArenaEntity> {
    let mut entities = Vec::new();

    // Floor: 20x0.2x20 box (flat ground)
    let (floor_v, floor_i) = generate_box(20.0, 0.2, 20.0);
    entities.push(ArenaEntity {
        vertices: floor_v,
        indices: floor_i,
        base_color: [0.5, 0.5, 0.5, 1.0], // gray
        metallic: 0.0,
        roughness: 0.8,
    });

    // 4 obstacle boxes: 2x1x2 each at predefined positions
    let obstacle_positions: [(f32, f32, f32); 4] = [
        (-5.0, 0.6, -3.0),
        (4.0, 0.6, 2.0),
        (-3.0, 0.6, 5.0),
        (6.0, 0.6, -4.0),
    ];

    for _pos in &obstacle_positions {
        let (obs_v, obs_i) = generate_box(2.0, 1.0, 2.0);
        entities.push(ArenaEntity {
            vertices: obs_v,
            indices: obs_i,
            base_color: [0.3, 0.3, 0.35, 1.0], // dark gray
            metallic: 0.1,
            roughness: 0.6,
        });
    }

    entities
}

/// Model matrices for the 5 entities (floor + 4 obstacles).
pub fn arena_model_matrices() -> Vec<voltex_math::Mat4> {
    use voltex_math::Mat4;

    let mut models = Vec::new();

    // Floor centered at origin, y=0 (the box is 0.2 tall, so top is at y=0.1)
    models.push(Mat4::translation(0.0, 0.0, 0.0));

    // Obstacles
    let obstacle_positions: [(f32, f32, f32); 4] = [
        (-5.0, 0.6, -3.0),
        (4.0, 0.6, 2.0),
        (-3.0, 0.6, 5.0),
        (6.0, 0.6, -4.0),
    ];

    for (x, y, z) in &obstacle_positions {
        models.push(Mat4::translation(*x, *y, *z));
    }

    models
}
