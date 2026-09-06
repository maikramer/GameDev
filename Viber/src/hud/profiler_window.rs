//! Janela do profiler (tecla **P**) — o análogo nativo do painel
//! `?profiler=1` do VibeGame, com as mesmas 5 abas:
//!
//! * **Sistemas** — sparkline de frame-times, fps/p95/pior, *headroom* de
//!   orçamento (60/30 fps), barras por grupo, top sistemas e scripts Luau
//!   (média/p95);
//! * **Mundo** — player/câmara/entidades próximas num raio (PageUp/PageDown);
//! * **Física** — corpos/colisores/sensores Rapier, passo do solver;
//! * **Áudio** — buses, layers de música, sinks a tocar;
//! * **Extras** — toggles de debug da engine (colisores, relva, pausar
//!   física).
//!
//! Teclas com a janela aberta: **F5** muda de aba (Shift volta), **F12** ou
//! **Pause** congela a aquisição, **`** exporta o snapshot JSON (o caminho
//! aparece no rodapé). Consome os módulos `profiler::*`; o mesmo snapshot
//! sai pela bridge (`viber debug prof --tab/--export`).
//!
//! Pintura no tema TINTA QUENTE: fundo stone-950 translúcido, texto papel
//! e acentos amber — os mesmos valores que o hud.css usa nos cards.

use bevy::prelude::*;

use super::assets::{HudAssets, label};
use super::widgets::{GraphBar, StatValue, sparkline};

use crate::profiler::{
    self, FrameStats, ProfilerExtras, ProfilerState, TAB_AUDIO, TAB_EXTRAS, TAB_PHYSICS,
    TAB_SYSTEMS, TAB_WORLD, TABS, timed,
};

/// Orçamentos de frame (ms) — os mesmos do VibeGame.
const BUDGET_60: f32 = 1000.0 / 60.0;
const BUDGET_30: f32 = 1000.0 / 30.0;

/// Kinds dos [`StatValue`]/[`StatLabel`] — o mesmo esquema de marcadores do
/// menu. Linhas dinâmicas (grupos/sistemas/scripts) têm label E valor.
mod kinds {
    pub const FPS: usize = 0;
    pub const FRAME: usize = 1;
    pub const P95: usize = 2;
    pub const WORST: usize = 3;
    pub const HEADROOM: usize = 4;
    pub const STATUS: usize = 5;
    /// Barras de grupo: 6 linhas, top por média.
    pub const GROUP_FIRST: usize = 10;
    pub const GROUP_ROWS: usize = 6;
    /// Top sistemas: 8 linhas.
    pub const SYSTEM_FIRST: usize = 20;
    pub const SYSTEM_ROWS: usize = 8;
    /// Scripts Luau: 4 linhas.
    pub const SCRIPT_FIRST: usize = 30;
    pub const SCRIPT_ROWS: usize = 4;
    /// Headlines dos tabs: mundo (player/próximas), física (step), áudio (sinks).
    pub const PLAYER: usize = 40;
    pub const NEARBY: usize = 41;
    pub const STEP: usize = 42;
    pub const SINKS: usize = 43;
    /// Estados dos extras (até 8) + texto de descrições.
    pub const EXTRA_FIRST: usize = 50;
    pub const EXTRA_ROWS: usize = 8;
    pub const EXTRA_DESC: usize = 58;
    /// Corpos de texto multilinha dos tabs.
    pub const TEXT_WORLD: usize = 60;
    pub const TEXT_PHYSICS: usize = 61;
    pub const TEXT_AUDIO: usize = 62;
}

/// Janela raiz (visibilidade alterna com **P**).
#[derive(Component)]
struct ProfilerWindow;

/// Aba clicável da janela (componente próprio — o `TabButton` dos widgets é
/// do menu [Q] e o sistema dele reagiria a cliques daqui).
#[derive(Component)]
struct ProfilerTabButton {
    tab: usize,
}

