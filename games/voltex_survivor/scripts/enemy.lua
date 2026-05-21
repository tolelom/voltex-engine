-- enemy.lua — chase the player; on contact, drop a damage_marker and self-destruct.
-- A single LuaState is shared across every enemy entity (ScriptInstancePool
-- groups by .lua path), so module-level state stays cheap.

local SPEED = 1.2
local HIT_DIST_SQ = 0.55 * 0.55

function on_update(id, gen, dt)
    local ex, ey, ez = get_position(id, gen)
    if ex == nil then return end

    local p_id, p_gen = find_by_tag("player")
    if p_id == nil then return end

    local px, py, pz = get_position(p_id, p_gen)
    if px == nil then return end

    local dx, dz = px - ex, pz - ez
    local d_sq = dx * dx + dz * dz
    if d_sq < HIT_DIST_SQ then
        -- Hit the player. Despawn self + leave a damage_marker for game.lua.
        local mid, mgen = spawn_entity()
        set_tag(mid, mgen, "damage_marker")
        destroy_entity(id, gen)
        return
    end

    local len = math.sqrt(d_sq)
    local step = SPEED * dt
    set_position(id, gen, ex + dx / len * step, ey, ez + dz / len * step)
end
