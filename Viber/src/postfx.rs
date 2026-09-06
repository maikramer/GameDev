//! Pós-processamento da câmara: exposição, bloom e ambient occlusion.
//!
//! A câmara era um `Camera3d::default()` puro — sem bloom, sem AO, sem
//! exposição autoral —, pelo que a cena lia achatada e lavada: o sol a
//! 10 000 lux com a exposição por omissão do Bevy nunca deixa nada saturar e
//! nada brilha. Os XMLs do `simple-rpg` já pedem tudo isto
//! (`pp-exposure`, `pp-bloom-strength` por `<BiomeRegion>`); o migrador
//! listou-os como *dropped attrs*.
//!
//! O que este módulo liga, por ordem de impacto visual:
//!
//! 1. **[`Exposure`]** — a exposição fotográfica da câmara. `EV100_SUNLIGHT`
//!    seria correcto para 100 000 lux reais; os mundos autoram o sol a
//!    ~10 000 lux, logo a base fica entre o interior e o dia claro e os
//!    biomas ajustam-na (`pp-exposure`, onde 1.0 = base).
//! 2. **[`Bloom`]** — só o que já está acima do branco floresce (fogueiras,
//!    materiais emissivos, o disco solar do domo). Preset `NATURAL`, que é
//!    energy-conserving: não injecta luz nova na imagem.
//! 3. **[`ScreenSpaceAmbientOcclusion`]** — o contacto entre objectos e chão.
//!    É o que dá "assentado" às árvores, às bancas e ao herói; sem ele tudo
//!    parece autocolante sobre o terreno. Exige os prepasses de profundidade
//!    e normal, que o próprio componente declara via `#[require]`.
//!
//! `VIBER_NO_POSTFX=1` desliga tudo (comparações A/B e GPUs fracas).

use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::Exposure;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel};
use bevy::post_process::bloom::{Bloom, BloomPrefilter};
use bevy::prelude::*;
use bevy::render::view::Msaa;

use crate::ambient::point_in_polygon;
use crate::player::Player;
use crate::worldsys::BiomeRegions;

/// Exposição base (EV100) — o mesmo default do Bevy (`EV100_BLENDER`).
///
/// Mantê-lo aqui é deliberado: os mundos autoram o sol a ~10 000 lux (e não
/// aos ~100 000 do `EV100_SUNLIGHT`), pelo que 9.7 é a exposição a que a cena
/// já foi iluminada. O ganho de contraste vem do AO, do bloom e das sombras,
/// não de escurecer a imagem por baixo do que o autor viu; quem quiser
/// escurecer usa o `pp-exposure` do bioma (o `simple-rpg` pede 0.70–0.78).
pub const BASE_EV100: f32 = 9.7;
/// Intensidade base do bloom (o preset `NATURAL` do Bevy usa 0.15).
pub const BASE_BLOOM: f32 = 0.12;
/// Teto do bloom — o clamp do bioma e o punch de impacto partilham-no.
pub const MAX_BLOOM: f32 = 0.5;
/// Piso do escurecimento por punch (EV) — o "ai" do dano recebido não
/// transforma o ecrã em breu.
pub const MAX_DARKEN_EV: f32 = 2.5;
/// Velocidade do crossfade de exposição/bloom ao mudar de bioma (por segundo).
const BLEND_RATE: f32 = 1.6;

/// Constante de tempo do decay de um kick de exposição (s). ~3× isto lê-se
/// como "um flash que dura 1 s" — o kick do level-up vive neste regime.
pub const KICK_TAU: f32 = 0.32;
/// Teto do kick acumulado (EV) — dois level-ups seguidos não estouram a imagem.
pub const MAX_KICK_EV: f32 = 1.2;
/// Abaixo deste valor o kick corta a zero (fim determinístico do decay).
const KICK_EPS: f32 = 1e-3;

