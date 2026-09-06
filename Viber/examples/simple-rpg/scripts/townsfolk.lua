-- townsfolk.lua: moradores passeiam perto de casa; perto do player param,
-- viram-se e gesticulam de vez em quando; [E] troca banter.
local SPEED = 1.1
local RADIUS = 4
local GESTOS = { "wave", "talk", "yes", "no" }
local BANTER = {
  "“Bom dia para uma caminhada, não achas?”",
  "“Dizem que há ferro novo no mercado.”",
  "“Cuidado com os lobos a norte.”",
}

function on_update(dt)
  local st = viber.state() -- POR ENTIDADE (3 townsfolk partilham globals)
  -- Interação por entidade: o top-level corre 1× por path, não por NPC.
  if not st.ready then
    st.ready = true
    st.gesture_t = 4 + math.random() * 4 -- 1.º gesto cedo
    viber.set_interaction("Falar", "e", 3.5)
  end
  local has, px, py, pz = viber.player_position()
  if not has then return end
  local x, y, z = viber.position()
  local dist = math.sqrt((px - x)^2 + (pz - z)^2)
  if dist < 4.5 then
    viber.face_player()
    st.gesture_t = st.gesture_t - dt
    if st.gesture_t <= 0 then
      st.gesture_t = 8 + math.random() * 8 -- 8–16 s
      viber.gesture(GESTOS[math.random(#GESTOS)])
    end
    if viber.interacted("e") then
      st.i = ((st.i or 0) % #BANTER) + 1
      viber.toast(BANTER[st.i])
      viber.gesture("talk")
    end
    return
  end
  if st.target == nil then
    local tx, tz = viber.wander_target(RADIUS)
    st.target = { tx, tz }
  end
  local td = math.sqrt((st.target[1] - x)^2 + (st.target[2] - z)^2)
  if td < 0.6 then
    st.target = nil
    st.stuck, st.last_td = 0, nil -- chegou: limpa o anti-stuck
  else
    -- anti-stuck: td sem diminuir = preso num collider; novo destino após ~6 s
    if st.last_td ~= nil and td >= st.last_td then
      st.stuck = (st.stuck or 0) + dt
      if st.stuck > 6 then
        local tx, tz = viber.wander_target(RADIUS)
        st.target = { tx, tz }
        st.stuck, st.last_td = 0, nil
      end
    else
      st.stuck = 0
    end
    st.last_td = td
    viber.move_towards(st.target[1], st.target[2], SPEED)
  end
end
