-- well.lua: prompt de interação + efeito (portado do TS; colheita pesada chega
-- com o Destructible/Inventory da Fase 3).
function on_update(dt)
  local st = viber.state() -- POR ENTIDADE (no top-level era partilhado)
  if not st.ready then
    st.ready = true
    viber.set_interaction("Beber", "e", 3.0)
  end
  if viber.interacted("e") and not st.done then
    st.done = true
    viber.toast("A água fresca restaura o ânimo.")
  end
end
