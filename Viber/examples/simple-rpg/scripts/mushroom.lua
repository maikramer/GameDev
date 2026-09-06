-- mushroom.lua: colheita de cogumelo (+XP).
function on_update(dt)
  local st = viber.state() -- POR ENTIDADE (no top-level era partilhado)
  if not st.ready then
    st.ready = true
    viber.set_interaction("Colher cogumelo", "j", 2.5)
  end
  if viber.interacted("j") and not st.done then
    st.done = true
    viber.add_xp(5)
    viber.toast("Cogumelo colhido (+5 XP)")
    viber.despawn_self() -- colhido: sai do mundo (o prompt desaparece com ele)
  end
end
