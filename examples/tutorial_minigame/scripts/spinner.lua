-- spinner.lua — Chapter 3 결과물. ScriptComponent 로 entity 에 부착하면
-- 매 프레임 Y 축으로 회전한다.

function on_start(id, gen) end

function on_update(id, gen, dt)
    local rx, ry, rz = get_rotation(id, gen)
    if ry == nil then return end
    set_rotation(id, gen, rx, ry + dt, rz)
end
