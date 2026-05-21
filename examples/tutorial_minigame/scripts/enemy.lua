-- enemy.lua — Chapter 4 결과물. player 위치를 찾아 매 프레임 0.5 unit/sec
-- 속도로 끌어당긴다. ScriptInstancePool 이 path 당 1 LuaState 공유 이므로
-- 여러 enemy entity 가 같은 인터프리터를 쓴다.

function on_update(id, gen, dt)
    local ex, ey, ez = get_position(id, gen)
    if ex == nil then return end

    local p_id, p_gen = find_by_tag("player")
    if p_id == nil then return end

    local px, py, pz = get_position(p_id, p_gen)
    if px == nil then return end

    local dx = px - ex
    local dz = pz - ez
    local len = math.sqrt(dx * dx + dz * dz)
    if len < 0.05 then return end

    local speed = 0.5 * dt
    set_position(id, gen, ex + dx / len * speed, ey, ez + dz / len * speed)
end
