-- player.lua — auto-fire at the nearest enemy every 0.8s.
-- Player movement is driven from Rust (input_system in src/main.rs).

local fire_cd = 0.8

function on_start(id, gen)
    fire_cd = 0.8
end

function on_update(id, gen, dt)
    fire_cd = fire_cd - dt
    if fire_cd > 0 then return end

    local px, py, pz = get_position(id, gen)
    if px == nil then return end

    -- Find nearest enemy by squared distance.
    local enemies = iter_by_tag("enemy")
    if enemies == nil or #enemies == 0 then return end
    local best_d, best_x, best_z = 1e9, 0, 0
    local found = false
    for _, e in ipairs(enemies) do
        local ex, ey, ez = get_position(e.id, e.gen)
        if ex ~= nil then
            local dx, dz = ex - px, ez - pz
            local d = dx * dx + dz * dz
            if d < best_d then
                best_d, best_x, best_z = d, ex, ez
                found = true
            end
        end
    end
    if not found then return end

    -- Spawn a small projectile at the player, tagged so Rust attaches
    -- mesh/material handles next frame.
    local proj_id, proj_gen = spawn_entity()
    set_position(proj_id, proj_gen, px, py, pz)
    set_scale(proj_id, proj_gen, 0.25, 0.25, 0.25)
    -- Encode target direction in the rotation field (XZ unit vector), so
    -- projectile.lua can read it without needing a separate component.
    -- (rotation.y is repurposed as direction angle).
    local dx, dz = best_x - px, best_z - pz
    local len = math.sqrt(dx * dx + dz * dz)
    if len < 1e-6 then return end
    -- math.atan(y, x) acts as atan2 in Lua 5.4.
    local angle = math.atan(dz, dx)
    set_rotation(proj_id, proj_gen, 0, angle, 0)
    set_tag(proj_id, proj_gen, "projectile")
    fire_cd = 0.8
end