/// Alvos de pós-processamento em vigor (base do mundo × bioma atual).
#[derive(Debug, Clone, Resource)]
pub struct PostFxState {
    /// EV100 alvo, já com o multiplicador do bioma aplicado.
    pub target_ev100: f32,
    /// Intensidade de bloom alvo.
    pub target_bloom: f32,
    /// Valores correntes (interpolados na direção dos alvos).
    pub ev100: f32,
    pub bloom: f32,
    /// Kick de juice em curso (EV, ≥0 — positivo CLAREIA a imagem). Soma-se
    /// à exposição interpolada e decai exponencialmente ([`decay_kick`]);
    /// é o flash do level-up (`vitals::LEVELUP_KICK_EV`).
    pub kick: f32,
}

impl Default for PostFxState {
    fn default() -> Self {
        Self {
            target_ev100: BASE_EV100,
            target_bloom: BASE_BLOOM,
            ev100: BASE_EV100,
            bloom: BASE_BLOOM,
            kick: 0.0,
        }
    }
}

impl PostFxState {
    /// Soma um kick de exposição (EV positivo clareia). Acumulativo até
    /// [`MAX_KICK_EV`]; o decay ([`decay_kick`], no `drive_postfx`) trata do
    /// resto — chamar de novo num kick a meio não reinicia o flash, soma-lhe.
    pub fn kick_exposure(&mut self, ev_delta: f32) {
        if !ev_delta.is_finite() || ev_delta <= 0.0 {
            return;
        }
        self.kick = (self.kick + ev_delta).clamp(0.0, MAX_KICK_EV);
    }
}

/// EV100 efetivo na câmara: o kick CLAREIA (menos EV = mais luz captada).
pub fn ev_with_kick(ev100: f32, kick: f32) -> f32 {
    ev100 - kick
}

/// Decay exponencial do kick (`dt` em segundos). Monotónico e determinístico:
/// corta a zero em [`KICK_EPS`] para o flash ter fim exato (e a escrita na
/// câmara não ficar eternamente em `Changed`).
pub fn decay_kick(kick: f32, dt: f32) -> f32 {
    if kick <= 0.0 || !kick.is_finite() {
        return 0.0;
    }
    let next = kick * (-dt.max(0.0) / KICK_TAU).exp();
    if next < KICK_EPS {
        0.0
    } else {
        next
    }
}

/// `pp-exposure` do XML é um multiplicador de exposição linear (0.78, 0.70…),
/// não um EV. Menos luz = mais EV100 na câmara, e a relação é logarítmica:
/// `ev = base − log2(mult)`. Um `pp-exposure` de 0.5 escurece exactamente um
/// stop.
pub fn ev100_for_exposure_multiplier(base_ev100: f32, multiplier: f32) -> f32 {
    if multiplier <= 0.0 || !multiplier.is_finite() {
        return base_ev100;
    }
    base_ev100 - multiplier.log2()
}

/// Pulso de pós-processo num impacto de combate: `stops` de exposição
/// (positivo CLAREIA um instante — hit 0.25, crítico/finisher 0.5, abate 0.7;
/// negativo ESCURECE — o "ai" do dano recebido) + `bloom_add` de
/// florescimento. Clarear usa o kick de juice (teto [`MAX_KICK_EV`], decay
/// próprio); escurecer empurra o EV efetivo para cima (piso
/// [`MAX_DARKEN_EV`]) e o bloom soma até [`MAX_BLOOM`] — o `drive_postfx`
/// devolve tudo ao alvo do bioma a [`BLEND_RATE`]/s sozinho, o punch nunca
/// fica preso.
pub fn punch_impact(state: &mut PostFxState, stops: f32, bloom_add: f32) {
    if !stops.is_finite() || !bloom_add.is_finite() {
        return;
    }
    if stops > 0.0 {
        state.kick_exposure(stops);
    } else if stops < 0.0 {
        state.ev100 = (state.ev100 - stops).min(BASE_EV100 + MAX_DARKEN_EV);
    }
    if bloom_add != 0.0 {
        state.bloom = (state.bloom + bloom_add).clamp(0.0, MAX_BLOOM);
    }
}

