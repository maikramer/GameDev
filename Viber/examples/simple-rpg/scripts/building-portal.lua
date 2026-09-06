-- building-portal.lua: portas bidireccionais. Os exits interiores (interiors.xml,
-- x > 500) teleportam de volta para a porta exterior CORRESPONDENTE na vila; as
-- portas da vila (portals.xml, x < 500) são a ENTRADA e teleportam para o
-- interior da sala. v3: tabela exata de saídas por posição (o `x % 100` antigo
-- quebrava com x negativo e quase tudo caía na casa comprida); estado POR
-- ENTIDADE — no top-level os 20 portais partilhavam o cooldown.
-- v4: tabela inversa ENTRIES — [E] na porta da vila entra na sala; spawn ~5 m
-- para dentro (+Z) do vão −Z, ao centro em x (interiors.xml: "Spawn no CENTRO;
-- exit no vão −Z"), longe de paredes e mobília.
local EXITS = {
  -- { interior_x, interior_z, vila_x, vila_z, spawn_x, spawn_z }
  { 797, 218.9,  26.35,   8.44, 797, 223.9 },  -- casa comum (house_a)
  { 857, 217.9,   7.46,  22.46, 857, 222.9 },  -- capela
  { 917, 218.9, -30.47, -29.06, 917, 223.9 },  -- ferraria
  { 797, 273.9, -17.44,  22.47, 797, 278.9 },  -- casa b
  { 857, 273.9, -20.47, -18.44, 857, 278.9 },  -- casa c (cozinha)
  { 917, 273.9, -22.33,  12.95, 917, 278.9 },  -- cabana do pastor
  { 797, 326.9, -26.11,  30.00, 797, 331.9 },  -- celeiro
  { 862, 326.9,  35.53, -37.53, 862, 331.9 },  -- longhouse
  { 927, 329.9,  10.10, -15.70, 927, 334.9 },  -- banca do mercado (stall a)
}

-- ENTRIES: a tabela inversa (porta da vila → interior) deriva dos pares
-- EXITS; as bancas b/c partilham a sala da banca a (interiors.xml).
local ENTRIES = {}
for _, e in ipairs(EXITS) do
  ENTRIES[#ENTRIES + 1] = { e[3], e[4], e[5], e[6] }
end
ENTRIES[#ENTRIES + 1] = { 11.70, -14.40, 927, 334.9 }  -- portal.market_stall_b → mesma banca
ENTRIES[#ENTRIES + 1] = { 18.90, -20.30, 927, 334.9 }  -- portal.market_stall_c → mesma banca

function on_update(dt)
  local st = viber.state() -- POR ENTIDADE (20 portais partilhavam st.cd)
  local x, y, z = viber.position()
  if not st.ready then
    st.ready = true
    st.inside = x > 500 -- exits interiores "Sair"; portas da vila "Entrar"
    if st.inside then
      viber.set_interaction("Sair", "e", 2.5)
    else
      viber.set_interaction("Entrar", "e", 2.5)
    end
  end
  if not viber.interacted("e") then
    st.cd = false
    return
  end
  if st.cd then return end
  st.cd = true
  if st.inside then
    -- exit interior → porta da vila correspondente
    local best, bd = nil, math.huge
    for _, e in ipairs(EXITS) do
      local dd = (e[1] - x) ^ 2 + (e[2] - z) ^ 2
      if dd < bd then bd, best = dd, e end
    end
    viber.sound("door_close") -- a porta fecha-se atrás
    viber.teleport_player(best[3], (y or 25) + 0.2, best[4])
    viber.toast("A porta te devolve à vila.")
  else
    -- porta da vila → interior da sala
    local best, bd = nil, math.huge
    for _, e in ipairs(ENTRIES) do
      local dd = (e[1] - x) ^ 2 + (e[2] - z) ^ 2
      if dd < bd then bd, best = dd, e end
    end
    viber.sound("door_open")
    viber.teleport_player(best[3], (y or 25) + 0.2, best[4])
    viber.toast("Entras no edifício.")
  end
end