/// Painel de conteúdo de uma aba (só a activa fica visível).
#[derive(Component)]
struct ProfilerPane {
    tab: usize,
}

/// Botão de toggle no tab Extras.
#[derive(Component)]
struct ProfilerExtraButton {
    id: &'static str,
}

/// Rótulo dinâmico de uma linha (o `StatValue` guarda só o valor).
#[derive(Component)]
struct StatLabel {
    kind: usize,
}

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

/// Linha "rótulo → valor" com rótulo dinâmico opcional. `label_text` vazio
/// → o refrescamento preenche o rótulo (linhas de grupos/sistemas/scripts).
fn stat_row_dyn(
    row: &mut bevy::ecs::hierarchy::ChildSpawner<'_>,
    hud: &HudAssets,
    label_text: &str,
    value_text: &str,
    kind: usize,
    width: f32,
) {
    row.spawn(Node {
        width: Val::Px(width),
        justify_content: JustifyContent::SpaceBetween,
        margin: UiRect::bottom(Val::Px(3.0)),
        ..Default::default()
    })
    .with_children(|line| {
        line.spawn((
            Text::new(label_text.to_string()),
            TextColor(Color::srgba(0.839, 0.827, 0.820, 0.85)), // stone-300
            TextFont {
                font: hud.font.clone().into(),
                font_size: 11.0.into(),
                ..Default::default()
            },
            StatLabel { kind },
        ));
        line.spawn((
            Text::new(value_text.to_string()),
            TextColor(Color::srgb(0.980, 0.980, 0.976)), // papel (stone-50)
            TextFont {
                font: hud.font.clone().into(),
                font_size: 12.0.into(),
                ..Default::default()
            },
            StatValue { kind },
        ));
    });
}

/// Versão com rótulo fixo (o rótulo nunca é reescrito).
fn stat_row_fixed(
    row: &mut bevy::ecs::hierarchy::ChildSpawner<'_>,
    hud: &HudAssets,
    label_text: &str,
    kind: usize,
    width: f32,
) {
    stat_row_dyn(row, hud, label_text, "—", kind, width);
    // O refrescamento só escreve StatValue; garantir que o label fica.
}

/// Corpo de texto multilinha de um tab (as listas: entidades, formas, sinks).
fn body_text(row: &mut bevy::ecs::hierarchy::ChildSpawner<'_>, hud: &HudAssets, kind: usize) {
    row.spawn((
        Text::new(""),
        TextColor(Color::srgba(0.88, 0.86, 0.80, 0.92)),
        TextFont {
            font: hud.font.clone().into(),
            font_size: 11.0.into(),
            ..Default::default()
        },
        StatValue { kind },
    ));
}

