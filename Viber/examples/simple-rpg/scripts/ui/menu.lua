-- simple-rpg — comportamento do menu.
--
-- O CONTEÚDO das listas vem da engine (`src/ui/menu_data.rs`) e é montado
-- pelos repetidores `<UiList>`; este script trata das ACÇÕES.
--
-- Os ids dos botões vêm do template (`skill-{id}`, `shop-{id}`), registados
-- pelo repetidor à medida que constrói — por isso `viber.ui.clicked(id)`
-- funciona sem este ficheiro saber quantas linhas existem.
--
-- API usada: `viber.ui.clicked(id)`, `viber.ui.open(id, bool)`,
-- `viber.ui.action(nome, arg)` (learn / buy / sell).

local SKILLS = {
  "vitality1", "strength1", "agility1", "precision1",
  "vitality2", "strength2", "agility2", "precision2",
}

-- Espelha `shop_catalog()` da engine: o que se compra e o que se vende.
local BUY  = { "potion", "antidote", "bomb" }
local SELL = { "wood", "stone" }

function on_update(dt)
  if viber.ui.clicked("menu-close") then
    viber.ui.open("menu", false)
  end

  -- Talentos: a engine recusa sem pontos ou sem requisitos e a lista
  -- repinta-se sozinha no refresh seguinte.
  for _, id in ipairs(SKILLS) do
    if viber.ui.clicked("skill-" .. id) then
      viber.ui.action("learn", id)
    end
  end

  for _, item in ipairs(BUY) do
    if viber.ui.clicked("shop-" .. item) then
      viber.ui.action("buy", item)
    end
  end
  for _, item in ipairs(SELL) do
    if viber.ui.clicked("shop-" .. item) then
      viber.ui.action("sell", item)
    end
  end

  if viber.ui.clicked("act-save") then viber.ui.action("save", "") end
  if viber.ui.clicked("act-load") then viber.ui.action("load", "") end
  if viber.ui.clicked("act-profiler") then
    viber.toast("Profiler: tecla P")
  end
end
