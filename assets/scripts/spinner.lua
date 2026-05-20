-- spinner.lua — rotate the host entity around its local Y axis.
-- One LuaState backs every entity that points at this file, so do not store
-- per-entity state in globals; rely on the (id, gen) arguments instead.

function on_start(id, gen)
    -- Nothing to initialise — `on_update` reads the entity's current
    -- rotation directly.
end

function on_update(id, gen, dt)
    local rx, ry, rz = get_rotation(id, gen)
    if ry == nil then return end
    set_rotation(id, gen, rx, ry + dt, rz)
end
