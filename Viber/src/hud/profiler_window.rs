//! Janela do profiler (tecla **P**): painel lateral com gráfico de
//! frame-times, estatísticas ao vivo (fps/ms/entidades/scripts/partículas/
//! chunks) e posições de câmera/player. Consome os dados públicos do
//! módulo `profiler` (`FrameStats`, `collect_counters`, `DiagnosticsStore`).
//!
//! Pintura no tema TINTA QUENTE: fundo stone-950 translúcido, texto papel
//! e acentos amber — os mesmos valores que o hud.css usa nos cards.

use bevy::prelude::*;

use super::assets::{HudAssets, label};
use super::widgets::{GraphBar, StatValue, sparkline};

/// Tipos de valor que a janela actualiza (marcadores [`StatValue`]).
mod kinds {
    pub const FPS: usize = 0;
    pub const MS: usize = 1;
    pub const ENTITIES: usize = 2;
    pub const SCRIPTS: usize = 3;
    pub const SCRIPTS_ACTIVE: usize = 4;
    pub const PARTICLES: usize = 5;
    pub const CHUNKS: usize = 6;
    pub const CAM: usize = 7;
    pub const PLAYER: usize = 8;
    pub const SPEED: usize = 10;
    pub const UPTIME: usize = 9;
}

/// Janela raiz (visibilidade alterna com **P**).
#[derive(Component)]
struct ProfilerWindow;

/// Estado da janela: aberta? histórico de frame-times, refresh.
#[derive(Resource)]
struct ProfilerWindowState {
    open: bool,
    history: Vec<f32>,
    head: usize,
    filled: usize,
    refresh: f32,
}

const HISTORY_SLOTS: usize = 60;
const REFRESH_SECS: f32 = 0.2;

/// Linha "rótulo → valor" com a voz do tema (papel sobre tinta, valor em
/// destaque). Mesma métrica do `widgets::stat_row`, pintura stone/amber.
#[allow(clippy::too_many_arguments)]
fn stone_stat_row(
    row: &mut bevy::ecs::hierarchy::ChildSpawner<'_>,
    hud: &HudAssets,
    label_text: &str,
    value_text: &str,
    kind: usize,
    width: f32,
) {
    row.spawn((Node {
        width: Val::Px(width),
        justify_content: JustifyContent::SpaceBetween,
        margin: UiRect::bottom(Val::Px(4.0)),
        ..Default::default()
    },))
    .with_children(|line| {
        line.spawn((
            Text::new(label_text.to_string()),
            TextColor(Color::srgba(0.839, 0.827, 0.820, 0.85)), // stone-300
            TextFont {
                font: hud.font.clone().into(),
                font_size: 12.0.into(),
                ..Default::default()
            },
        ));
        line.spawn((
            Text::new(value_text.to_string()),
            TextColor(Color::srgb(0.980, 0.980, 0.976)), // papel (stone-50)
            TextFont {
                font: hud.font.clone().into(),
                font_size: 13.0.into(),
                ..Default::default()
            },
            StatValue { kind },
        ));
    });
}

/// Constrói a janela (chamado uma vez pelo `HudScreenLayer`).
pub fn build_profiler_window(world: &mut World, hud: &HudAssets) {
    if world.get_resource::<ProfilerWindowState>().is_none() {
        world.insert_resource(ProfilerWindowState {
            open: false,
            history: vec![0.0; HISTORY_SLOTS],
            head: 0,
            filled: 0,
            refresh: 0.0,
        });
    }
    world
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(172.0),
                right: Val::Px(14.0),
                width: Val::Px(300.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..Default::default()
            },
            // stone-950 translúcido + fio dourado da casa (#b7a47770) +
            // sombra quente (#0c0a0988) — a mesma assinatura dos cards.
            BackgroundColor(Color::srgba(0.047, 0.039, 0.035, 0.94)),
            BorderColor::all(Color::srgba(0.718, 0.643, 0.467, 0.44)),
            BoxShadow::new(
                Color::srgba(0.047, 0.039, 0.035, 0.53),
                Val::Px(0.0),
                Val::Px(4.0),
                Val::ZERO,
                Val::Px(10.0),
            ),
            Visibility::Hidden,
            Name::new("hud:profiler"),
            ProfilerWindow,
        ))
        .with_children(|panel| {
            panel.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..Default::default()
                },
                label(hud, "PROFILER", 18.0, Color::srgb(0.839, 0.725, 0.475)), // amber-300 #d6b979
            ));
            panel.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..Default::default()
                },
                label(
                    hud,
                    "P alterna · valores ao vivo",
                    10.0,
                    Color::srgba(0.660, 0.640, 0.610, 0.60), // stone-400 esbatido
                ),
            ));
            sparkline(panel, HISTORY_SLOTS, 3.0, 2.0, 44.0);
            // Estatísticas: (rótulo, kind).
            for (label_text, kind) in [
                ("fps", kinds::FPS),
                ("frame", kinds::MS),
                ("entidades", kinds::ENTITIES),
                ("scripts", kinds::SCRIPTS),
                ("scripts activos", kinds::SCRIPTS_ACTIVE),
                ("partículas (emissores)", kinds::PARTICLES),
                ("terreno (chunks)", kinds::CHUNKS),
                ("câmera", kinds::CAM),
                ("player", kinds::PLAYER),
                ("velocidade", kinds::PLAYER + 1000),
                ("uptime", kinds::UPTIME),
            ] {
                stone_stat_row(panel, hud, label_text, "—", kind, 272.0);
            }
        });
}