/// Liga o pós-processamento e mantém exposição/bloom sincronizados com o
/// bioma do herói.
pub struct PostFxPlugin;

impl Plugin for PostFxPlugin {
    fn build(&self, app: &mut App) {
        // O estado vive MESMO com o pós-processo desligado: os punches de
        // combate/skills escrevem-no (kick_exposure/punch_impact) e é só o
        // drive que não corre — sem o init, esses sistemas panica-iam.
        app.init_resource::<PostFxState>();
        if std::env::var_os("VIBER_NO_POSTFX").is_some() {
            info!("postfx: desligado por VIBER_NO_POSTFX");
            return;
        }
        app.add_systems(
            bevy::app::Update,
            (
                attach_postfx_to_cameras,
                // O grading lê [`crate::worldsys::AtmosphereState`] do MESMO
                // frame (publicada depois de `sun_drive`; o registo/glue vive
                // no `AmbientPlugin`, que também consome a paleta).
                drive_postfx.after(crate::worldsys::atmosphere_drive),
            )
                .chain(),
        );
    }
}

/// Equipa cada `Camera3d` que ainda não tem pós-processamento. Corre no
/// `Update` (e não num startup) porque a câmara do mundo nasce dentro do
/// spawn exclusivo, depois dos startup systems normais.
fn attach_postfx_to_cameras(
    mut commands: Commands,
    state: Res<PostFxState>,
    cameras: Query<Entity, (With<Camera3d>, Without<Bloom>)>,
) {
    for camera in &cameras {
        commands.entity(camera).insert((
            Bloom {
                intensity: state.bloom,
                // O preset NATURAL tem threshold 0.0: COM TUDO a brilhar, o
                // boost de baixa-frequência (0.7) transforma regiões lisas e
                // grandes de HDR alto — o domo do céu em radiância de cena
                // (~400, ver sky.wgsl) — numa wash de ecrã inteiro (superfícies
                // texturizadas cancelam-se nos mips; um gradiente liso não).
                // Threshold 700 deixa o glow para o que é realmente brilhante:
                // o disco solar e fontes emissivas à noite.
                prefilter: BloomPrefilter {
                    threshold: 700.0,
                    threshold_softness: 0.5,
                },
                ..Bloom::NATURAL
            },
            Exposure { ev100: state.ev100 },
            // TonyMcMapface (r5): o ACES esmagava o toe — sombras e o
            // primeiro plano da golden hour liam-se como preto puro (o
            // crítico reprovou 3×). O TMcMapface levanta a toe e amortece o
            // ombro: sombras com textura, highlights sem clipar a branco.
            Tonemapping::TonyMcMapface,
            // O AO precisa dos dois prepasses; o componente declara-os em
            // `#[require]`, mas inseri-los aqui deixa a dependência explícita
            // para quem ler o spawn da câmara.
            DepthPrepass,
            NormalPrepass,
            ScreenSpaceAmbientOcclusion {
                // `High` é o default do Bevy; `Medium` custa metade das
                // amostras e sem TAA a diferença de ruído é pequena depois do
                // denoise espacial.
                quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Medium,
                ..ScreenSpaceAmbientOcclusion::default()
            },
            // O SSAO do Bevy resolve contra um alvo de amostra única e recusa
            // correr com MSAA ligado — sem isto o render loga
            // `SSAO ... requires Msaa::Off` por frame e o AO nunca aparece.
            // O FXAA substitui o anti-aliasing que o MSAA fazia.
            Msaa::Off,
            Fxaa::default(),
        ));
    }
}