/// Constrói a janela (chamado uma vez pelo `HudScreenLayer`).
pub fn build_profiler_window(world: &mut World, hud: &HudAssets) {
    // Clonado antes do spawn mutável (os extras são estáticos em runtime).
    let extras_list: Vec<crate::profiler::ProfilerExtra> = world
        .get_resource::<ProfilerExtras>()
        .map(|e| e.items.clone())
        .unwrap_or_default();
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
                top: Val::Px(64.0),
                right: Val::Px(14.0),
                width: Val::Px(360.0),
                max_height: Val::Percent(88.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                overflow: Overflow::clip_y(),
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
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..Default::default()
                },
                label(hud, "PROFILER", 18.0, Color::srgb(0.839, 0.725, 0.475)),
            ));
            panel.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..Default::default()
                },
                label(
                    hud,
                    "P alterna · F5 aba · F12 congela · ` exporta",
                    10.0,
                    Color::srgba(0.660, 0.640, 0.610, 0.60),
                ),
            ));

            // Abas clicáveis (componente próprio do profiler).
            panel
                .spawn(Node {
                    column_gap: Val::Px(4.0),
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..Default::default()
                })
                .with_children(|tabs| {
                    for (tab, name) in TABS.iter().enumerate() {
                        tabs.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                ..Default::default()
                            },
                            Button,
                            Interaction::default(),
                            BackgroundColor(if tab == TAB_SYSTEMS {
                                Color::srgba(0.9, 0.7, 0.2, 0.85)
                            } else {
                                Color::srgba(0.16, 0.15, 0.13, 0.85)
                            }),
                            ProfilerTabButton { tab },
                        ))
                        .with_children(|btn| {
                            btn.spawn(label(
                                hud,
                                *name,
                                12.0,
                                if tab == TAB_SYSTEMS {
                                    Color::srgb(0.25, 0.17, 0.03)
                                } else {
                                    Color::srgb(0.95, 0.93, 0.85)
                                },
                            ));
                        });
                    }
                });

            // ---- ABA SISTEMAS ------------------------------------------------
            panel
                .spawn((
                    ProfilerPane { tab: TAB_SYSTEMS },
                    Name::new("hud:profiler:systems"),
                ))
                .with_children(|pane| {
                    sparkline(pane, HISTORY_SLOTS, 3.0, 2.0, 44.0);
                    stat_row_fixed(pane, hud, "fps", kinds::FPS, 332.0);
                    stat_row_fixed(pane, hud, "frame (média)", kinds::FRAME, 332.0);
                    stat_row_fixed(pane, hud, "frame p95", kinds::P95, 332.0);
                    stat_row_fixed(pane, hud, "pior fps (janela)", kinds::WORST, 332.0);
                    stat_row_fixed(pane, hud, "margem 60 fps", kinds::HEADROOM, 332.0);
                    for kind in 0..kinds::GROUP_ROWS {
                        stat_row_dyn(pane, hud, "", "", kinds::GROUP_FIRST + kind, 332.0);
                    }
                    for kind in 0..kinds::SYSTEM_ROWS {
                        stat_row_dyn(pane, hud, "", "", kinds::SYSTEM_FIRST + kind, 332.0);
                    }
                    for kind in 0..kinds::SCRIPT_ROWS {
                        stat_row_dyn(pane, hud, "", "", kinds::SCRIPT_FIRST + kind, 332.0);
                    }
                });

            // ---- ABA MUNDO ---------------------------------------------------
            panel
                .spawn((
                    ProfilerPane { tab: TAB_WORLD },
                    Name::new("hud:profiler:world"),
                ))
                .with_children(|pane| {
                    stat_row_fixed(pane, hud, "player", kinds::PLAYER, 332.0);
                    stat_row_fixed(pane, hud, "próximas", kinds::NEARBY, 332.0);
                    body_text(pane, hud, kinds::TEXT_WORLD);
                });

            // ---- ABA FÍSICA --------------------------------------------------
            panel
                .spawn((
                    ProfilerPane { tab: TAB_PHYSICS },
                    Name::new("hud:profiler:physics"),
                ))
                .with_children(|pane| {
                    stat_row_fixed(pane, hud, "step", kinds::STEP, 332.0);
                    body_text(pane, hud, kinds::TEXT_PHYSICS);
                });

            // ---- ABA ÁUDIO ---------------------------------------------------
            panel
                .spawn((
                    ProfilerPane { tab: TAB_AUDIO },
                    Name::new("hud:profiler:audio"),
                ))
                .with_children(|pane| {
                    stat_row_fixed(pane, hud, "sinks", kinds::SINKS, 332.0);
                    body_text(pane, hud, kinds::TEXT_AUDIO);
                });

            // ---- ABA EXTRAS --------------------------------------------------
            panel
                .spawn((
                    ProfilerPane { tab: TAB_EXTRAS },
                    Name::new("hud:profiler:extras"),
                ))
                .with_children(|pane| {
                    // Os extras vêm do resource `ProfilerExtras` (inserido no
                    // build do ProfilerPlugin, antes do Startup do HUD).
                    let extras = &extras_list;
                    if extras.is_empty() {
                        pane.spawn(label(
                            hud,
                            "(sem extras registados)",
                            11.0,
                            Color::srgba(0.66, 0.64, 0.61, 0.7),
                        ));
                    }
                    for (i, extra) in extras.iter().take(kinds::EXTRA_ROWS).enumerate() {
                        pane.spawn(Node {
                            width: Val::Px(332.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..Default::default()
                        })
                        .with_children(|line| {
                            line.spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)),
                                    border_radius: BorderRadius::all(Val::Px(8.0)),
                                    ..Default::default()
                                },
                                Button,
                                Interaction::default(),
                                BackgroundColor(Color::srgba(0.16, 0.15, 0.13, 0.85)),
                                ProfilerExtraButton { id: extra.id },
                            ))
                            .with_children(|btn| {
                                btn.spawn(label(
                                    hud,
                                    extra.label,
                                    12.0,
                                    Color::srgb(0.95, 0.93, 0.85),
                                ));
                            });
                            line.spawn((
                                Text::new("—".to_string()),
                                TextColor(Color::srgb(0.92, 0.94, 0.88)),
                                TextFont {
                                    font: hud.font.clone().into(),
                                    font_size: 12.0.into(),
                                    ..Default::default()
                                },
                                StatValue {
                                    kind: kinds::EXTRA_FIRST + i,
                                },
                            ));
                        });
                    }
                    pane.spawn((
                        Text::new(String::new()),
                        TextColor(Color::srgba(0.66, 0.64, 0.61, 0.75)),
                        TextFont {
                            font: hud.font.clone().into(),
                            font_size: 10.0.into(),
                            ..Default::default()
                        },
                        StatValue {
                            kind: kinds::EXTRA_DESC,
                        },
                    ));
                });

            // Rodapé de estado (export/congelação/raio).
            panel.spawn((
                Node {
                    margin: UiRect::top(Val::Px(8.0)),
                    ..Default::default()
                },
                Text::new(String::new()),
                TextColor(Color::srgba(0.839, 0.725, 0.475, 0.9)),
                TextFont {
                    font: hud.font.clone().into(),
                    font_size: 10.0.into(),
                    ..Default::default()
                },
                StatValue {
                    kind: kinds::STATUS,
                },
            ));
        });
}

