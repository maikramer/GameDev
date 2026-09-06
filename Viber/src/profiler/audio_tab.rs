//! Snapshot "Áudio" do profiler — o equivalente compacto do `AudioDebug`
//! do VibeGame: buses do mixer (`AudioMixerSettings`), layers de BGM
//! ([`crate::music::MusicLayerTag`]) e sinks activos (`PlaybackSettings` +
//! `AudioSink`), com estados de pausa/mute/spatial/loop e volume efectivo
//! (bus × layer × master).

use serde::Serialize;

use bevy::audio::{AudioSinkPlayback, PlaybackMode};
use bevy::prelude::*;

use crate::music::{AudioMixerSettings, MusicLayerTag};

#[derive(Debug, Clone, Serialize)]
pub struct BusSnapshot {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerSnapshot {
    pub layer: String,
    pub base_volume: f32,
    pub paused: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SinkSnapshot {
    pub entity: u64,
    pub name: String,
    pub paused: bool,
    pub muted: bool,
    pub spatial: bool,
    pub looping: bool,
    pub volume: f32,
    pub layer: Option<String>,
}

/// Snapshot do tab Áudio.
#[derive(Debug, Clone, Serialize)]
pub struct AudioSnapshot {
    pub buses: BusSnapshot,
    pub layers: Vec<LayerSnapshot>,
    pub sinks: Vec<SinkSnapshot>,
    pub total: usize,
    pub playing: usize,
    pub paused: usize,
    pub muted: usize,
    pub spatial: usize,
    pub looping: usize,
}

/// Volume linear de um `bevy::audio::Volume` (0..=1).
fn volume_linear(volume: &bevy::audio::Volume) -> f32 {
    volume.to_linear()
}

/// Recolhe o snapshot de áudio (5 Hz — iterar sinks é barato).
pub fn snapshot(world: &mut World) -> AudioSnapshot {
    let buses = world
        .get_resource::<AudioMixerSettings>()
        .map(|m| BusSnapshot {
            master: m.master,
            music: m.music,
            sfx: m.sfx,
        })
        .unwrap_or(BusSnapshot {
            master: 1.0,
            music: 1.0,
            sfx: 1.0,
        });

    let mut layers = Vec::new();
    let mut sinks = Vec::new();
    let (mut playing, mut paused, mut muted, mut spatial, mut looping) =
        (0usize, 0usize, 0usize, 0usize, 0usize);

    let mut q = world.query::<(
        Entity,
        &PlaybackSettings,
        Option<&AudioSink>,
        Option<&MusicLayerTag>,
        Option<&Name>,
    )>();
    for (entity, settings, sink, layer, name) in q.iter(world) {
        let is_paused = sink.map(|s| s.is_paused()).unwrap_or(settings.paused);
        let is_muted = sink.map(|s| s.is_muted()).unwrap_or(settings.muted);
        let is_looping = matches!(settings.mode, PlaybackMode::Loop);
        let vol = volume_linear(&settings.volume);

        if let Some(layer) = layer {
            layers.push(LayerSnapshot {
                layer: layer.layer.clone(),
                base_volume: layer.base_volume,
                paused: is_paused,
                muted: is_muted,
            });
        }

        let (is_playing, is_counted_paused, is_counted_muted, is_spatial, is_looping_counted) =
            classify_sink(is_paused, is_muted, settings.spatial, is_looping);
        if is_playing {
            playing += 1;
        }
        if is_counted_paused {
            paused += 1;
        }
        if is_counted_muted {
            muted += 1;
        }
        if is_spatial {
            spatial += 1;
        }
        if is_looping_counted {
            looping += 1;
        }

        sinks.push(SinkSnapshot {
            entity: entity.to_bits(),
            name: name
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("#{}", entity.index())),
            paused: is_paused,
            muted: is_muted,
            spatial: settings.spatial,
            looping: is_looping,
            volume: vol,
            layer: layer.map(|l| l.layer.clone()),
        });
    }
    sinks.sort_by(|a, b| a.name.cmp(&b.name));

