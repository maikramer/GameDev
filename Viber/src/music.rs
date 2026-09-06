//! BGM driver: loops every `<MusicLayer>` at volume 0 and crossfades the
//! layer matching the player's zone (ported from the example's `bgmZone`).

use bevy::audio::Volume;
use bevy::math::Vec2;
use bevy::prelude::*;

/// Bus volumes from `<AudioMixer master music sfx>`.
///
/// The resource always exists (worlds without an `<AudioMixer>` get this
/// default): a derived `Default` would zero every bus — i.e. a silent game —
/// and any system taking `ResMut<AudioMixerSettings>` used to panic outright
/// on a world that never declared the tag.
#[derive(Debug, Clone, Resource)]
pub struct AudioMixerSettings {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
}

impl Default for AudioMixerSettings {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 1.0,
            sfx: 1.0,
        }
    }
}

/// One playing BGM layer.
#[derive(Debug, Component)]
pub struct MusicLayerTag {
    pub layer: String,
    pub base_volume: f32,
}

/// Player context marker for the driver (reuses the `Player` component).
pub fn bgm_zone(x: f32, z: f32) -> &'static str {
    // Interiores remotos (caixa dos interiores, com margem)
    if (770.0..950.0).contains(&x) && (205.0..355.0).contains(&z) {
        return "dungeon";
    }
    // Cunha dos Picos Gelados: z <= -240, |x| abre 240 → 1040
    if z <= -240.0 && x.abs() <= 240.0 + 0.0f32.max(-z - 240.0) {
        return "mountain";
    }
    // Vila murada na origem (SpawnExclusion r52 + margem)
    if x * x + z * z < 55.0 * 55.0 {
        return "village";
    }
    "explore"
}

/// Segundos que a layer de combate sobrevive ao último evento de combate —
/// tiros trocados mantêm o timer vivo; afastar-se sem luta deixa voltar a
/// música da zona.
pub const COMBAT_MUSIC_HOLD: f32 = 8.0;

/// Estado da música de combate: enquanto ativo, a layer `battle` (ou `boss`
/// quando o alvo o é) ganha à zona do player. Escrito pelos gatilhos de
/// combate (aggro da IA, golpes); expira sozinho após
/// [`COMBAT_MUSIC_HOLD`] s sem eventos.
///
/// O recurso é inicializado no [`crate::ai::AiPlugin`] (que também o
/// escreve) — worlds sem IA ficam com o crossfade puro por zona.
#[derive(Debug, Clone, Resource, Default)]
pub struct CombatMusicState {
    /// Instante (`Time.elapsed_secs_f64()`) até ao qual o combate "soa".
    until: f64,
    /// Alvo atual é um boss → layer `boss` em vez de `battle`.
    boss: bool,
}

impl CombatMusicState {
    /// (Re)acende a música de combate; `boss` promove para a layer de boss
    /// (nunca desce de boss para battle enquanto o timer viver).
    pub fn engage(&mut self, now: f64, boss: bool) {
        self.until = (now + COMBAT_MUSIC_HOLD as f64).max(self.until);
        if boss {
            self.boss = true;
        }
    }

    /// Layer de combate ativa agora, expirando o timer (`boss` > `battle`).
    pub fn active_layer(&mut self, now: f64) -> Option<&'static str> {
        if now >= self.until {
            self.boss = false;
            None
        } else if self.boss {
            Some("boss")
        } else {
            Some("battle")
        }
    }
}

/// Target linear volume of one layer given the active zone.
pub fn layer_target(layer: &str, zone: &str, base_volume: f32, music: f32, master: f32) -> f32 {
    if layer == zone {
        base_volume * music * master
    } else {
        0.0
    }
}

/// Crossfade one step: move `current` toward `target` by `speed` per second.
pub fn fade_step(current: f32, target: f32, dt: f32, speed: f32) -> f32 {
    // NaN/inf não morrem no clamp: devolver o `current` evita que
    // `next != current` seja sempre verdadeiro (reescrita de Volume todos
    // os frames para sempre).
    if !current.is_finite() || !target.is_finite() {
        return current;
    }
    let delta = target - current;
    let max_step = speed * dt;
    if delta.abs() <= max_step {
        target
    } else {
        current + delta.signum() * max_step
    }
}