/// Toggle **P** + teclas da janela (F5/F12/Pause/Backquote/PageUp/PageDown),
/// cliques nas abas/extras e refrescamento ao vivo (throttle
/// [`REFRESH_SECS`]). Sistema exclusivo: lê o mundo inteiro num passe.
pub fn hud_profiler_window(world: &mut World) {
    let dt_secs = world.resource::<Time>().delta_secs();
    let dt_ms = dt_secs * 1000.0;
    let menus_open = world
        .get_resource::<crate::menus::MenusOpen>()
        .map(|m| m.any())
        .unwrap_or(false);
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let press_p = keys.just_pressed(KeyCode::KeyP);
    let tab_next = keys.just_pressed(KeyCode::F5) && !keys.pressed(KeyCode::ShiftLeft);
    let tab_prev = keys.just_pressed(KeyCode::F5) && keys.pressed(KeyCode::ShiftLeft);
    let freeze = keys.just_pressed(KeyCode::F12) || keys.just_pressed(KeyCode::Pause);
    let export = keys.just_pressed(KeyCode::Backquote);
    let radius_up = keys.just_pressed(KeyCode::PageUp);
    let radius_down = keys.just_pressed(KeyCode::PageDown);

    if press_p && !menus_open {
        if let Some(mut state) = world.get_resource_mut::<ProfilerWindowState>() {
            state.open = !state.open;
            state.refresh = REFRESH_SECS; // refresca já ao abrir/fechar
        }
    }
    if let Some(mut state) = world.get_resource_mut::<ProfilerWindowState>() {
        let head = state.head;
        state.history[head] = dt_ms;
        state.head = (head + 1) % state.history.len();
        state.filled = state.filled.max(state.head);
        state.refresh += dt_secs;
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

    // Teclas só com a janela aberta e sem menus por cima.
    if open && !menus_open {
        if tab_next || tab_prev {
            let dir: isize = if tab_prev { -1 } else { 1 };
            if let Some(mut state) = world.get_resource_mut::<ProfilerState>() {
                state.tab = (state.tab as isize + dir).rem_euclid(TABS.len() as isize) as usize;
            }
            if let Some(mut w) = world.get_resource_mut::<ProfilerWindowState>() {
                w.refresh = REFRESH_SECS;
            }
        }
        if freeze {
            if let Some(mut state) = world.get_resource_mut::<ProfilerState>() {
                state.frozen = !state.frozen;
                state.status = if state.frozen {
                    "congelado (F12 retoma)".into()
                } else {
                    "aquisição retomada".into()
                };
            }
        }
        if export {
            let path = profiler::export_to_file(world, None);
            if let Some(mut state) = world.get_resource_mut::<ProfilerState>() {
                state.status = match path {
                    Ok(p) => format!("exportado → {}", p.display()),
                    Err(e) => format!("export falhou: {e}"),
                };
            }
        }
        if radius_up || radius_down {
            let delta: f32 = if radius_up { 10.0 } else { -10.0 };
            if let Some(mut state) = world.get_resource_mut::<ProfilerState>() {
                state.nearby_radius = (state.nearby_radius + delta).clamp(5.0, 200.0);
                state.status = format!("raio próximas: {:.0} m", state.nearby_radius);
            }
        }
    }

    // Visibilidade da janela.
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

    // Cliques: abas e extras (Interaction::Pressed, mesmo padrão do menu).
    let mut clicked_tab: Option<usize> = None;
    let mut clicked_extra: Option<&'static str> = None;
    {
        let mut q_tabs = world.query::<(&Interaction, &ProfilerTabButton)>();
        for (interaction, tab) in q_tabs.iter(world) {
            if *interaction == Interaction::Pressed {
                clicked_tab = Some(tab.tab);
            }
        }
    }
    {
        let mut q_extras = world.query::<(&Interaction, &ProfilerExtraButton)>();
        for (interaction, extra) in q_extras.iter(world) {
            if *interaction == Interaction::Pressed {
                clicked_extra = Some(extra.id);
            }
        }
    }
    if let Some(tab) = clicked_tab {
        if let Some(mut state) = world.get_resource_mut::<ProfilerState>() {
            state.tab = tab;
        }
        if let Some(mut w) = world.get_resource_mut::<ProfilerWindowState>() {
            w.refresh = REFRESH_SECS;
        }
    }
    if let Some(id) = clicked_extra {
        let label_state = profiler::toggle_extra(world, id);
        if let Some(mut state) = world.get_resource_mut::<ProfilerState>() {
            state.status = format!(
                "{id} → {}",
                label_state
                    .map(|on| if on { "ON" } else { "OFF" })
                    .unwrap_or("?")
            );
        }
        if let Some(mut w) = world.get_resource_mut::<ProfilerWindowState>() {
            w.refresh = REFRESH_SECS;
        }
    }

    // Refrescamento throttle dos textos/barras.
    let should_refresh = world.resource::<ProfilerWindowState>().refresh >= REFRESH_SECS;
    if should_refresh {
        world.resource_mut::<ProfilerWindowState>().refresh = 0.0;
    } else {
        return;
    }

    // Sincroniza cores das abas com o estado (dourada = activa).
    let active_tab = world.resource::<ProfilerState>().tab;
    let mut q_tab_btns = world.query::<(&Interaction, &ProfilerTabButton, &mut BackgroundColor)>();
    for (interaction, tab, mut bg) in q_tab_btns.iter_mut(world) {
        let wanted = BackgroundColor(if active_tab == tab.tab {
            Color::srgba(0.9, 0.7, 0.2, 0.85)
        } else if *interaction == Interaction::Hovered {
            Color::srgba(0.24, 0.22, 0.19, 0.9)
        } else {
            Color::srgba(0.16, 0.15, 0.13, 0.85)
        });
        if bg.0 != wanted.0 {
            *bg = wanted;
        }
    }

    // Painéis visíveis: só a aba activa.
    let mut q_panes = world.query::<(&mut Node, &ProfilerPane)>();
    for (mut node, pane) in q_panes.iter_mut(world) {
        let wanted = if pane.tab == active_tab {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != wanted {
            node.display = wanted;
        }
    }

    // ---- barras do gráfico (mais recente à direita) ------------------------
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

    // ---- dados ---------------------------------------------------------------
    let fps = world
        .get_resource::<bevy::diagnostic::DiagnosticsStore>()
        .and_then(|d| {
            d.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|v| v.smoothed())
        })
        .unwrap_or(0.0);
    let (avg_ms, _min_ms, max_ms, p95_ms) = world
        .get_resource::<FrameStats>()
        .map(|s| s.window())
        .unwrap_or((None, None, None, None));
    let min_fps = world.get_resource::<FrameStats>().and_then(|s| s.min_fps());

    /// Linhas dinâmicas: (rótulos, valores) por kind.
    type DynRows = (Vec<(usize, String)>, Vec<(usize, String)>);
    let (labels, values): DynRows = match active_tab {
        TAB_SYSTEMS => {
            let frame_avg = avg_ms.unwrap_or(0.0);
            let mut labels: Vec<(usize, String)> = Vec::new();
            let mut values: Vec<(usize, String)> = vec![
                (kinds::FPS, format!("{fps:.0}")),
                (
                    kinds::FRAME,
                    avg_ms
                        .map(|v| format!("{v:.1} ms"))
                        .unwrap_or_else(|| "—".into()),
                ),
                (
                    kinds::P95,
                    p95_ms
                        .map(|v| format!("{v:.1} ms"))
                        .unwrap_or_else(|| "—".into()),
                ),
                (
                    kinds::WORST,
                    min_fps
                        .map(|v| format!("{v:.0} (pico {:.1} ms)", max_ms.unwrap_or(0.0)))
                        .unwrap_or_else(|| "—".into()),
                ),
                (
                    kinds::HEADROOM,
                    if frame_avg > 0.0 {
                        let h60 = BUDGET_60 - frame_avg;
                        format!(
                            "{h60:+.1} ms{}  (30fps {:+.1})",
                            if frame_avg > BUDGET_60 { " ✗" } else { "" },
                            BUDGET_30 - frame_avg
                        )
                    } else {
                        "—".into()
                    },
                ),
            ];
            for (i, (group, stats)) in timed::groups_snapshot(frame_avg)
                .iter()
                .filter(|(_, s)| s.samples > 0)
                .take(kinds::GROUP_ROWS)
                .enumerate()
            {
                labels.push((kinds::GROUP_FIRST + i, group.as_str().to_string()));
                values.push((
                    kinds::GROUP_FIRST + i,
                    format!("{:.2} ms  {:.0}%", stats.avg_ms, stats.pct),
                ));
            }
            for (i, s) in timed::systems_snapshot(frame_avg)
                .iter()
                .filter(|s| s.avg_ms >= 0.02)
                .take(kinds::SYSTEM_ROWS)
                .enumerate()
            {
                let hot = if s.avg_ms >= timed::HOT_MS { "! " } else { "" };
                labels.push((kinds::SYSTEM_FIRST + i, format!("{hot}{}", s.name)));
                values.push((
                    kinds::SYSTEM_FIRST + i,
                    format!("{:.2} / {:.2} ms", s.avg_ms, s.p95_ms),
                ));
            }
            for (i, s) in timed::scripts_snapshot(frame_avg)
                .iter()
                .take(kinds::SCRIPT_ROWS)
                .enumerate()
            {
                labels.push((kinds::SCRIPT_FIRST + i, s.name.clone()));
                values.push((kinds::SCRIPT_FIRST + i, format!("{:.2} ms", s.avg_ms)));
            }
            (labels, values)
        }
        TAB_WORLD => {
            let radius = world.resource::<ProfilerState>().nearby_radius;
            let frames = world.resource::<profiler::FrameCounter>().0;
            let snap = profiler::world_tab::snapshot(world, radius, frames);
            let player_line = snap
                .player
                .as_ref()
                .map(|p| format!("{:.1} {:.1} {:.1}", p.pos.x, p.pos.y, p.pos.z))
                .unwrap_or_else(|| "(nenhum)".into());
            let nearby_line = format!(
                "{}/{} ≤{:.0}m",
                snap.nearby.len(),
                snap.nearby_in_radius,
                radius
            );
            (
                vec![],
                vec![
                    (kinds::PLAYER, player_line),
                    (kinds::NEARBY, nearby_line),
                    (
                        kinds::TEXT_WORLD,
                        profiler::world_tab::window_lines(&snap).join("\n"),
                    ),
                ],
            )
        }
        TAB_PHYSICS => {
            let frame_avg = avg_ms.unwrap_or(0.0);
            let snap = profiler::physics_tab::snapshot(world, frame_avg);
            let step_line = snap
                .step
                .map(|s| format!("{:.2} ms (média {:.2})", s.last_ms, s.avg_ms))
                .unwrap_or_else(|| "—".into());
            (
                vec![],
                vec![
                    (kinds::STEP, step_line),
                    (
                        kinds::TEXT_PHYSICS,
                        profiler::physics_tab::window_lines(&snap).join("\n"),
                    ),
                ],
            )
        }
        TAB_AUDIO => {
            let snap = profiler::audio_tab::snapshot(world);
            let sinks_line = format!(
                "{} total · {} a tocar · {} spatial",
                snap.total, snap.playing, snap.spatial
            );
            (
                vec![],
                vec![
                    (kinds::SINKS, sinks_line),
                    (
                        kinds::TEXT_AUDIO,
                        profiler::audio_tab::window_lines(&snap).join("\n"),
                    ),
                ],
            )
        }
        _ => {
            // EXTRAS: estado ON/OFF por botão + descrições no rodapé do tab.
            let extras = world
                .get_resource::<ProfilerExtras>()
                .map(|e| e.items.clone())
                .unwrap_or_default();
            let mut values: Vec<(usize, String)> = extras
                .iter()
                .take(kinds::EXTRA_ROWS)
                .enumerate()
                .map(|(i, extra)| {
                    let on = (extra.is_on)(world);
                    (
                        kinds::EXTRA_FIRST + i,
                        if on {
                            "ON".to_string()
                        } else {
                            "OFF".to_string()
                        },
                    )
                })
                .collect();
            let desc = extras
                .iter()
                .map(|e| format!("{} — {}", e.label, e.description))
                .collect::<Vec<_>>()
                .join("\n");
            values.push((kinds::EXTRA_DESC, desc));
            (vec![], values)
        }
    };

    // Status (todas as abas): aba activa + congelado + status pendente.
    let frozen = world.resource::<ProfilerState>().frozen;
    let status = world.resource::<ProfilerState>().status.clone();
    let status_line = format!(
        "{}{}  {}",
        TABS[active_tab],
        if frozen { " · CONGELADO" } else { "" },
        status
    );
    let mut all_values = values;
    all_values.push((kinds::STATUS, status_line));

    // Aplica textos (labels dinâmicos + valores).
    let mut q_labels = world.query::<(&StatLabel, &mut Text)>();
    for (stat, mut text) in q_labels.iter_mut(world) {
        if let Some((_, value)) = labels.iter().find(|(k, _)| *k == stat.kind) {
            if text.0 != *value {
                text.0 = value.clone();
            }
        }
    }
    let mut q_values = world.query::<(&StatValue, &mut Text)>();
    for (stat, mut text) in q_values.iter_mut(world) {
        if let Some((_, value)) = all_values.iter().find(|(k, _)| *k == stat.kind) {
            if text.0 != *value {
                text.0 = value.clone();
            }
        }
    }
}