/// Interpola exposição/bloom na direção do bioma onde o herói está.
#[allow(clippy::type_complexity)]
fn drive_postfx(
    time: Res<Time>,
    atmosphere: Res<crate::worldsys::AtmosphereState>,
    biomes: Option<Res<BiomeRegions>>,
    players: Query<&GlobalTransform, With<Player>>,
    mut state: ResMut<PostFxState>,
    mut cameras: Query<(&mut Bloom, &mut Exposure), With<Camera3d>>,
) {
    let mut exposure_mult = 1.0;
    let mut bloom = BASE_BLOOM;
    if let (Some(biomes), Ok(player)) = (biomes, players.single()) {
        let pos = player.translation();
        let region = biomes
            .list
            .iter()
            .find(|b| point_in_polygon(pos.x, pos.z, &b.polygon));
        if let Some(region) = region {
            exposure_mult = region.pp_exposure.unwrap_or(1.0);
            bloom = region.pp_bloom_strength.unwrap_or(BASE_BLOOM);
        }
    }
    // A HORA manda por cima do bioma: a noite tem de ser mesmo mais escura
    // (senão é "o dia com o brilho baixado") e o bloom sobe onde há fontes
    // quentes — fogueiras à noite, glare do sol rasante.
    state.target_ev100 = ev100_for_exposure_multiplier(
        BASE_EV100,
        exposure_mult * atmosphere.exposure_scale.max(0.05),
    );
    state.target_bloom = (bloom + atmosphere.bloom_boost).clamp(0.0, 0.5);

    let t = (time.delta_secs() * BLEND_RATE).clamp(0.0, 1.0);
    // O kick de juice decai SEMPRE (mesmo com o alvo de bioma atingido) —
    // é um flash transitório, não um estado.
    if state.kick > 0.0 {
        state.kick = decay_kick(state.kick, time.delta_secs());
    }
    // Early-out quando o alvo já foi atingido: sem isto a escrita por frame
    // mantinha `Bloom`/`Exposure` em `Changed` para sempre (mundo estático
    // re-disparava dependências desnecessariamente).
    if (state.target_ev100 - state.ev100).abs() > 1e-4 {
        state.ev100 += (state.target_ev100 - state.ev100) * t;
    }
    if (state.target_bloom - state.bloom).abs() > 1e-4 {
        state.bloom += (state.target_bloom - state.bloom) * t;
    }
    let (ev100, bloom) = (ev_with_kick(state.ev100, state.kick), state.bloom);
    for (mut camera_bloom, mut exposure) in &mut cameras {
        if camera_bloom.intensity != bloom {
            camera_bloom.intensity = bloom;
        }
        if exposure.ev100 != ev100 {
            exposure.ev100 = ev100;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exposure_multiplier_maps_to_stops() {
        // Multiplicador 1 = sem alteração.
        assert!((ev100_for_exposure_multiplier(10.0, 1.0) - 10.0).abs() < 1e-5);
        // Metade da luz = mais um stop de EV (imagem mais escura).
        assert!((ev100_for_exposure_multiplier(10.0, 0.5) - 11.0).abs() < 1e-5);
        // O dobro = menos um stop.
        assert!((ev100_for_exposure_multiplier(10.0, 2.0) - 9.0).abs() < 1e-5);
        // Valores inválidos não mexem na base.
        assert!((ev100_for_exposure_multiplier(10.0, 0.0) - 10.0).abs() < 1e-5);
        assert!((ev100_for_exposure_multiplier(10.0, f32::NAN) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn test_biome_exposure_of_the_simple_rpg_darkens() {
        // `pp-exposure="0.70"` do pântano tem de escurecer face à base.
        let swamp = ev100_for_exposure_multiplier(BASE_EV100, 0.70);
        assert!(swamp > BASE_EV100, "0.70 escurece: {swamp} vs {BASE_EV100}");
        // E o deserto (0.74) escurece menos que o pântano.
        let desert = ev100_for_exposure_multiplier(BASE_EV100, 0.74);
        assert!(desert < swamp, "0.74 é mais claro que 0.70");
    }

    #[test]
    fn test_punch_impact_brightens_darkens_and_clamps() {
        // Hit normal: +0.25 stops de clareza via kick, bloom sobe.
        let mut state = PostFxState::default();
        punch_impact(&mut state, 0.25, 0.10);
        assert!((state.kick - 0.25).abs() < 1e-5, "kick {}", state.kick);
        assert!((state.bloom - (BASE_BLOOM + 0.10)).abs() < 1e-5);
        // Abate: kick acumula até ao teto e o bloom bate no MAX_BLOOM.
        let mut state = PostFxState::default();
        punch_impact(&mut state, MAX_KICK_EV, MAX_BLOOM);
        punch_impact(&mut state, MAX_KICK_EV, MAX_BLOOM);
        assert!((state.kick - MAX_KICK_EV).abs() < 1e-5, "teto do kick");
        assert!((state.bloom - MAX_BLOOM).abs() < 1e-5, "teto do bloom");
        // Dano recebido: escurece (EV sobe), com piso.
        let mut state = PostFxState::default();
        punch_impact(&mut state, -0.3, 0.0);
        assert!((state.ev100 - (BASE_EV100 + 0.3)).abs() < 1e-5);
        punch_impact(&mut state, -5.0, 0.0);
        assert!(
            (state.ev100 - (BASE_EV100 + MAX_DARKEN_EV)).abs() < 1e-5,
            "piso do escurecimento: {}",
            state.ev100
        );
        // Punch não mexe nos ALVOS (só no estado corrente/kick) — o bioma
        // continua a mandar.
        assert!((state.target_ev100 - BASE_EV100).abs() < 1e-5);
        // Lixo não finito é no-op.
        let mut state = PostFxState::default();
        punch_impact(&mut state, f32::NAN, f32::NAN);
        assert!((state.ev100 - BASE_EV100).abs() < 1e-5);
        assert!((state.bloom - BASE_BLOOM).abs() < 1e-5);
        assert_eq!(state.kick, 0.0);
    }

    #[test]
    fn test_kick_decay_is_monotonic_and_terminates() {
        // Curva do flash do level-up: 0.6 EV a decair a 60 fps.
        let dt = 1.0 / 60.0;
        let mut kick = 0.6;
        let mut prev = kick;
        let mut frames = 0;
        while kick > 0.0 {
            kick = decay_kick(kick, dt);
            assert!(kick <= prev, "decay monotónico: {prev} → {kick}");
            prev = kick;
            frames += 1;
            assert!(frames < 600, "kick tem de terminar (~1 s), não eternizar-se");
        }
        // ~1 s de decay visível: 3 τ ≈ 0.96 s.
        assert!(
            (120..=240).contains(&frames),
            "decay do kick termina em ~3τ: {frames} frames"
        );
        // Zero fica zero; lixo finito/não finito não fabrica kick.
        assert_eq!(decay_kick(0.0, dt), 0.0);
        assert_eq!(decay_kick(-0.5, dt), 0.0);
        assert_eq!(decay_kick(f32::NAN, dt), 0.0);
        assert_eq!(decay_kick(0.6, 0.0), 0.6, "dt=0 não decai");
        assert_eq!(decay_kick(0.6, -1.0), 0.6, "dt negativo é ignorado");
    }

    #[test]
    fn test_kick_accumulates_with_cap_and_exposure_brightens() {
        let mut state = PostFxState::default();
        assert_eq!(state.kick, 0.0);
        state.kick_exposure(crate::vitals::LEVELUP_KICK_EV);
        assert!((state.kick - 0.6).abs() < 1e-5, "kick do level-up: {}", state.kick);
        // Um segundo level-up a meio soma (não reinicia) — até ao teto.
        state.kick_exposure(crate::vitals::LEVELUP_KICK_EV);
        assert!((state.kick - MAX_KICK_EV).abs() < 1e-5, "teto: {}", state.kick);
        state.kick_exposure(MAX_KICK_EV);
        assert!((state.kick - MAX_KICK_EV).abs() < 1e-5, "não passa do teto");
        // Lixo não mexe.
        state.kick_exposure(f32::NAN);
        assert!((state.kick - MAX_KICK_EV).abs() < 1e-5);
        // Kick positivo CLAREIA: EV efetivo desce.
        assert!(
            ev_with_kick(state.ev100, state.kick) < state.ev100,
            "kick clareia (menos EV)"
        );
    }
}