/// Move each layer's volume toward its zone target every frame.
pub fn music_driver(
    time: Res<Time>,
    players: Query<&GlobalTransform, With<crate::player::Player>>,
    mixer: Option<Res<AudioMixerSettings>>,
    mut combat: ResMut<CombatMusicState>,
    mut layers: Query<(&MusicLayerTag, &mut PlaybackSettings)>,
) {
    // `iter().next()` e não `single()`: mundos com 0 ou ≥2 players (o
    // auto-orbit suporta mundo sem player) ficavam para sempre sem BGM.
    let Some(player) = players.iter().next() else {
        return;
    };
    let pos = player.translation();
    // Combate ativo ganha à zona (boss > battle); sem luta, música do sítio.
    let zone = combat
        .active_layer(time.elapsed_secs_f64())
        .unwrap_or_else(|| bgm_zone(pos.x, pos.z));
    let (master, music) = mixer.map(|m| (m.master, m.music)).unwrap_or((1.0, 1.0));
    let dt = time.delta_secs().clamp(0.0, 0.2);
    for (tag, mut settings) in &mut layers {
        let target = layer_target(&tag.layer, zone, tag.base_volume, music, master);
        let current = settings.volume.to_linear();
        let next = fade_step(current, target, dt, 0.6);
        if next != current {
            settings.volume = Volume::Linear(next);
        }
    }
}

/// Unused import guard — `Vec2` kept for future positional (2D) BGM filters.
#[allow(dead_code)]
type _Unused = Vec2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgm_zone_village_at_origin() {
        assert_eq!(bgm_zone(0.0, 0.0), "village");
        assert_eq!(bgm_zone(40.0, 30.0), "village");
    }

    #[test]
    fn test_bgm_zone_dungeon_box() {
        assert_eq!(bgm_zone(850.0, 280.0), "dungeon");
        assert_eq!(bgm_zone(775.0, 210.0), "dungeon");
        // fora da caixa
        assert_eq!(bgm_zone(760.0, 280.0), "explore");
    }

    #[test]
    fn test_bgm_zone_mountain_wedge() {
        assert_eq!(bgm_zone(0.0, -300.0), "mountain");
        assert_eq!(bgm_zone(200.0, -400.0), "mountain");
        // wedge fecha a sul: em z=-250 só |x| <= 240+10
        assert_eq!(bgm_zone(600.0, -250.0), "explore");
    }

    #[test]
    fn test_bgm_zone_explore_default() {
        assert_eq!(bgm_zone(300.0, 300.0), "explore");
    }

    #[test]
    fn test_layer_target_crossfades() {
        assert!(
            layer_target("explore", "explore", 0.18, 0.7, 1.0) > 0.1,
            "active layer gets volume"
        );
        assert_eq!(
            layer_target("boss", "explore", 0.24, 0.7, 1.0),
            0.0,
            "inactive layer mutes"
        );
    }

    #[test]
    fn test_fade_step_converges() {
        assert_eq!(fade_step(0.5, 0.5, 0.1, 0.6), 0.5);
        assert!(fade_step(0.0, 1.0, 0.1, 0.6) > 0.0);
        assert_eq!(fade_step(0.0, 1.0, 5.0, 0.6), 1.0, "reaches target");
        // NaN não se propaga: devolve o current (o driver não reescreve).
        assert!(fade_step(f32::NAN, 1.0, 0.1, 0.6).is_nan());
        assert_eq!(fade_step(0.25, f32::NAN, 0.1, 0.6), 0.25);
    }

    #[test]
    fn test_combat_music_engage_and_expiry() {
        let mut combat = CombatMusicState::default();
        assert_eq!(combat.active_layer(0.0), None, "sem combate não há layer");

        combat.engage(10.0, false);
        assert_eq!(combat.active_layer(10.0), Some("battle"));
        assert_eq!(combat.active_layer(10.0 + COMBAT_MUSIC_HOLD as f64 - 0.5), Some("battle"));
        assert_eq!(
            combat.active_layer(10.0 + COMBAT_MUSIC_HOLD as f64 + 0.5),
            None,
            "expira sozinho após o hold"
        );

        // Refrescos prolongam; boss promove e não desce a battle.
        combat.engage(20.0, false);
        combat.engage(22.0, true);
        assert_eq!(combat.active_layer(27.0), Some("boss"), "boss > battle");
        // 22 + hold = 30; a 31 já ninguém renovou.
        assert_eq!(combat.active_layer(31.0), None);
    }

    #[test]
    fn test_combat_music_beats_zone_in_driver_target() {
        // layer_target recebe a zona resolvida — com combate ativo a layer
        // "battle" tem de ser a ativa mesmo em pé na vila.
        assert!(layer_target("battle", "battle", 0.24, 0.7, 1.0) > 0.1);
        assert_eq!(layer_target("village", "battle", 0.2, 0.7, 1.0), 0.0);
    }
}
