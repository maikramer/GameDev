//! day_tint para os materiais standard dos GltfScene — copas, casas, props
//! e personagens que os sistemas da relva/splat não alcançam (peça
//! exploração, r10 do gauntlet BOTW; mecanismo reportado e autorizado pela
//! lead).
//!
//! **Porquê**: `grass_daynight_tint` e `terrain_daynight_tint` escurecem os
//! MATERIAIS partilhados da relva e do splat ao anoitecer; os materiais dos
//! glTFs carregados por `<GltfScene>` não recebem passada nenhuma — ficam
//! com o albedo de dia sob o luar azul e leem-se como "recortes colados"
//! contra a relva em silhueta.
//!
//! **Como**: mesma curva `day_tint` da relva multiplicada no `base_color`
//! ORIGINAL de cada `StandardMaterial` (capturado à primeira vista e guardado,
//! para o tint nunca compor). Ficam de fora materiais `unlit` (marcadores) e
//! `emissive` ≠ preto (vidros de janela/lanterna têm de continuar acesos —
//! as pools quentes são as luzes de ponto, não o albedo).
//!
//! Seguro para materiais partilhados: a chave é o `AssetId` do material
//! (por ficheiro GLB — todas as instâncias da mesma copa partilham o tint,
//! como no VibeGame).

use std::collections::HashMap;

use bevy::asset::AssetId;
use bevy::color::LinearRgba;
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;

use crate::grass::day_tint;

/// Originais de albedo capturados por material + último tint aplicado.
#[derive(Resource, Default)]
struct PropTintState {
    originals: HashMap<AssetId<StandardMaterial>, LinearRgba>,
    last_tint: Option<[f32; 3]>,
    throttle: f32,
}

pub struct PropTintPlugin;

impl Plugin for PropTintPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PropTintState>()
            .add_systems(Update, prop_daynight_tint);
    }
}

/// Aplica a curva de noite da relva a todos os `StandardMaterial` (glTFs e
/// primitivas) que não sejam `unlit`/emissivos. Throttle 0,25 s + early-out
/// quando o factor de dia não mudou — o custo parado é uma subtração.
fn prop_daynight_tint(
    mut state: ResMut<PropTintState>,
    time: Res<Time>,
    clock: Option<Res<crate::worldsys::DayCycleState>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    state.throttle -= time.delta_secs();
    if state.throttle > 0.0 {
        return;
    }
    state.throttle = 0.25;

    let day = clock
        .as_deref()
        .map(|clock| {
            crate::worldsys::daylight_factor(
                clock.minute_of_day,
                clock.dawn_minute,
                clock.dusk_minute,
            )
        })
        .unwrap_or(1.0);
    let tint = day_tint(day);
    if let Some(last) = state.last_tint {
        if (tint[0] - last[0]).abs() < 1e-3
            && (tint[1] - last[1]).abs() < 1e-3
            && (tint[2] - last[2]).abs() < 1e-3
        {
            return;
        }
    }

    // ids são Copy — o Vec termina o borrow imutável antes do get_mut.
    let ids: Vec<_> = materials.ids().collect();
    for id in ids {
        let Some(mut material) = materials.get_mut(id) else {
            continue;
        };
        // Auto-iluminados ficam de fora: marcadores e vidros acesos.
        if material.unlit || material.emissive != LinearRgba::BLACK {
            continue;
        }
        let original = *state
            .originals
            .entry(id)
            .or_insert_with(|| material.base_color.to_linear());
        material.base_color = Color::linear_rgb(
            original.red * tint[0],
            original.green * tint[1],
            original.blue * tint[2],
        );
    }
    state.last_tint = Some(tint);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_at_noon_is_identity() {
        let noon = day_tint(1.0);
        assert!(noon.iter().all(|c| (c - 1.0).abs() < 1e-4));
    }

    #[test]
    fn tint_at_night_darkens_below_half() {
        let night = day_tint(0.0);
        assert!(night.iter().all(|c| *c < 0.5), "{night:?}");
    }
}
