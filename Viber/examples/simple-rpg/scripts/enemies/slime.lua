-- slime.lua: comportamento portado do TS (a engine provê os blocos: percepção,
-- movimento com snap no terreno, máquina wander/chase, dano).
local SPEED_WANDER, SPEED_CHASE = 0.7, 2.0
local AGGRO, DEAGGRO, ATTACK_RANGE = 6, 11, 1.4
local DAMAGE, COOLDOWN = 6, 2.0
local WANDER_RADIUS = 3

local function pick_target(st)
  local tx, tz = viber.wander_target(WANDER_RADIUS)
  st.target = { tx, tz }
end

function on_update(dt)
  local st = viber.state() -- POR ENTIDADE: no top-level partilhava entre instâncias
  if not st.logged then
    st.logged = true
    viber.log("slime ativo: FSM wander/chase rodando")
  end
  local has, px, py, pz = viber.player_position()
  if not has then return end
  local x, y, z = viber.position()
  local dist = math.sqrt((px - x)^2 + (pz - z)^2)
  st.state = viber.next_state(st.state or "wander", dist, AGGRO, DEAGGRO)

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
    else
      viber.move_towards(st.target[1], st.target[2], SPEED_WANDER)
    end
  end
end
