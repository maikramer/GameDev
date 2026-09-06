-- ui/showcase.lua — comportamento da vitrina da UI declarativa.
--
-- Demonstra a metade dinâmica da API: alimentar listas por script
-- (viber.ui.list), ler o estado de widgets (viber.ui.read), reagir
-- a cliques e ligar/desligar animação em runtime (viber.ui.set_anim).

-- Mochila de demonstração: uma fonte de lista que NÃO é da engine —
-- só existe porque o script a alimenta.
local mochila = {
  { name = "Poção", count = 12 },
  { name = "Antídoto", count = 3 },
  { name = "Pedaço de âmbar", count = 1 },
  { name = "Flechas", count = 40 },
}

local missoes = {
  { title = "Falar com o ferreiro", progress_text = "pronta", status = "ready" },
  { title = "Recolher 5 madeiras", progress_text = "3/5", status = "active" },
  { title = "Derrotar o lobo-ancião", progress_text = "0/1", status = "active" },
}

-- O top-level corre uma vez: instala as fontes e o estado inicial.
viber.ui.list("bag-demo", mochila)

-- Nota: `quests` já é fonte da engine (menu_data); reescrevê-la por
-- script sobrepõe-se — só para a vitrina ficar autónoma.
viber.ui.list("quests", missoes)

local function descreve()
  local nome = viber.ui.read("hero-name")
  local vol = viber.ui.read("volume")
  local hud = viber.ui.read("check-hud")
  local anim = viber.ui.read("check-anim")
  return string.format(
    "read() em vivo:\n" ..
    "  hero-name .text = %s\n" ..
    "  volume    .value = %s\n" ..
    "  check-hud .checked = %s\n" ..
    "  check-anim .checked = %s\n" ..
    "  listas: quests=%d, bag-demo=%d",
    nome and nome.text or "—",
    vol and tostring(vol.value) or "—",
    hud and tostring(hud.checked) or "—",
    anim and tostring(anim.checked) or "—",
    viber.ui.list_count("quests"),
    viber.ui.list_count("bag-demo")
  )
end

local st = viber.state() -- estado desta entidade UiRoot

function on_update(dt)
  viber.ui.set_text("readout-text", descreve())

  -- O selo roda só enquanto o check o manda (set_anim em runtime).
  local anim = viber.ui.read("check-anim")
  local a_ligar = anim and anim.checked or false
  if a_ligar ~= st.selo_a_rodar then
    st.selo_a_rodar = a_ligar
    viber.ui.set_anim("seal", a_ligar and "spin 6" or "none")
  end

  -- Cliques nos checks: feedback na consola de QA (log 1× por mudança).
  if viber.ui.clicked("check-sfx") then
    local sfx = viber.ui.read("check-sfx")
    viber.log("check-sfx -> " .. tostring(sfx and sfx.checked))
  end
end
