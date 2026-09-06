-- bosses/witch.lua: comportamento portado do TS (a engine provê os blocos: percepção,
-- movimento com snap no terreno, máquina wander/chase, dano).
local SPEED_WANDER, SPEED_CHASE = 1.4, 3.6
local AGGRO, DEAGGRO, ATTACK_RANGE = 13, 18, 2.4
local DAMAGE, COOLDOWN = 18, 1.2
local WANDER_RADIUS = 10

local function pick_target(st)
  local tx, tz = viber.wander_target(WANDER_RADIUS)
  st.target = { tx, tz }
end

function on_update(dt)
  local st = viber.state() -- POR ENTIDADE: no top-level partilhava entre instâncias
  local has, px, py, pz = viber.player_position()
  if not has then return end
  local x, y, z = viber.position()
  local dist = math.sqrt((px - x)^2 + (pz - z)^2)
  st.state = viber.next_state(st.state or "wander", dist, AGGRO, DEAGGRO)

  -- Roar na TRANSIÇÃO wander → chase (não repete por tick)
  if st.state == "chase" and st.pstate ~= "chase" then
    viber.sound("roar")
  end
  st.pstate = st.state

  if st.state == "chase" then
    if dist > ATTACK_RANGE then
      viber.move_towards(px, pz, SPEED_CHASE)
      st.t = 0
    else
      viber.face_player()
      st.t = (st.t or 0) + dt
      if st.t >= COOLDOWN then
        st.t = 0
        viber.damage_player(DAMAGE)
      end
    end
  else
    if st.target == nil then pick_target(st) end
    local td = math.sqrt((st.target[1] - x)^2 + (st.target[2] - z)^2)
    if td < 0.8 then
      st.target = nil
      st.stuck, st.last_td = 0, nil -- chegou: limpa o anti-stuck
    else
      -- anti-stuck: td sem diminuir = preso num collider; repick após ~6 s
      if st.last_td ~= nil and td >= st.last_td then
        st.stuck = (st.stuck or 0) + dt
        if st.stuck > 6 then
          pick_target(st)
          st.stuck, st.last_td = 0, nil
        end
      else
        st.stuck = 0
      end
      st.last_td = td
      viber.move_towards(st.target[1], st.target[2], SPEED_WANDER)
    end
  end
end
