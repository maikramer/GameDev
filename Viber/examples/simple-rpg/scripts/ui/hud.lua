-- HUD v3: os toggles de estado (danger, ready, empty, venom, xp-pop,
-- weather) são TODOS class-binds engine-driven no hud.xml — este script
-- só trata do clique do diário e do soco visual do combo.
local st = viber.state()

function on_update(dt)
  if viber.ui.clicked("open-journal") then
    viber.ui.open("menu", true)
  end

  local combo = viber.ui.get("combo.text")
  if combo ~= st.combo then
    st.combo = combo
    viber.ui.set_anim("combo-text", combo ~= "" and "shake 0.45 0.5" or "none")
  end
end
