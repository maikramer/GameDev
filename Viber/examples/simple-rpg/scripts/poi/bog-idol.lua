-- poi/bog-idol.lua: objeto místico (portado do TS) — prompt quando perto, leitura dá
-- XP uma única vez.
function on_update(dt)
  local st = viber.state() -- POR ENTIDADE (no top-level era partilhado)
  if not st.ready then
    st.ready = true
    viber.set_interaction("Estudar o ídolo", "e", 3.0)
  end
  if viber.interacted("e") and not st.read then
    st.read = true
    viber.add_xp(25)
    viber.toast("O ídolo do pântano observa de volta (+25 XP).")
  end
end
