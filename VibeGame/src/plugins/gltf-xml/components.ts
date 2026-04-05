import { defineComponent, Types } from 'bitecs';

/** 0 = pendente; 1 = carregamento concluído (ou ignorado). */
export const GltfPending = defineComponent({
  loaded: Types.ui8,
});