/// Toggle **F3**/**P** + atualização ao vivo: histórico de frame-times,
/// barras do gráfico (altura ∝ ms, cor por orçamento) e textos das
/// estatísticas. Sistema exclusivo: lê o mundo inteiro (contadores +
/// câmera + player) e os textos da janela num só passe, a cada
/// [`REFRESH_SECS`]. Sem menus abertos — o P aprende skill no modal.
pub fn hud_profiler_window(world: &mut World) {
    let dt_ms = world.resource::<Time>().delta_secs() * 1000.0;
    let menus_open = world
        .get_resource::<crate::menus::MenusOpen>()
        .map(|m| m.any())
        .unwrap_or(false);
    let toggle = !menus_open
        && (world
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::KeyP)
            || world
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::F3));

    // 1. Amostra o frame e avança o histórico.
    if let Some(mut state) = world.get_resource_mut::<ProfilerWindowState>() {
        if toggle {
            state.open = !state.open;
        }
        let head = state.head;
        state.history[head] = dt_ms;
        state.head = (head + 1) % state.history.len();
        state.filled = state.filled.max(state.head);
        state.refresh += dt_ms / 1000.0;
    }
    let open = world
        .get_resource::<ProfilerWindowState>()
        .map(|s| s.open)
        // Mundos sem <HudScreenLayer> nunca constroem a janela — sem este
        // guard, o world.resource() panicava no 1.º frame (terrain.xml,
        // hello.xml, scaffold do `viber create`).
        .unwrap_or(false);
    if world.get_resource::<ProfilerWindowState>().is_none() {
        return;
    }

    // 2. Visibilidade da janela.
    let mut windows = world.query::<(&mut Visibility, &ProfilerWindow)>();
    for (mut visibility, _) in windows.iter_mut(world) {
        *visibility = if open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !open {
        return;
    }

    // 3. Refresh throttle dos textos/barras.
    let should_refresh = world.resource::<ProfilerWindowState>().refresh >= REFRESH_SECS;
    if should_refresh {
        world.resource_mut::<ProfilerWindowState>().refresh = 0.0;
    } else {
        return;
    }

    // ---- dados ----
    let fps = world
        .get_resource::<bevy::diagnostic::DiagnosticsStore>()
        .and_then(|d| {
            d.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|v| v.smoothed())
        })
        .unwrap_or(0.0);
    let counters = crate::profiler::collect_counters(world);
    let cam_pos = world
        .query_filtered::<&GlobalTransform, With<Camera>>()
        .iter(world)
        .next()
        .map(|t| t.translation())
        .unwrap_or_default();
    let (player_pos, player_speed) = world
        .query_filtered::<(&GlobalTransform, &crate::player::Player), With<crate::player::Player>>()
        .iter(world)
        .next()
        .map(|(t, p)| {
            (
                t.translation(),
                (p.vel_x * p.vel_x + p.vel_z * p.vel_z).sqrt(),
            )
        })
        .unwrap_or((Vec3::ZERO, 0.0));
    let uptime = world.resource::<Time>().elapsed_secs();

    // ---- barras do gráfico (mais recente à direita) ----
    let (history, head) = {
        let s = world.resource::<ProfilerWindowState>();
        (s.history.clone(), s.head)
    };
    let mut bars = world.query::<(&mut Node, &mut BackgroundColor, &GraphBar)>();
    for (mut node, mut color, bar) in bars.iter_mut(world) {
        let slot = (head + bar.slot) % history.len();
        let ms = history[slot];
        let frac = (ms / 33.3).clamp(0.05, 1.0);
        node.height = Val::Px(4.0 + 40.0 * frac);
        *color = BackgroundColor(if ms <= 16.7 {
            Color::srgb(0.36, 0.72, 0.30)
        } else if ms <= 33.3 {
            Color::srgb(0.92, 0.75, 0.2)
        } else {
            Color::srgb(0.9, 0.32, 0.22)
        });
    }

    // ---- textos ----
    let fmt_pos = |p: Vec3| format!("{:.1}, {:.1}, {:.1}", p.x, p.y, p.z);
    let values: Vec<(usize, String)> = vec![
        (kinds::FPS, format!("{fps:.0}")),
        (kinds::MS, format!("{dt_ms:.1} ms")),
        (kinds::ENTITIES, counters.entities.to_string()),
        (kinds::SCRIPTS, counters.scripts_total.to_string()),
        (kinds::SCRIPTS_ACTIVE, counters.scripts_active.to_string()),
        (kinds::PARTICLES, counters.particle_emitters.to_string()),
        (kinds::CHUNKS, counters.terrain_chunks.to_string()),
        (kinds::CAM, fmt_pos(cam_pos)),
        (kinds::PLAYER, fmt_pos(player_pos)),
        (kinds::SPEED, format!("{player_speed:.1} m/s")),
        (kinds::UPTIME, format!("{uptime:.0} s")),
    ];
    let mut stats = world.query::<(&StatValue, &mut Text)>();
    for (stat, mut text) in stats.iter_mut(world) {
        if let Some((_, value)) = values.iter().find(|(k, _)| *k == stat.kind) {
            if text.0 != *value {
                text.0 = value.clone();
            }
        }
    }
}