    AudioSnapshot {
        buses,
        layers,
        total: sinks.len(),
        playing,
        paused,
        muted,
        spatial,
        looping,
        sinks,
    }
}

/// Classifica um sink: `(playing, paused, muted, spatial, looping)` —
/// "playing" = nem pausado nem muted (como o VibeGame conta active plays).
fn classify_sink(
    is_paused: bool,
    is_muted: bool,
    spatial: bool,
    is_looping: bool,
) -> (bool, bool, bool, bool, bool) {
    (
        !is_paused && !is_muted,
        is_paused,
        is_muted,
        spatial,
        is_looping,
    )
}

/// Linhas de texto da janela para o tab Áudio.
pub fn window_lines(snap: &AudioSnapshot) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "buses  master {:.2}  música {:.2}  sfx {:.2}",
        snap.buses.master, snap.buses.music, snap.buses.sfx
    ));
    lines.push(format!(
        "sinks  {} total · {} a tocar · {} pausados · {} muted · {} spatial · {} loop",
        snap.total, snap.playing, snap.paused, snap.muted, snap.spatial, snap.looping
    ));
    if snap.layers.is_empty() {
        lines.push("música (sem layers activos)".into());
    } else {
        lines.push("música:".into());
        for layer in &snap.layers {
            lines.push(format!(
                "  {} base {:.2}{}{}",
                layer.layer,
                layer.base_volume,
                if layer.paused { " [pausa]" } else { "" },
                if layer.muted { " [mute]" } else { "" },
            ));
        }
    }
    // Sinks a tocar primeiro, depois pausados — teto de 12 linhas.
    let mut sorted: Vec<&SinkSnapshot> = snap.sinks.iter().collect();
    sorted.sort_by_key(|s| (s.paused, s.name.clone()));
    for sink in sorted.iter().take(12) {
        lines.push(format!(
            "  {} {}{}{} v={:.2}{}",
            if sink.paused { "·" } else { "▶" },
            sink.name,
            if sink.spatial { " 3d" } else { "" },
            if sink.looping { " ∞" } else { "" },
            sink.volume,
            sink.layer
                .as_ref()
                .map(|l| format!(" [{l}]"))
                .unwrap_or_default(),
        ));
    }
    if sorted.len() > 12 {
        lines.push(format!("  … +{} mais", sorted.len() - 12));
    }
    lines
}

/// Payload JSON do tab.
pub fn json(snap: &AudioSnapshot) -> serde_json::Value {
    serde_json::to_value(snap).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::audio::{PlaybackMode, PlaybackSettings, Volume};

    fn settings(mode: PlaybackMode, paused: bool, muted: bool, spatial: bool) -> PlaybackSettings {
        PlaybackSettings {
            mode,
            volume: Volume::Linear(0.7),
            speed: 1.0,
            paused,
            muted,
            spatial,
            spatial_scale: None,
            start_position: None,
            duration: None,
        }
    }

    #[test]
    fn test_volume_linear() {
        assert_eq!(volume_linear(&Volume::Linear(0.5)), 0.5);
        assert!((volume_linear(&Volume::Decibels(0.0)) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_classify_sink_playing_vs_paused() {
        assert_eq!(
            classify_sink(false, false, true, true),
            (true, false, false, true, true)
        );
        assert_eq!(
            classify_sink(true, false, false, false),
            (false, true, false, false, false)
        );
        // muted não conta como playing
        assert_eq!(
            classify_sink(false, true, false, false),
            (false, false, true, false, false)
        );
    }

    #[test]
    fn test_snapshot_from_world() {
        let mut world = World::new();
        world.init_resource::<AudioMixerSettings>();
        world.spawn((settings(PlaybackMode::Loop, false, false, false),));
        world.spawn((settings(PlaybackMode::Once, true, false, true),));

        let snap = snapshot(&mut world);
        assert_eq!(snap.total, 2);
        assert_eq!(snap.playing, 1, "o pausado não conta como a tocar");
        assert_eq!(snap.paused, 1);
        assert_eq!(snap.spatial, 1);
        assert_eq!(snap.looping, 1);
        assert!(snap.layers.is_empty());
        assert!(snap.buses.master > 0.0);

        let lines = window_lines(&snap);
        assert!(lines.iter().any(|l| l.contains("2 total")), "{lines:?}");
        let json = json(&snap);
        assert_eq!(json["total"], 2, "{json}");
    }
}
