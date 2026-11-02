-- game.lua — wave manager + global score/lives state. Attached to the
-- "game_manager" entity created in scene_builder. ScriptInstancePool keeps
-- a single LuaState for game.lua's path, so module-level locals act as
-- safe shared state.

local wave = 0
local score = 0
local lives = 5
local time = 0
local next_wave_at = 3.0     -- first wave after 3s
local game_over = false

function on_start(id, gen)
    wave, score, lives = 0, 0, 5
    time, next_wave_at, game_over = 0, 3.0, false
    print("[survivor] game start — lives=5, first wave in 3s")
end

function on_update(id, gen, dt)
    if game_over then return end
    time = time + dt

    -- Consume any kill_marker / damage_marker entities dropped by other
    -- scripts. Each .lua has its own LuaState (ScriptInstancePool keys by
    -- path), so this entity-based mailbox is how scripts communicate.
    local kills = iter_by_tag("kill_marker")
    if kills ~= nil then
        for _, k in ipairs(kills) do
            score = score + 1
            destroy_entity(k.id, k.gen)
        end
    end
    local damages = iter_by_tag("damage_marker")
    if damages ~= nil then
        for _, d in ipairs(damages) do
            lives = lives - 1
            destroy_entity(d.id, d.gen)
            print(string.format("[hit] lives=%d", lives))
            if lives <= 0 then
                game_over = true
                print(string.format("[GAME OVER] final score=%d wave=%d", score, wave))
                return
            end
        end
    end

    if time >= next_wave_at then
        wave = wave + 1
        for i = 1, 4 do
            local angle = (i - 1) * (math.pi / 2) + (wave * 0.3)
            local r = 5.0 + (wave * 0.2)
            local ex, ez = math.cos(angle) * r, math.sin(angle) * r
            local eid, egen = spawn_entity()
            set_position(eid, egen, ex, 0.5, ez)
            set_tag(eid, egen, "enemy")
        end
        next_wave_at = time + 5.0
        print(string.format("[wave %d] score=%d lives=%d", wave, score, lives))
    end
end
