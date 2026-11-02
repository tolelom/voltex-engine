-- projectile.lua — fly in the rotation.y direction at constant speed,
-- check for nearby enemy and kill on contact, or self-destruct after a
-- distance/time budget.

local SPEED = 6.0
local HIT_DIST_SQ = 0.4 * 0.4
local LIFETIME = 3.0

-- Each projectile entity has its own age tracked in this table keyed by id.
-- A single LuaState is shared across all projectile entities (path-based pool).
local ages = {}

function on_update(id, gen, dt)
    local px, py, pz = get_position(id, gen)
    if px == nil then return end

    local key = id * 0x100000000 + gen  -- composite key
    ages[key] = (ages[key] or 0) + dt

    if ages[key] > LIFETIME then
        ages[key] = nil
        destroy_entity(id, gen)
        return
    end

    -- Move along stored rotation.y direction.
    local rx, ry, rz = get_rotation(id, gen)
    if ry == nil then return end
    local nx, nz = math.cos(ry), math.sin(ry)
    set_position(id, gen, px + nx * SPEED * dt, py, pz + nz * SPEED * dt)

    -- Check enemies.
    local enemies = iter_by_tag("enemy")
    if enemies == nil then return end
    for _, e in ipairs(enemies) do
        local ex, ey, ez = get_position(e.id, e.gen)
        if ex ~= nil then
            local dx, dz = ex - px, ez - pz
            if dx * dx + dz * dz < HIT_DIST_SQ then
                destroy_entity(e.id, e.gen)
                destroy_entity(id, gen)
                ages[key] = nil
                -- ScriptInstancePool isolates each .lua to its own LuaState,
                -- so we can't call into game.lua directly. Instead, drop a
                -- "kill_marker" entity that game.lua scans + counts.
                local mid, mgen = spawn_entity()
                set_tag(mid, mgen, "kill_marker")
                return
            end
        end
    end
end
