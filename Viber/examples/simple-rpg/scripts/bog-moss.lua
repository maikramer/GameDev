-- bog-moss.lua: tufo de musgo-do-pântano colhível ([J]) — fonte do
-- item `bog-moss` (objetivo collect ×10 da quest do pântano).
function on_update(dt)
  local st = viber.state() -- POR ENTIDADE
  if not st.ready then
    st.ready = true
    viber.set_interaction("Colher musgo", "j", 2.5)
  end
  if viber.interacted("j") and not st.done then
    st.done = true
    viber.add_xp(8)
    viber.item_add("bog-moss", 1)
    viber.toast("Musgo-do-pântano colhido (+8 XP, +1 bog-moss)")
    viber.topple()
  end
end
