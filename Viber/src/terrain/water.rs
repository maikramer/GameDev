//! Water features — lakes and rivers: lower-only terrain carve plus water
//! surface geometry and the [`WaterBody`] query registry.
//!
//! Ported from the VibeGame water plugin (`water/carve.ts`, `river-channel.ts`,
//! `lake-bowl.ts`). Contracts kept from the original:
//!
//! * **Lower-only carve** — water never raises terrain, so overlapping bodies
//!   and pre-existing valleys are always safe (brush [`BrushMode::Lower`];
//!   banks are the one intentional raise, applied first and capped).
//! * **Lakes** — an organic contour (`lake_shape_radius`: ±28% of the radius
//!   across three sine harmonics with position-seeded phase) is sampled on a
//!   32-ray ring to find the `rim`; the bowl floor is
//!   `rim − depth · (1 − t²)^1.5` (C1 at the rim), carved out to
//!   `carveR = radius · 1.25`. The water mirror sits at `rim − water_offset`.
//! * **Rivers** — the path is Chaikin-smoothed ×2 and resampled to 3 m
//!   stations; axis heights are sampled from the **post-pad, pre-water** grid
//!   and box-smoothed (±2 stations); the surface is a **descending prefix
//!   min** (`surf[i] = min(surf[i−1], axis[i] − water_offset)`) so water never
//!   flows uphill. Carving runs per station: banks raise first (capped
//!   profile), then the channel + bank cut lower — a bank from a neighbouring
//!   station never ends up inside the channel.
//! * **Registry** — every carve returns a [`WaterBody`]; gameplay (spawner
//!   `avoid-water` / `near-water`) queries it without touching the grid.

use bevy::math::Vec2;

use super::brush::{
    BrushGrid, BrushMode, BrushRequest, min_effective, smootherstep01, smoothstep01,
};
use super::mesh::ChunkMeshData;
use super::paths::{PathHit, chaikin_smooth, distance_to_path, nearest_on_path, resample, station_lerp};
use super::roads::ADAPTIVE_FALLOFF_FACTOR;

/// Lake contour: number of rim rays sampled (VibeGame `rimY` ring).
const RIM_RAYS: usize = 32;
/// Lake contour: harmonic amplitudes of `shapeRadius` (±28% total).
const SHAPE_AMPLITUDES: [f32; 3] = [0.12, 0.10, 0.06];
/// Peak of the organic contour relative to the design radius
/// (`1 + Σ SHAPE_AMPLITUDES = 1.28`). The carve AABB must cover
/// `radius · CONTOUR_PEAK · CARVE_MARGIN` — capping it at plain
/// `radius · CARVE_MARGIN` clipped the bowl of large lakes and left the water
/// mirror over dry land on the harmonic lobes.
const CONTOUR_PEAK: f32 = 1.28;
/// Lake carve margin over the design radius (VibeGame `carveMargin`).
pub const CARVE_MARGIN: f32 = 1.25;
/// River station spacing (meters, VibeGame `STATION_SPACING`).
pub const RIVER_STATION_SPACING: f32 = 3.0;
/// River: **minimum** falloff band outside the banks (meters). The band grows
/// with the local cut depth (see [`ADAPTIVE_FALLOFF_FACTOR`]) — a fixed band
/// turns every deep cut into a vertical wall.
pub const FEATHER_WIDTH: f32 = 2.5;
/// River: maximum bank raise over the **pre-carve axis height** (meters) —
/// deeper cuts read as a cascade instead of a levee wall.
pub const MAX_BANK_RAISE: f32 = 2.0;
/// River channel: minimum cut below the water surface (meters).
const MIN_CHANNEL_DEPTH: f32 = 0.2;
/// Water surface alpha fades to 0 over the outer fraction of the width.
const WATER_EDGE_FADE: f32 = 0.25;
/// Lake fan segments around the contour (VibeGame 72-segment mirror).
const LAKE_FAN_SEGMENTS: usize = 72;
/// Queda (m) entre estações vizinhas que marca uma cascata — abaixo disto
/// a ribbon lê-se como corredeira; acima, o [`river_water_mesh`] constrói
/// face vertical e o canal ganha caldeirão.
pub const CASCADE_DROP: f32 = 1.2;
/// Fase dos poços/rápidos: hash determinístico do primeiro ponto do path
/// (mesma família de [`shape_phases`]) — mesmos pontos, mesmos poços.
fn river_phase(path: &[Vec2]) -> f32 {
    let s = path
        .first()
        .map(|p| p.x * 12.989_8 + p.y * 78.233)
        .unwrap_or(0.0);
    (s.sin() * 43_758.55).fract() * std::f32::consts::TAU
}

/// Estilo de margem (`bank="soft|beach|cliff|terraced|gorge|overhang"` no
/// `<Lake>`/`<River>`) — o mesmo vocabulário de perfis do cliff system
/// aplicado à borda da água. `Gorge`/`Overhang` são os estilos VOXEL: a
/// parede acima da lâmina não é rampa no heightfield, é sólido cortado por
/// mods ([`super::voxel`]) — vertical de primeira ou com undercut real
/// (rocha por cima da água).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BankStyle {
    /// Feather adaptativo — a transição lê a encosta natural (comportamento
    /// histórico).
    #[default]
    Soft,
    /// Praia: banda larga com rampa de slope zero nas pontas — a margem
    /// desce suave vários metros antes da água.
    Beach,
    /// Falésia: banda estreita com parede (perfil vertical do cliff system) —
    /// lagos de recorte em encosta.
    Cliff,
    /// Terraços: banda média em degraus (perfis do cliff system) — margem
    /// escavada, look de arrozal/pedreira.
    Terraced,
    /// Garganta: paredes verticais dos DOIS lados (sólido voxel) — o rio
    /// corre num canhão; o heightfield mantém o terreno natural por cima.
    Gorge,
    /// Saliente: como gorge, com o topo da parede a avançar SOBRE a água
    /// (undercut — `profile_offset` negativo do cliff voxel).
    Overhang,
}

impl BankStyle {
    /// `None` para nomes fora do vocabulário (o parser avisa e mantém soft).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "soft" => Some(Self::Soft),
            "beach" | "praia" => Some(Self::Beach),
            "cliff" | "wall" | "falésia" | "falesia" => Some(Self::Cliff),
            "terraced" | "terrace" | "terraços" | "terracos" => Some(Self::Terraced),
            "gorge" | "garganta" | "canhão" | "canhao" => Some(Self::Gorge),
            "overhang" | "saliente" => Some(Self::Overhang),
            _ => None,
        }
    }

    /// Estilos cuja parede vive no CAMPO VOXEL: o carve do heightfield não
    /// esculpe a rampa da margem (o sólido natural fica de pé e é o mod que
    /// corta a parede até abaixo da lâmina).
    pub fn is_voxel(self) -> bool {
        matches!(self, Self::Gorge | Self::Overhang)
    }

    /// Largura da banda de margem para um corte local de `cut` m (lago) ou
    /// profundidade de canal `depth` (rio). `feather_base` é o mínimo útil.
    fn band(self, feather_base: f32, cut: f32) -> f32 {
        match self {
            // A regra histórica: o band cresce com o corte local — um feather
            // fixo sobre um corte fundo era uma parede vertical.
            Self::Soft => feather_base.max(ADAPTIVE_FALLOFF_FACTOR * cut),
            Self::Beach => (feather_base * 2.2).max(ADAPTIVE_FALLOFF_FACTOR * cut * 1.6),
            Self::Cliff => (feather_base * 0.8).max(cut * 0.9),
            Self::Terraced => (feather_base * 1.2).max(cut * 1.8),
            // Voxel: sem rampa — só o assento mínimo do canal (a parede nasce
            // do sólido natural e é cortada pelo mod).
            Self::Gorge | Self::Overhang => feather_base * 0.5,
        }
    }

    /// Progresso 0→1 do natural para a taça ao longo da banda (`t` 0..1).
    /// O peso do brush é `1 − progresso`: cliff mantém a taça cheia até
    /// quase a crista e cai a meio da banda (parede), terraced desce em
    /// degraus, beach/spread suave.
    fn progress(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Soft | Self::Beach => {
                if matches!(self, Self::Soft) {
                    smoothstep01(t)
                } else {
                    smootherstep01(t)
                }
            }
            Self::Cliff | Self::Gorge | Self::Overhang => {
                // Perfil vertical do cliff system: plano no terço superior e
                // inferior, queda no terço médio.
                smoothstep01(((t - 1.0 / 3.0) * 3.0).clamp(0.0, 1.0))
            }
            Self::Terraced => {
                // Escadaria: degraus com risers suaves (margem em bancos).
                let steps = 3.0;
                let x = (t * steps).min(steps - 1e-4);
                let i = x.floor();
                (i + smoothstep01(x - i)) / steps
            }
        }
    }
}

/// Fração do contorno onde a taça cruza a lâmina: resolvendo
/// `depth·(1−t²)^1.5 = offset` → `t = √(1 − (offset/depth)^⅔)`. O espelho de
/// água termina EXATAMENTE aí — acabar antes deixa um anel de leito
/// submerso sem água por cima; passar, pousa a lâmina sobre areia seca.
pub(crate) fn waterline_reach(depth: f32, offset: f32) -> f32 {
    let ratio = (offset / depth.max(1e-3)).clamp(0.0, 1.0);
    (1.0 - ratio.powf(2.0 / 3.0)).max(0.0).sqrt()
}

/// Reach do ESPELHO de um lago sobre o contorno nominal: a linha de água
/// real ([`waterline_reach`]) em unidades de contorno — ×[`CARVE_MARGIN`]
/// (a taça carva estende-se até lá) e clampado ao pico do contorno
/// harmónico. Partilhado pelo carve (registry), pelo mesh do espelho e
/// pela métrica de [`WaterBody::distance_to_waterline`] — os três têm de
/// concordar ao centímetro, senão a banda de areia/lama do splat
/// desancora da linha de água.
pub(crate) fn lake_mirror_reach(depth: f32, water_offset: f32) -> f32 {
    (waterline_reach(depth, water_offset) * CARVE_MARGIN).clamp(0.5, CONTOUR_PEAK * CARVE_MARGIN)
}

/// Ilha declarada dentro de um lago (filho repetível
/// `<Island at="x z" radius height/>`). É um RAISE de domo na bacia — a
/// lâmina rodeia-a, o splat pinta a praia em anel e a relva nasce no topo
/// (acima da lâmina os gates de água já não apanham o texel).
#[derive(Debug, Clone, PartialEq)]
pub struct IslandSpec {
    /// Centro da ilha em XZ mundo.
    pub at: Vec2,
    /// Raio da base (m) — a praia espalha-se para lá dele.
    pub radius: f32,
    /// Altura do domo ACIMA da lâmina (m).
    pub height: f32,
}

impl Default for IslandSpec {
    fn default() -> Self {
        Self {
            at: Vec2::ZERO,
            radius: 5.0,
            height: 1.2,
        }
    }
}

/// Declarative lake (`<Lake at radius depth water-offset color opacity>`).
#[derive(Debug, Clone, PartialEq)]
pub struct LakeSpec {
    /// Lake center in world XZ.
    pub at: Vec2,
    /// Design radius of the water mirror (meters).
    pub radius: f32,
    /// Maximum bowl depth below the rim (meters).
    pub depth: f32,
    /// Water surface drop below the rim (meters).
    pub water_offset: f32,
    /// Water body color (sRGB 0..1).
    pub color: [f32; 3],
    /// Water surface opacity (0..1).
    pub opacity: f32,
    /// Ripple strength (reserved for the animated water material).
    pub ripple: f32,
    /// Margem esculpida (`bank="soft|beach|cliff|terraced|gorge|overhang"`).
    pub bank: BankStyle,
    /// Pedras de margem automáticas (`rocks="1"`).
    pub rocks: bool,
    /// Parâmetros dessas pedras (`rocks-density` / `rocks-scale-max`).
    pub rocks_spec: super::shore_rocks::ShoreRocksSpec,
    /// Ilhas na bacia (filhos `<Island/>`).
    pub islands: Vec<IslandSpec>,
}

impl Default for LakeSpec {
    fn default() -> Self {
        Self {
            at: Vec2::ZERO,
            radius: 6.0,
            depth: 1.5,
            water_offset: 0.5,
            color: [0.184, 0.478, 0.604], // #2f7a9a
            // `opacity` deixou de ser um alpha constante: o shader lê-o
            // como a ESCALA DE EXTINÇÃO da coluna de água (uv.x), pelo que
            // 0.75 dá um lago fundo praticamente opaco no centro e ainda
            // assim transparente nos 30 cm da margem.
            opacity: 0.75,
            ripple: 0.6,
            bank: BankStyle::Soft,
            rocks: false,
            rocks_spec: super::shore_rocks::ShoreRocksSpec::default(),
            islands: Vec::new(),
        }
    }
}

/// Declarative river (`<River path width depth water-offset bank-width …>`).
#[derive(Debug, Clone, PartialEq)]
pub struct RiverSpec {
    /// Centerline polyline in world XZ (`"x z x z …"`).
    pub path: Vec<Vec2>,
    /// Full water width (meters).
    pub width: f32,
    /// Channel depth below the water surface (meters).
    pub depth: f32,
    /// Water surface drop below the smoothed axis (meters).
    pub water_offset: f32,
    /// Bank band width outside the water (meters).
    pub bank_width: f32,
    /// Bank raise height (meters).
    pub bank_height: f32,
    /// Water body color (sRGB 0..1).
    pub color: [f32; 3],
    /// Water surface opacity (0..1).
    pub opacity: f32,
    /// Margem esculpida (`bank="soft|beach|cliff|terraced|gorge|overhang"`).
    pub bank: BankStyle,
    /// Pedras de margem automáticas (`rocks="1"`).
    pub rocks: bool,
    /// Parâmetros dessas pedras (`rocks-density` / `rocks-scale-max`).
    pub rocks_spec: super::shore_rocks::ShoreRocksSpec,
    /// Espaçamento (m) entre poços e rápidos (`pool-spacing`; 0 = leito
    /// uniforme como sempre). A SUPERFÍCIE mantém o prefix-min descendente —
    /// o LEITO ondula por baixo (poços fundos, rápidos rasos), e a
    /// profundidade variável aparece no shader pela absorção por depth
    /// prepass.
    pub pool_spacing: f32,
    /// Deteção automática de cascatas (`cascades="0"` desliga): queda maior
    /// que [`CASCADE_DROP`] entre estações marca uma cascata, com caldeirão
    /// esculpido a jusante e face de água vertical ([`cascade_ranges`]).
    pub cascades: bool,
    /// Nascente esculpida na estação 0 (`spring="1"`): anfiteatro + arco de
    /// rocha voxel voltado a jusante, a água "sai" da rocha.
    pub spring: bool,
}

impl Default for RiverSpec {
    fn default() -> Self {
        Self {
            path: Vec::new(),
            width: 6.0,
            depth: 1.5,
            water_offset: 0.3,
            bank_width: 2.0,
            bank_height: 0.9,
            color: [0.165, 0.4, 0.522], // #2a6685
            opacity: 0.8,               // escala de extinção da coluna (ver `LakeSpec`)
            bank: BankStyle::Soft,
            rocks: false,
            rocks_spec: super::shore_rocks::ShoreRocksSpec::default(),
            pool_spacing: 0.0,
            cascades: true,
            spring: false,
        }
    }
}

/// Kind of a registered [`WaterBody`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    Lake,
    River,
}

/// Query registry entry produced by every water carve.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterBody {
    pub kind: WaterKind,
    /// Anchor point (lake center; river centroid).
    pub at: Vec2,
    /// Lake: design water radius. River: `0.0`.
    pub radius: f32,
    /// Avoid-zone radius (lake: carve radius; river: half carve width).
    pub carve_radius: f32,
    /// Lake mirror height (rivers: mean surface height).
    pub water_y: f32,
    /// Lake: reach do ESPELHO sobre o contorno nominal — o MESMO fator
    /// [`lake_mirror_reach`] que o mesh do espelho aplica ao anel exterior;
    /// [`WaterBody::distance_to_waterline`] mede até
    /// `contorno × mirror_reach` (a linha de água real, não o contorno
    /// nominal — a banda de areia/lama do splat fica ancorada ao bordo da
    /// água). Rio / corpo à mão: `1.0` (contrato antigo).
    pub mirror_reach: f32,
    /// River stations in world XZ (empty for lakes).
    pub stations: Vec<Vec2>,
    /// River surface height per station (empty for lakes).
    pub surface_y: Vec<f32>,
    /// River full water width (lakes: `0.0`).
    pub water_width: f32,
    /// Meia-largura EFETIVA da água POR estação (poços largos, rápidos
    /// estreitos, já escalada pelo reach da linha de água no carve — a
    /// ribbon e as queries de água partilham esta métrica; ver
    /// [`river_water_mesh`]). Vazio = largura constante
    /// (`water_width * 0.5` em toda a linha) — é o contrato que os
    /// consumidores antigos assumem.
    pub half_width: Vec<f32>,
    /// Rio: profundidade EFETIVA do canal POR estação — poços fundos,
    /// rápidos rasos, caldeirão de cascata/nascente; é este vetor que o
    /// carve usa no perfil do canal e que alimenta o reach da linha de
    /// água guardado em `half_width`. Vazio = profundidade constante
    /// (contrato antigo: o consumidor usa a nominal da spec — ver
    /// [`WaterBody::depth_at`]).
    pub depths: Vec<f32>,
    /// Índices das estações LIP de cada cascata (queda
    /// `surface[i] − surface[i+1] > CASCADE_DROP`). Vazio = rio sem quedas.
    pub cascades: Vec<usize>,
}

impl WaterBody {
    /// Meia-largura da água numa estação (com fallback do contrato antigo).
    pub fn half_width_at(&self, i: usize) -> f32 {
        self.half_width
            .get(i)
            .copied()
            .unwrap_or(self.water_width * 0.5)
    }
    /// Profundidade EFETIVA do canal numa estação. `fallback` é a
    /// profundidade NOMINAL da spec — o contrato antigo dos corpos sem
    /// `depths` (construídos à mão ou por versões anteriores do carve).
    pub fn depth_at(&self, i: usize, fallback: f32) -> f32 {
        self.depths.get(i).copied().unwrap_or(fallback)
    }
    /// Meia-largura EFETIVA interpolada no ponto do caminho (`hit`) — a
    /// linha de água real (as larguras guardadas já incluem o reach do
    /// carve). Fallback do contrato antigo quando o vetor está vazio.
    fn half_width_lerp(&self, hit: &PathHit) -> f32 {
        if self.half_width.is_empty() {
            self.water_width * 0.5
        } else {
            station_lerp(&self.half_width, hit)
        }
    }
    /// Point is inside the carve zone (spawner `avoid-water`).
    pub fn contains(&self, p: Vec2) -> bool {
        match self.kind {
            WaterKind::Lake => p.distance(self.at) <= self.carve_radius,
            WaterKind::River => {
                !self.stations.is_empty()
                    && distance_to_path(&self.stations, p) <= self.carve_radius
            }
        }
    }

    /// Point is inside the carve zone plus `margin` (spawner `near-water`).
    pub fn is_near(&self, p: Vec2, margin: f32) -> bool {
        match self.kind {
            WaterKind::Lake => p.distance(self.at) <= self.carve_radius + margin,
            WaterKind::River => {
                !self.stations.is_empty()
                    && distance_to_path(&self.stations, p) <= self.carve_radius + margin
            }
        }
    }

    /// Water surface height at `p` when the point is over the water ribbon
    /// (rivers: nearest station surface; lakes: flat mirror when inside the
    /// design radius). `None` when the point is on dry land.
    pub fn surface_y_at(&self, p: Vec2) -> Option<f32> {
        match self.kind {
            WaterKind::Lake => {
                let d = p.distance(self.at);
                (d <= self.radius.max(self.carve_radius)).then_some(self.water_y)
            }
            WaterKind::River => {
                let hit = nearest_on_path(&self.stations, p)?;
                let d = hit.point.distance(p);
                // Meia-largura EFETIVA da estação — a ribbon acaba em
                // `half · reach` (guardado no carve); usar a nominal punha a
                // lâmina/queries sobre a margem seca.
                (d <= self.half_width_lerp(&hit)).then(|| {
                    if self.surface_y.is_empty() {
                        self.water_y
                    } else {
                        station_lerp(&self.surface_y, &hit)
                    }
                })
            }
        }
    }

    /// Horizontal distance from `p` to the waterline (meters; **negative**
    /// inside the water mirror). This is the geometry line — the shoreline a
    /// renderer or spawner cares about — not the carve radius: over a lake
    /// slope the carve reaches past the mirror, and painting sand to the carve
    /// radius would put beaches on dry hillsides. No lago a linha é o
    /// contorno ×[`WaterBody::mirror_reach`] — o bordo REAL do espelho (a
    /// mesma métrica do mesh): medir ao contorno nominal punha a praia de
    /// areia/lama do splat para lá (fundo) ou para dentro (raso) da água.
    pub fn distance_to_waterline(&self, p: Vec2) -> f32 {
        match self.kind {
            WaterKind::Lake => {
                let delta = p - self.at;
                let theta = delta.y.atan2(delta.x);
                let contour = lake_shape_radius(self.radius, theta, shape_phases(self.at))
                    * self.mirror_reach;
                delta.length() - contour
            }
            WaterKind::River => match nearest_on_path(&self.stations, p) {
                Some(hit) => hit.point.distance(p) - self.half_width_lerp(&hit),
                None => f32::INFINITY,
            },
        }
    }

    /// Water surface height above `p` for ground-color work (the splat map):
    /// like [`WaterBody::surface_y_at`] but with a **margin** so a texel just
    /// outside the mirror still resolves a surface while it lies inside the
    /// waterline-plus-margin band. `None` far from any water.
    pub fn blend_surface_y(&self, p: Vec2, margin: f32) -> Option<f32> {
        match self.kind {
            WaterKind::Lake => {
                let d = p.distance(self.at);
                (d <= self.radius.max(self.carve_radius) + margin).then_some(self.water_y)
            }
            WaterKind::River => {
                let hit = nearest_on_path(&self.stations, p)?;
                let d = hit.point.distance(p);
                // Meia-largura EFETIVA + margem (mesma métrica de
                // [`WaterBody::surface_y_at`]).
                (d <= self.half_width_lerp(&hit) + margin).then(|| {
                    if self.surface_y.is_empty() {
                        self.water_y
                    } else {
                        station_lerp(&self.surface_y, &hit)
                    }
                })
            }
        }
    }
}

/// Deterministic harmonic phases for a lake contour, seeded by position.
pub(crate) fn shape_phases(at: Vec2) -> [f32; 3] {
    // Classic deterministic hash — precision beyond f32 is meaningless here.
    let s = at.x * 12.989_8_f32 + at.y * 78.233_f32;
    let base = (s.sin() * 43_758.55_f32).fract() * std::f32::consts::TAU;
    [base, base * 1.7 + 2.1, base * 0.6 + 4.4]
}

/// Organic lake contour: `radius · (1 + Σ aₖ·sin(kθ + φₖ))` (±28%).
pub fn lake_shape_radius(radius: f32, theta: f32, phases: [f32; 3]) -> f32 {
    let mut r = 1.0;
    for (k, a) in SHAPE_AMPLITUDES.iter().enumerate() {
        let harmonic = [2.0_f32, 3.0, 5.0][k]; // sin(2θ), sin(3θ), sin(5θ)
        r += a * (harmonic * theta + phases[k]).sin();
    }
    radius * r
}

/// Lake mirror height: the 32-ray rim minimum minus `water_offset`.
pub fn lake_water_height(grid: &BrushGrid, spec: &LakeSpec) -> f32 {
    let phases = shape_phases(spec.at);
    let mut rim = f32::INFINITY;
    for i in 0..RIM_RAYS {
        let theta = i as f32 / RIM_RAYS as f32 * std::f32::consts::TAU;
        let r = lake_shape_radius(spec.radius, theta, phases);
        let p = spec.at + Vec2::new(theta.cos(), theta.sin()) * r;
        rim = rim.min(grid.sample(p.x, p.y));
    }
    rim - spec.water_offset
}

/// Carves a lake bowl (lower-only) and returns its registry body.
/// Returns `None` when the radius/depth are degenerate.
pub fn carve_lake(grid: &mut BrushGrid, spec: &LakeSpec, index: usize) -> Option<WaterBody> {
    if spec.radius <= 0.0 || spec.depth <= 0.0 {
        return None;
    }
    let texel = grid.texel();
    let phases = shape_phases(spec.at);
    let water_y = lake_water_height(grid, spec);
    let carve_r = spec.radius * CARVE_MARGIN;
    // Reach do espelho sobre o contorno — o MESMO fator que o mesh aplica
    // (ver `lake_water_mesh`); o registry guarda-o para que
    // `distance_to_waterline` meça até à linha de água real.
    let mirror_reach = lake_mirror_reach(spec.depth, spec.water_offset);

    // Bowl floor: C1 at the contour rim (`(1 − t²)^1.5` has zero derivative).
    let bowl = |p: Vec2| -> f32 {
        let d = p.distance(spec.at);
        let theta = (p.y - spec.at.y).atan2(p.x - spec.at.x);
        let r = lake_shape_radius(spec.radius, theta, phases) * CARVE_MARGIN;
        if d >= r {
            return water_y + spec.water_offset;
        }
        let t = d / r;
        (water_y + spec.water_offset) - spec.depth * (1.0 - t * t).max(0.0).powf(1.5)
    };

    // Shore band, per rim ray. The contour used to be a hard 0/1 mask, so a
    // lake dropped into a slope cut a cylinder: everything inside the contour
    // went down to the water surface and the uphill side became a sheer wall
    // as tall as the hillside. Grading the rim over a band that widens with
    // the local cut (the rule `roads` and the river bank already use) turns
    // that wall into a shore. The `bank` style reshapes that band — praia
    // larga, falésia estreita, terraços em degraus ([`BankStyle`]).
    let surface = water_y + spec.water_offset;
    let feather_base = min_effective(FEATHER_WIDTH, texel);
    let style = spec.bank;
    // Estilos VOXEL (gorge/overhang): espelho do rio — o heightfield carva
    // SÓ a taça até à LINHA DE ÁGUA real do perfil (a mesma métrica de
    // `voxel::riverbank::lake_shore_band` e do espelho) e NÃO esculpe a
    // rampa da margem: o sólido natural fica de pé e é o mod voxel que
    // corta a parede até abaixo da lâmina. Carvar a peso 1 até
    // contorno·CARVE_MARGIN colapsava a parede (a sonda do topo do banco
    // lia dentro da taça → margem submersa) e pintava pedra da banda no
    // LEITO (`add_authored_bands`).
    let voxel_waterline = if style.is_voxel() {
        Some(mirror_reach)
    } else {
        None
    };
    let shore: Vec<f32> = if voxel_waterline.is_some() {
        Vec::new()
    } else {
        (0..RIM_RAYS)
            .map(|i| {
                let theta = i as f32 / RIM_RAYS as f32 * std::f32::consts::TAU;
                let r = lake_shape_radius(spec.radius, theta, phases) * CARVE_MARGIN;
                let p = spec.at + Vec2::new(theta.cos(), theta.sin()) * r;
                let cut = (grid.sample(p.x, p.y) - surface).max(0.0);
                style.band(feather_base, cut)
            })
            .collect()
    };
    let shore_max = shore.iter().copied().fold(feather_base, f32::max);
    // Shore width at an arbitrary angle: linear blend of the two rim rays.
    // (No caminho voxel a banda não existe — nunca é chamado.)
    let shore_at = move |theta: f32| -> f32 {
        if shore.is_empty() {
            return 0.0;
        }
        let tau = std::f32::consts::TAU;
        let f = (theta.rem_euclid(tau) / tau) * RIM_RAYS as f32;
        let i = (f.floor() as usize) % RIM_RAYS;
        let j = (i + 1) % RIM_RAYS;
        let frac = f - f.floor();
        shore[i] + (shore[j] - shore[i]) * frac
    };

    let owner = format!("lake:{index}");
    grid.begin_stroke(&owner);
    let mut weight = |p: Vec2| {
        let d = p.distance(spec.at);
        let theta = (p.y - spec.at.y).atan2(p.x - spec.at.x);
        let contour = lake_shape_radius(spec.radius, theta, phases);
        if let Some(reach) = voxel_waterline {
            // Parede voxel: peso 1 até à linha de água, 0 para lá — sem
            // rampa (o mod voxel trata da margem).
            return if d < contour * reach { 1.0 } else { 0.0 };
        }
        let r = contour * CARVE_MARGIN;
        if d < r {
            return 1.0;
        }
        let band = shore_at(theta);
        if d >= r + band {
            return 0.0;
        }
        let t = (d - r) / band.max(1e-4);
        1.0 - style.progress(t)
    };
    let mut target = bowl;
    // No guard: outside the contour `bowl` is just the water surface, not a
    // design — clamping the unweighted ring onto it is what cut a slot around
    // the shore (see the river carve for the same trap). The extent covers the
    // tallest contour lobe (`CONTOUR_PEAK · CARVE_MARGIN`), not the mean
    // `carve_r` — o peso é 1 dentro de `d < r` e o contorno orgânico passa de
    // `carve_r` nos lóbulos.
    let extent = spec.radius * CONTOUR_PEAK * CARVE_MARGIN + shore_max + texel * 2.0;
    grid.apply(BrushRequest {
        mode: BrushMode::Lower,
        min_x: spec.at.x - extent,
        min_z: spec.at.y - extent,
        max_x: spec.at.x + extent,
        max_z: spec.at.y + extent,
        target: &mut target,
        weight: &mut weight,
    });
    grid.commit_stroke();

    // Ilhas: RAISE de domo dentro da taça — o único levantamento que um lago
    // faz (a bacia é lower-only). O domo é C1 no topo e aplana em praia junto
    // à lâmina; o splat pinta o anel de areia por si (texel acima da lâmina
    // = seco na métrica de profundidade). Ilha que não caiba no contorno
    // harmónico é ignorada — só aqui se sabe o contorno real.
    for (j, island) in spec.islands.iter().enumerate() {
        if island.height <= 0.0 || island.radius <= 0.0 {
            continue;
        }
        let theta = (island.at.y - spec.at.y).atan2(island.at.x - spec.at.x);
        let contour = lake_shape_radius(spec.radius, theta, phases) * CARVE_MARGIN;
        if island.at.distance(spec.at) + island.radius > contour * 0.9 {
            continue;
        }
        let beach = (island.radius * 0.5).max(1.5);
        grid.begin_stroke(&format!("{owner}:island:{j}"));
        let mut island_weight = |p: Vec2| {
            let d = p.distance(island.at);
            if d >= island.radius + beach {
                return 0.0;
            }
            if d <= island.radius {
                return 1.0;
            }
            1.0 - smoothstep01((d - island.radius) / beach)
        };
        let mut island_target = |p: Vec2| {
            let t = (p.distance(island.at) / island.radius).clamp(0.0, 1.0);
            // Mesmo perfil da taça: C1 no topo, praia a aplanar na base.
            water_y + island.height * (1.0 - t * t).max(0.0).powf(1.5)
        };
        let iextent = island.radius + beach + texel * 2.0;
        grid.apply(BrushRequest {
            mode: BrushMode::Raise,
            min_x: island.at.x - iextent,
            min_z: island.at.y - iextent,
            max_x: island.at.x + iextent,
            max_z: island.at.y + iextent,
            target: &mut island_target,
            weight: &mut island_weight,
        });
        grid.commit_stroke();
    }

    Some(WaterBody {
        kind: WaterKind::Lake,
        at: spec.at,
        radius: spec.radius,
        carve_radius: carve_r,
        water_y,
        mirror_reach,
        stations: Vec::new(),
        surface_y: Vec::new(),
        water_width: 0.0,
        half_width: Vec::new(),
        depths: Vec::new(),
        cascades: Vec::new(),
    })
}

/// Carves a river (banks then channel) and returns its registry body.
/// `lakes` são os corpos JÁ registados — estações dentro do contorno de um
/// lago sobem à cota do espelho (confluência sem degrau). Usar o REGISTRY
/// (não as specs): `water_y` já foi resolvido no carve do lago — voltar a
/// amostrar a grid pós-carve lia o FUNDO da taça, não o espelho.
/// Returns `None` for degenerate paths.
pub fn carve_river(
    grid: &mut BrushGrid,
    spec: &RiverSpec,
    index: usize,
    lakes: &[WaterBody],
) -> Option<WaterBody> {
    if spec.path.len() < 2 || spec.width <= 0.0 {
        return None;
    }
    let texel = grid.texel();
    let width = min_effective(spec.width, texel);
    let half = width * 0.5;
    let bank = min_effective(spec.bank_width, texel);
    let bank_height = spec.bank_height;
    let feather_base = min_effective(FEATHER_WIDTH, texel);

    // Design profile: smooth the path, sample the pre-water axis heights and
    // build the descending surface (water never flows uphill).
    let smoothed = chaikin_smooth(&spec.path, 2, false);
    let stations = resample(&smoothed, RIVER_STATION_SPACING.max(texel * 0.5));
    if stations.len() < 2 {
        return None;
    }
    let mut axis: Vec<f32> = stations.iter().map(|p| grid.sample(p.x, p.y)).collect();
    box_smooth(&mut axis, 2);
    let mut surface = Vec::with_capacity(axis.len());
    let mut running = f32::INFINITY;
    for &h in &axis {
        running = running.min(h - spec.water_offset);
        surface.push(running);
    }

    // Confluência rio→lago: dentro do contorno de água de um lago, a
    // superfície SOBE à cota do espelho — o prefix-min descia para dentro
    // da taça e punha um degrau visível no encontro das duas lâminas.
    // (water_y vem do REGISTRY: re-amostrar a grid pós-carve lia o fundo.)
    for lake in lakes {
        if lake.kind != WaterKind::Lake || lake.radius <= 0.0 {
            continue;
        }
        let phases = shape_phases(lake.at);
        for (i, st) in stations.iter().enumerate() {
            let theta = (st.y - lake.at.y).atan2(st.x - lake.at.x);
            let contour = lake_shape_radius(lake.radius, theta, phases);
            if st.distance(lake.at) <= contour {
                surface[i] = surface[i].max(lake.water_y);
            }
        }
    }
    // O raise da confluência pode INVERTER o prefix-min: uma estação
    // elevada à cota do lago fica ACIMA da vizinha de MONTANTE (as estações
    // correm da fonte para a foz) — rampa ascendente, a água "subia" entre
    // estações. A confluência é um remanso: a cota propaga-se a montante
    // até repor a descida (água nunca sobe no sentido do fluxo). Sem
    // lagos o passe é um no-op — o prefix-min já é descendente.
    for i in (1..surface.len()).rev() {
        if surface[i] > surface[i - 1] {
            surface[i - 1] = surface[i];
        }
    }

    // Poços e rápidos: profundidade e meia-largura do canal modulam-se ao
    // longo do percurso (determinístico pela posição do path). A SUPERFÍCIE
    // mantém o prefix-min liso — o LEITO ondula por baixo, e a profundidade
    // variável aparece no shader (absorção/espuma por depth prepass).
    let phase = river_phase(&spec.path);
    let mut depths: Vec<f32> = Vec::with_capacity(stations.len());
    let mut halfs: Vec<f32> = Vec::with_capacity(stations.len());
    let mut arc = 0.0f32;
    for (i, st) in stations.iter().enumerate() {
        if i > 0 {
            arc += st.distance(stations[i - 1]);
        }
        if spec.pool_spacing > 0.0 {
            let wave = (std::f32::consts::TAU * arc / spec.pool_spacing + phase).sin();
            // Poços ×1.6, rápidos ×0.4 (clampado acima do mínimo útil);
            // largura ±20% — rápidos apertam, poços abrem.
            depths.push((spec.depth * (1.0 + 0.6 * wave)).max(spec.depth * 0.4));
            halfs.push(half * (1.0 + 0.2 * (wave + 0.9).sin()));
        } else {
            depths.push(spec.depth);
            halfs.push(half);
        }
    }

    // Cascatas: queda > CASCADE_DROP entre estações vizinhas marca a
    // cascata; quedas contíguas (o Chaikin espalha a escarpa por 2-3
    // estações) são UMA queda — termina na primeira estação sem grande
    // queda, e é aí que o caldeirão se escava (×1.6 → ×1.25).
    let mut plunge = vec![1.0f32; stations.len()];
    let mut cascades = Vec::new();
    if spec.cascades {
        let n = stations.len();
        let mut in_fall = false;
        let mut last_lip = 0usize;
        for i in 0..n.saturating_sub(1) {
            let drop = surface[i] - surface[i + 1];
            if drop > CASCADE_DROP {
                if !in_fall {
                    cascades.push(i);
                    in_fall = true;
                }
                last_lip = i;
            } else if in_fall {
                in_fall = false;
                for (k, f) in [(1usize, 1.6f32), (2usize, 1.25)] {
                    if let Some(p) = plunge.get_mut(last_lip + k) {
                        *p = (*p).max(f);
                    }
                }
            }
        }
        if in_fall {
            for (k, f) in [(1usize, 1.6f32), (2usize, 1.25)] {
                if let Some(p) = plunge.get_mut((last_lip + k).min(n - 1)) {
                    *p = (*p).max(f);
                }
            }
        }
    }
    // Nascente: a água "sai" da rocha — a estação 0 ganha um poozinho
    // mais fundo (a boca da nascente) para a lâmina arrancar submersa.
    if spec.spring {
        plunge[0] = plunge[0].max(1.35);
        if let Some(p) = plunge.get_mut(1) {
            *p = (*p).max(1.2);
        }
    }
    for (depth, p) in depths.iter_mut().zip(&plunge) {
        *depth *= p;
    }

    // Adaptive outer feather, per station. At the outer bank rim the design
    // profile is the water surface, so the hillside there is cut by
    // `axis - surface` meters. A fixed feather would spread that whole cut
    // over 2.5 m — a ~33 m cliff where the path crosses a ridge, which is
    // exactly the vertical fin that used to line the banks. Grading the band
    // with the cut (the same rule `roads` already uses for its falloff) turns
    // the gorge walls into slopes; the `bank` style reshapes the band
    // (praia larga, parede estreita, terraços — [`BankStyle`]). Os estilos
    // VOXEL (gorge/overhang) ficam SEM rampa: o sólido natural fica de pé e
    // a parede é cortada por mods (`voxel/riverbank.rs`).
    let style = spec.bank;
    let feathers: Vec<f32> = if style.is_voxel() {
        vec![0.0; stations.len()]
    } else {
        axis.iter()
            .zip(surface.iter())
            .map(|(&a, &s)| style.band(feather_base, (a - s).max(0.0)))
            .collect()
    };
    let feather_max = feathers.iter().copied().fold(feather_base, f32::max);

    // The registry radius stays the *water* reach (the `avoid-water` zone):
    // the graded slopes beyond the bank are ordinary terrain, not river —
    // o feather (base ou adaptativo) NÃO entra no raio, senão os spawners
    // evitavam a encosta graduada que o próprio contrato declara terreno.
    let reach = half + bank;
    let extent = half + bank + feather_max + texel * 2.0;
    let owner = format!("river:{index}");

    // Bank + channel profile at a point: the pass-2 design surface. The bank
    // band rises to `surf + bank_height` (capped against the axis height so
    // deep valleys stay cascades), the channel bowls under the surface.
    // Profundidade e meia-largura vêm das estações (pools/cascatas).
    let channel_profile = {
        let stations = &stations;
        let surface = &surface;
        let axis = &axis;
        let depths = &depths;
        let halfs = &halfs;
        move |p: Vec2| -> f32 {
            let hit = nearest_on_path(stations, p).expect("stations >= 2");
            let d = hit.point.distance(p);
            // Interpolated along the path: reading `surface[seg]` directly
            // steps by a whole station at every segment boundary, and the
            // lower-only pass then cuts that step into a vertical fin.
            let surf = station_lerp(surface, &hit);
            let axis_h = station_lerp(axis, &hit);
            let depth = station_lerp(depths, &hit);
            let half_local = station_lerp(halfs, &hit).max(texel * 0.5);
            if d <= half_local {
                let t = d / half_local;
                surf - depth.max(MIN_CHANNEL_DEPTH) * (1.0 - t * t).max(0.0).powf(1.5)
            } else {
                let band = ((d - half_local) / bank.max(1e-4)).clamp(0.0, 1.0);
                let raised = surf + bank_height * smoothstep01(1.0 - band);
                raised.min(axis_h + MAX_BANK_RAISE)
            }
        }
    };

    // Pass 1 — banks (raise): fills the near bank band up to the profile.
    // Estilos voxel saltam: a margem mantém o terreno natural (o mod é que
    // corta a parede) — levantar bancos aqui punha terra SOLTA onde a
    // parede tem de ser rocha.
    if !style.is_voxel() {
        grid.begin_stroke(&format!("{owner}:banks"));
        let mut bank_weight = |p: Vec2| {
            let hit = nearest_on_path(&stations, p).expect("stations >= 2");
            let d = hit.point.distance(p);
            let half_local = station_lerp(&halfs, &hit);
            let feather = station_lerp(&feathers, &hit);
            if d <= half_local || d > half_local + bank + feather {
                return 0.0;
            }
            let band = ((d - half_local) / bank.max(1e-4)).clamp(0.0, 1.0);
            let mut w = smoothstep01(1.0 - band);
            if d > half_local + bank {
                w *= 1.0 - smoothstep01((d - half_local - bank) / feather);
            }
            w
        };
        let mut bank_target = |p: Vec2| channel_profile(p);
        river_apply(
            grid,
            &stations,
            extent,
            &mut bank_target,
            &mut bank_weight,
            BrushMode::Raise,
        );
        grid.commit_stroke();
    }

    // Pass 2 — channel + bank cut (lower-only): bowls the channel through the
    // water surface and cuts the hillside down to the bank profile.
    grid.begin_stroke(&owner);
    let mut channel_weight = |p: Vec2| {
        let hit = nearest_on_path(&stations, p).expect("stations >= 2");
        let d = hit.point.distance(p);
        let half_local = station_lerp(&halfs, &hit).max(texel * 0.5);
        let feather = station_lerp(&feathers, &hit);
        if d > half_local + bank + feather {
            return 0.0;
        }
        if d > half_local + bank {
            let t = (d - half_local - bank) / feather.max(1e-4);
            return 1.0 - style.progress(t);
        }
        1.0
    };
    let mut channel_target = |p: Vec2| channel_profile(p);
    // Deliberately **no guard**. The guard clamp is a flat-design device: it
    // exists so a pad/road bed has no bilinear-stencil lip just outside its
    // falloff, where the design surface is still meaningful. A river has no
    // design surface out there — past the bank band `channel_profile` decays
    // to the bare water surface, tens of meters under the hillside it is
    // crossing. Since the guard only ever visits texels the main pass left
    // unweighted (everything beyond `half + bank + feather`), wiring it up
    // here did nothing *except* stamp that water height into a two-texel
    // column at the footprint edge — the vertical fins along every bank.
    river_apply(
        grid,
        &stations,
        extent,
        &mut channel_target,
        &mut channel_weight,
        BrushMode::Lower,
    );
    grid.commit_stroke();

    let water_y = surface.iter().sum::<f32>() / surface.len() as f32;
    // O registry guarda a meia-largura EFETIVA da linha de água — a ribbon
    // e as queries de água (`surface_y_at`/`distance_to_waterline`) têm de
    // ler a MESMA métrica. O reach de cada estação segue a profundidade
    // EFETIVA (`depths[i]`, pós-poços/rápidos/plunge — o mesmo vetor do
    // perfil do canal): com o reach da profundidade NOMINAL, os rápidos
    // rasos estendiam a ribbon 1.5-2.7 m para além da linha de água real e
    // a lâmina cortava a margem; os poços e o caldeirão abrem na mesma
    // proporção. `depths` fica no registry para consumidores e testes
    // partilharem a métrica.
    let reaches: Vec<f32> = depths
        .iter()
        .map(|&d| waterline_reach(d, spec.water_offset).clamp(0.4, 1.0))
        .collect();
    Some(WaterBody {
        kind: WaterKind::River,
        at: centroid(&stations),
        radius: 0.0,
        carve_radius: reach,
        water_y,
        mirror_reach: 1.0,
        stations,
        surface_y: surface,
        water_width: width,
        half_width: halfs
            .iter()
            .zip(&reaches)
            .map(|(&h, &r)| h * r)
            .collect(),
        depths,
        cascades,
    })
}

/// One river carve pass: AABB over all stations; the closures spatially
/// filter (nearest-on-path distance).
fn river_apply(
    grid: &mut BrushGrid,
    stations: &[Vec2],
    extent: f32,
    target: &mut dyn FnMut(Vec2) -> f32,
    weight: &mut dyn FnMut(Vec2) -> f32,
    mode: BrushMode,
) {
    let (min_x, min_z, max_x, max_z) = stations.iter().fold(
        (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ),
        |(x0, z0, x1, z1), p| (x0.min(p.x), z0.min(p.y), x1.max(p.x), z1.max(p.y)),
    );
    grid.apply(BrushRequest {
        mode,
        min_x: min_x - extent,
        min_z: min_z - extent,
        max_x: max_x + extent,
        max_z: max_z + extent,
        target,
        weight,
    });
}

fn centroid(points: &[Vec2]) -> Vec2 {
    points.iter().sum::<Vec2>() / points.len().max(1) as f32
}

/// In-place moving average over a window of `±half` entries (edges clamp).
fn box_smooth(values: &mut [f32], half: usize) {
    if half == 0 || values.len() < 3 {
        return;
    }
    let n = values.len();
    let smoothed: Vec<f32> = (0..n)
        .map(|i| {
            let a = i.saturating_sub(half);
            let b = (i + half + 1).min(n);
            values[a..b].iter().sum::<f32>() / (b - a) as f32
        })
        .collect();
    values.copy_from_slice(&smoothed);
}

/// Builds the lake water surface mesh: a 72-segment two-ring disc following
/// the organic contour. Positions are **world space**. `water_y` is the mirror
/// height resolved by the carve ([`WaterBody::water_y`]) so the mesh always
/// matches the registry.
///
/// Vertex contract shared with `water.wgsl`: colour rgb = body colour,
/// colour **alpha = geometric shore mask only**, and
/// **uv.x = [`LakeSpec::opacity`]** — the shader reads it as the extinction
/// scale of the water column, so `opacity` now controls how *murky* the body
/// is instead of flattening a constant transparency over the whole mirror.
pub fn lake_water_mesh(spec: &LakeSpec, water_y: f32) -> ChunkMeshData {
    let phases = shape_phases(spec.at);
    let y = water_y;
    let murk = spec.opacity;
    // O espelho acaba na linha de água REAL da taça (onde o perfil cruza a
    // lâmina), não no contorno nominal: nas taludes fundos a taça continua
    // molhada para lá do contorno, nos lagos rasos o contorno já é praia
    // seca. Sem isto sobrava um anel de leito submerso sem água (ou lâmina
    // sobre areia seca, nos rasos). O MESMO fator vive no registry
    // (`WaterBody::mirror_reach`) — ver [`lake_mirror_reach`].
    let reach = lake_mirror_reach(spec.depth, spec.water_offset);
    let push = |mesh: &mut ChunkMeshData, p: Vec2, radial: f32, mask: f32| {
        mesh.positions.push([p.x, y, p.y]);
        mesh.normals.push([0.0, 1.0, 0.0]);
        mesh.uvs.push([murk, radial]);
        mesh.colors
            .push([spec.color[0], spec.color[1], spec.color[2], mask]);
    };

    let mut mesh = ChunkMeshData::default();
    // Fade de ILHAS: a lâmina some sobre cada ilha (a praia da ilha é o
    // próprio domo do carve — o espelho não pode cortá-la).
    let island_mask = |p: Vec2| -> f32 {
        let mut a = 1.0f32;
        for island in &spec.islands {
            let d = p.distance(island.at);
            // 0 dentro de 0.75·raio, 1 além de raio + 0.8 m (praia seca).
            let t = ((d - island.radius * 0.75) / (island.radius * 0.25 + 0.8)).clamp(0.0, 1.0);
            a *= smoothstep01(t);
        }
        a
    };
    // Center vertex.
    push(&mut mesh, spec.at, 0.5, 1.0 * island_mask(spec.at));
    // Two rings: inner unmasked, outer faded to the shore.
    let ring_count = 2;
    for ring in 0..ring_count {
        let radial = (ring + 1) as f32 / ring_count as f32; // 0.5, 1.0
        for i in 0..LAKE_FAN_SEGMENTS {
            let theta = i as f32 / LAKE_FAN_SEGMENTS as f32 * std::f32::consts::TAU;
            let r = lake_shape_radius(spec.radius, theta, phases) * reach * radial;
            let p = spec.at + Vec2::new(theta.cos(), theta.sin()) * r;
            let mask = (1.0 - smoothstep01((radial - (1.0 - WATER_EDGE_FADE)) / WATER_EDGE_FADE))
                * island_mask(p);
            push(&mut mesh, p, radial, mask);
        }
    }
    // Center fan.
    let seg = LAKE_FAN_SEGMENTS as u32;
    let inner0 = 1u32;
    for i in 0..LAKE_FAN_SEGMENTS as u32 {
        let a = inner0 + i;
        let b = inner0 + (i + 1) % seg;
        mesh.indices.extend_from_slice(&[0, b, a]);
    }
    // Ring band: inner ring [1..73), outer ring [73..145).
    let outer0 = 1 + LAKE_FAN_SEGMENTS as u32;
    for i in 0..LAKE_FAN_SEGMENTS as u32 {
        let a = inner0 + i;
        let b = inner0 + (i + 1) % seg;
        let c = outer0 + i;
        let d = outer0 + (i + 1) % seg;
        mesh.indices.extend_from_slice(&[a, b, c, b, d, c]);
    }
    mesh
}

/// Builds the river water ribbon: three vertices per station (left/center/
/// right of the centerline), `y` per station surface, the shore mask fading
/// over the outer quarter of the width. Positions are **world space**.
///
/// Same vertex contract as [`lake_water_mesh`]: colour alpha is the shore mask
/// and **uv.x carries [`RiverSpec::opacity`]** as the extinction scale.
pub fn river_water_mesh(spec: &RiverSpec, body: &WaterBody) -> ChunkMeshData {
    let mut mesh = ChunkMeshData::default();
    let n = body.stations.len();
    if n < 2 {
        return mesh;
    }
    // A meia-largura de cada estação vem do registry (`half_width_at`) — o
    // carve já a guarda EFETIVA (escalada pelo reach da linha de água, ver
    // [`waterline_reach`]); pools abrem, rápidos apertam.
    let murk = spec.opacity;
    // Three vertices per station (left, center, right): the shore mask fades
    // on the outer quarter of each half, so the center stays unmasked.
    let mask_at = |v: f32| -> f32 {
        let edge = (((v - 0.5).abs() * 2.0 - (1.0 - WATER_EDGE_FADE * 2.0)).max(0.0)
            / (WATER_EDGE_FADE * 2.0))
            .clamp(0.0, 1.0);
        1.0 - edge
    };
    for (i, st) in body.stations.iter().enumerate() {
        let next = body.stations[(i + 1).min(n - 1)];
        let prev = body.stations[i.saturating_sub(1)];
        let dir = (next - prev).normalize_or_zero();
        let perp = Vec2::new(-dir.y, dir.x);
        let y = body.surface_y[i];
        // Meia-largura EFETIVA da estação (pools abrem, rápidos apertam).
        let half_i = body.half_width_at(i);
        for v in [0.0_f32, 0.5, 1.0] {
            let p = *st + perp * (half_i * (v * 2.0 - 1.0));
            mesh.positions.push([p.x, y, p.y]);
            mesh.normals.push([0.0, 1.0, 0.0]);
            mesh.uvs.push([murk, v]);
            mesh.colors
                .push([spec.color[0], spec.color[1], spec.color[2], mask_at(v)]);
        }
    }
    for i in 0..(n - 1) {
        // Cascade: em vez da rampa linear entre cotas, FACE VERTICAL — a
        // água despenca do lip (borda de montante) até à lâmina do
        // caldeirão. Mesmo contrato de vértices (uv.x murk, alpha=1: uma
        // cascata não tem margem seca); normais horizontais (a face é
        // vertical; o shader usa a geométrica para o fresnel rasante).
        if body.cascades.contains(&i) {
            let st0 = body.stations[i];
            let st1 = body.stations[i + 1];
            let dir = (st1 - st0).normalize_or_zero();
            let perp = Vec2::new(-dir.y, dir.x);
            // A face pende ligeiramente para jusante: borda de topo a 75 %
            // do segmento, base na estação do caldeirão.
            let top_c = st0.lerp(st1, 0.75);
            let half0 = body.half_width_at(i);
            let half1 = body.half_width_at(i + 1);
            let top_y = body.surface_y[i];
            let bot_y = body.surface_y[i + 1];
            let base = mesh.positions.len() as u32;
            for (c, h, y) in [(top_c, half0, top_y), (st1, half1, bot_y)] {
                for v in [0.0_f32, 0.5, 1.0] {
                    let p = c + perp * (h * (v * 2.0 - 1.0));
                    mesh.positions.push([p.x, y, p.y]);
                    mesh.normals.push([dir.x, 0.0, dir.y]);
                    mesh.uvs.push([murk, v]);
                    mesh.colors.push([spec.color[0], spec.color[1], spec.color[2], 1.0]);
                }
            }
            // Verso com normal PRÓPRIA: os 4 triângulos de trás partilhavam
            // os vértices da face — normal horizontal a apontar SEMPRE a
            // jusante — e liam-se como painel escuro visto de montante.
            // Duplicam-se os 6 vértices com a normal achatada para +Y: o
            // verso ilumina como a própria lâmina; de jusante nada muda (a
            // face mantém a horizontal e o fresnel rasante).
            let back = mesh.positions.len() as u32;
            for k in 0..6usize {
                let j = base as usize + k;
                mesh.positions.push(mesh.positions[j]);
                mesh.normals.push([0.0, 1.0, 0.0]);
                mesh.uvs.push(mesh.uvs[j]);
                mesh.colors.push(mesh.colors[j]);
            }
            let (l0, c0, r0) = (base, base + 1, base + 2);
            let (l1, c1, r1) = (base + 3, base + 4, base + 5);
            let (b0, bc0, br0) = (back, back + 1, back + 2);
            let (b1, bc1, br1) = (back + 3, back + 4, back + 5);
            mesh.indices.extend_from_slice(&[
                l0, l1, c0, c0, l1, c1, // left half (visto de jusante)
                c0, c1, r0, r0, c1, r1, // right half
                b1, b0, bc1, bc1, b0, bc0, // verso (cull_mode None) com os
                bc1, bc0, br1, br1, bc0, br0, // vértices próprios (+Y)
            ]);
            continue;
        }
        // Vertices: 3 per station (l, c, r). Four CCW triangles per segment.
        let l0 = (i * 3) as u32;
        let c0 = l0 + 1;
        let r0 = l0 + 2;
        let l1 = ((i + 1) * 3) as u32;
        let c1 = l1 + 1;
        let r1 = l1 + 2;
        mesh.indices.extend_from_slice(&[
            l0, l1, c0, c0, l1, c1, // left half
            c0, c1, r0, r0, c1, r1, // right half
        ]);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 96x96 grid, 96 m world, rolling hill profile.
    fn test_grid() -> BrushGrid {
        let mut grid = BrushGrid::new(vec![0; 96 * 96], 96, 96, 96.0, 50.0, 0.0).expect("grid");
        grid.begin_stroke("base");
        for z in 0..96 {
            for x in 0..96 {
                let p = grid.cell_center(x, z);
                let h = 12.0 + 4.0 * (p.x * 0.05).sin() + 3.0 * (p.y * 0.07).cos();
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        grid
    }

    /// Flat 96x96 grid at 8 m for predictable carves.
    fn flat_grid() -> BrushGrid {
        let mut grid = BrushGrid::new(vec![0; 96 * 96], 96, 96, 96.0, 50.0, 0.0).expect("grid");
        grid.begin_stroke("flat");
        for i in 0..96 * 96 {
            grid.set_cell_height(i % 96, i / 96, 8.0);
        }
        grid.commit_stroke();
        grid
    }

    #[test]
    fn test_defaults_match_vibegame() {
        let lake = LakeSpec::default();
        assert_eq!(lake.radius, 6.0);
        assert_eq!(lake.depth, 1.5);
        assert_eq!(lake.water_offset, 0.5);
        assert!((lake.opacity - 0.75).abs() < 1e-6);
        let river = RiverSpec::default();
        assert_eq!(river.width, 6.0);
        assert_eq!(river.bank_width, 2.0);
        assert!((river.bank_height - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_lake_shape_radius_varies_and_is_deterministic() {
        let phases = shape_phases(Vec2::new(3.0, -7.0));
        assert_eq!(
            lake_shape_radius(10.0, 0.3, phases),
            lake_shape_radius(10.0, 0.3, phases),
            "same position -> same contour"
        );
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for i in 0..64 {
            let r = lake_shape_radius(10.0, i as f32 / 64.0 * std::f32::consts::TAU, phases);
            min = min.min(r);
            max = max.max(r);
        }
        assert!(
            min < 10.0 * 0.85 && max > 10.0 * 1.15,
            "±28% contour: {min}..{max}"
        );
        assert!(min > 10.0 * 0.6 && max < 10.0 * 1.4, "bounded variation");
    }

    #[test]
    fn test_contour_peak_covers_the_harmonics() {
        // O AABB do carve usa CONTOUR_PEAK; tem de cobrir a soma real das
        // amplitudes dos harmónicos (e um pouco de folga pelo arredondamento).
        let peak = 1.0 + SHAPE_AMPLITUDES.iter().sum::<f32>();
        assert!(
            CONTOUR_PEAK + 1e-6 >= peak,
            "CONTOUR_PEAK {CONTOUR_PEAK} < pico real {peak}"
        );
    }

    #[test]
    fn test_carve_lake_lowers_inside_leaves_outside() {
        let mut grid = test_grid();
        let before = grid.sample(-40.0, -40.0);
        let spec = LakeSpec {
            at: Vec2::new(30.0, 30.0),
            radius: 12.0,
            depth: 3.0,
            water_offset: 0.5,
            ..LakeSpec::default()
        };
        let body = carve_lake(&mut grid, &spec, 0).expect("lake");
        let center = grid.sample(30.0, 30.0);
        assert!(
            center < body.water_y,
            "bowl floor sits below the mirror: {center} vs {}",
            body.water_y
        );
        assert!(
            (before - grid.sample(-40.0, -40.0)).abs() < 1e-3,
            "terrain outside the carve is untouched"
        );
        assert!(body.carve_radius > spec.radius, "carve margin applied");
        // After the carve the rim ring sits on discretized texels slightly
        // below the pre-carve rim (bowl C1 tail); tolerate the artifact.
        // Depth lands in the bowl between the mirror and rim + offset.
        let depth_reached = body.water_y + spec.water_offset - grid.sample(30.0, 30.0);
        assert!(
            (depth_reached - spec.depth).abs() < 0.15,
            "bowl depth at center: {depth_reached}"
        );
    }

    #[test]
    fn test_carve_lake_is_lower_only() {
        // Overlap the lake with an existing deep valley: the valley must not
        // be filled in.
        let mut grid = test_grid();
        grid.begin_stroke("valley");
        for z in 0..96 {
            for x in 0..96 {
                let p = grid.cell_center(x, z);
                if p.distance(Vec2::new(20.0, 20.0)) < 8.0 {
                    grid.set_cell_height(x, z, 1.0);
                }
            }
        }
        grid.commit_stroke();
        let spec = LakeSpec {
            at: Vec2::new(20.0, 20.0),
            radius: 12.0,
            depth: 2.0,
            ..LakeSpec::default()
        };
        let _ = carve_lake(&mut grid, &spec, 0).expect("lake");
        let valley = grid.sample(20.0, 20.0);
        assert!(
            valley < 2.0,
            "pre-existing valley is never raised: {valley}"
        );
    }

    #[test]
    fn test_carve_lake_degenerate_is_none() {
        let mut grid = test_grid();
        let spec = LakeSpec {
            radius: 0.0,
            ..LakeSpec::default()
        };
        assert!(carve_lake(&mut grid, &spec, 0).is_none());
        let spec = LakeSpec {
            depth: 0.0,
            ..LakeSpec::default()
        };
        assert!(carve_lake(&mut grid, &spec, 0).is_none());
    }

    fn river_spec() -> RiverSpec {
        RiverSpec {
            path: vec![Vec2::new(5.0, 0.0), Vec2::new(90.0, 0.0)],
            width: 6.0,
            depth: 2.0,
            water_offset: 0.4,
            bank_width: 2.5,
            bank_height: 0.8,
            color: [0.2, 0.4, 0.5],
            opacity: 0.85,
            bank: BankStyle::Soft,
            rocks: false,
            rocks_spec: crate::terrain::shore_rocks::ShoreRocksSpec::default(),
            pool_spacing: 0.0,
            cascades: true,
            spring: false,
        }
    }

    #[test]
    fn test_carve_river_creates_a_channel_below_banks() {
        // Flat ground keeps the axis heights predictable (a rolling grid plus
        // the descending prefix-min would carve to the global minimum).
        let mut grid = flat_grid();
        let spec = river_spec();
        let body = carve_river(&mut grid, &spec, 0, &[]).expect("river");
        let channel = grid.sample(48.0, 0.0);
        let bank = grid.sample(48.0, body.water_width * 0.5 + 1.0);
        assert!(
            channel < body.water_y,
            "channel floor below the surface: {channel} vs {}",
            body.water_y
        );
        assert!(
            bank > channel,
            "bank sits above the channel floor: {bank} vs {channel}"
        );
        // Far from the river the base terrain survives.
        let far = grid.sample(48.0, -32.0);
        assert!((far - 8.0).abs() < 0.05, "far terrain untouched: {far}");
    }

    #[test]
    fn test_river_surface_never_rises_downstream() {
        let mut grid = test_grid();
        let spec = RiverSpec {
            path: vec![Vec2::new(5.0, 10.0), Vec2::new(90.0, -20.0)],
            ..river_spec()
        };
        let body = carve_river(&mut grid, &spec, 0, &[]).expect("river");
        for w in body.surface_y.windows(2) {
            assert!(
                w[1] <= w[0] + 1e-4,
                "descending prefix min: {} -> {}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn test_carve_river_degenerate_is_none() {
        let mut grid = test_grid();
        let spec = RiverSpec {
            path: vec![Vec2::ZERO],
            ..river_spec()
        };
        assert!(carve_river(&mut grid, &spec, 0, &[]).is_none());
    }

    #[test]
    fn test_water_body_queries() {
        let mut grid = test_grid();
        let lake = carve_lake(
            &mut grid,
            &LakeSpec {
                at: Vec2::new(30.0, 30.0),
                radius: 10.0,
                ..LakeSpec::default()
            },
            0,
        )
        .expect("lake");
        assert!(lake.contains(Vec2::new(32.0, 30.0)), "inside the carve");
        assert!(!lake.contains(Vec2::new(46.0, 30.0)));
        assert!(
            lake.is_near(Vec2::new(46.0, 30.0), 20.0),
            "margin reaches past the carve edge"
        );
        assert!(!lake.is_near(Vec2::new(44.0, -44.0), 5.0));
        assert!(lake.surface_y_at(Vec2::new(31.0, 30.0)).is_some());
        assert!(lake.surface_y_at(Vec2::new(44.0, -44.0)).is_none());

        let spec = river_spec();
        let river = carve_river(&mut grid, &spec, 1, &[]).expect("river");
        assert!(river.contains(Vec2::new(48.0, 1.0)), "on the river");
        assert!(!river.contains(Vec2::new(48.0, 22.0)), "off the river");
        assert!(river.surface_y_at(Vec2::new(48.0, 0.5)).is_some());
        assert!(river.surface_y_at(Vec2::new(48.0, 22.0)).is_none());
    }

    #[test]
    fn test_lake_water_mesh_rings_and_fade() {
        let mut grid = test_grid();
        let spec = LakeSpec {
            at: Vec2::new(40.0, 40.0),
            radius: 10.0,
            ..LakeSpec::default()
        };
        let body = carve_lake(&mut grid, &spec, 0).expect("lake");
        let mesh = lake_water_mesh(&spec, body.water_y);
        let expected = 1 + LAKE_FAN_SEGMENTS * 2;
        assert_eq!(mesh.positions.len(), expected, "center + two rings");
        assert_eq!(mesh.indices.len(), LAKE_FAN_SEGMENTS * 9, "fan + band");
        let center = mesh.positions[0];
        assert!(
            (center[0] - 40.0).abs() < 1e-4 && (center[2] - 40.0).abs() < 1e-4,
            "fan is centered on the lake"
        );
        assert!(
            (center[1] - body.water_y).abs() < 1e-3,
            "mirror at the rim offset"
        );
        // Outer ring follows the contour radius and fades out.
        let outer0 = 1 + LAKE_FAN_SEGMENTS;
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for p in &mesh.positions[outer0..] {
            let d = ((p[0] - 40.0).powi(2) + (p[2] - 40.0).powi(2)).sqrt();
            min = min.min(d);
            max = max.max(d);
        }
        assert!(min < 10.0 && max > 10.0, "organic contour: {min}..{max}");
        let center_alpha = mesh.colors[0][3];
        let outer_alpha = mesh.colors[outer0][3];
        assert!(
            outer_alpha < center_alpha * 0.2,
            "shore fades to ~0: {outer_alpha} vs {center_alpha}"
        );
        // Novo contrato: alpha = SÓ máscara (1 no centro) e uv.x = opacity,
        // que o shader lê como escala de extinção da coluna.
        assert!((center_alpha - 1.0).abs() < 1e-6, "mask, not opacity");
        for uv in &mesh.uvs {
            assert!((uv[0] - spec.opacity).abs() < 1e-6, "uv.x carries opacity");
        }
    }

    #[test]
    fn test_river_water_mesh_ribbon() {
        let mut grid = test_grid();
        let spec = river_spec();
        let body = carve_river(&mut grid, &spec, 0, &[]).expect("river");
        let mesh = river_water_mesh(&spec, &body);
        assert_eq!(mesh.positions.len(), body.stations.len() * 3, "l/c/r");
        assert_eq!(mesh.indices.len(), (body.stations.len() - 1) * 12);
        for (i, p) in mesh.positions.iter().enumerate() {
            let st = body.stations[i / 3];
            // A ribbon acaba na linha de água real do canal (t_wl da taça).
            let side = body.water_width
                * 0.5
                * waterline_reach(spec.depth, spec.water_offset).clamp(0.4, 1.0);
            let side = match i % 3 {
                0 | 2 => side,
                _ => 0.0, // center vertex rides the surface
            };
            let d = ((p[0] - st.x).powi(2) + (p[2] - st.y).powi(2)).sqrt();
            assert!((d - side).abs() < 1e-3, "vertex {i} at expected offset");
        }
        let edge = mesh.colors[0][3];
        let mid = mesh.colors[1][3];
        assert!(edge < mid, "edge fades: {edge} vs {mid}");
        assert!((mid - 1.0).abs() < 1e-4, "center unmasked: {mid}");
        for uv in &mesh.uvs {
            assert!((uv[0] - spec.opacity).abs() < 1e-6, "uv.x carries opacity");
        }
    }

    /// A face da cascata: o mesh ganha 12 vértices verticais por cascata —
    /// 6 da FACE (topo no lip, base na lâmina do caldeirão, normais
    /// horizontais a apontar a jusante) + 6 do VERSO (mesma geometria,
    /// normal achatada para +Y, para não ler como painel escuro visto de
    /// montante).
    #[test]
    fn test_cascade_face_is_vertical_in_the_mesh() {
        let mut grid = BrushGrid::new(vec![0; 96 * 96], 96, 96, 96.0, 50.0, 0.0).expect("grid");
        grid.begin_stroke("plateau");
        for z in 0..96 {
            for x in 0..96 {
                let p = grid.cell_center(x, z);
                let h = if p.x < 0.0 { 14.0 } else { 5.0 };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        let spec = RiverSpec {
            path: vec![Vec2::new(-40.0, 0.0), Vec2::new(40.0, 0.0)],
            width: 6.0,
            depth: 1.5,
            ..river_spec()
        };
        let body = carve_river(&mut grid, &spec, 0, &[]).expect("river");
        assert_eq!(body.cascades.len(), 1);
        let mesh = river_water_mesh(&spec, &body);
        let n = body.stations.len();
        assert_eq!(
            mesh.positions.len(),
            n * 3 + 12,
            "ribbon verts + cascade face + back: {} vs {}",
            mesh.positions.len(),
            n * 3 + 12
        );
        // Os 6 vértices da FACE: topo = cota do lip, base = cota do
        // caldeirão, normais horizontais (face vertical).
        let lip = body.cascades[0];
        let face = &mesh.positions[n * 3..n * 3 + 6];
        for v in &face[0..3] {
            assert!((v[1] - body.surface_y[lip]).abs() < 1e-3, "top at the lip");
        }
        for v in &face[3..6] {
            assert!(
                (v[1] - body.surface_y[lip + 1]).abs() < 1e-3,
                "bottom at the plunge surface"
            );
        }
        for normal in &mesh.normals[n * 3..n * 3 + 6] {
            assert!(normal[1].abs() < 1e-6, "horizontal face normal");
        }
        // Os 6 do VERSO: mesma geometria, normal +Y.
        let back = &mesh.positions[n * 3 + 6..];
        assert_eq!(back.len(), 6, "duplicated back vertices");
        for (b, f) in back.iter().zip(face.iter()) {
            assert_eq!(b, f, "back shares the face geometry");
        }
        for normal in &mesh.normals[n * 3 + 6..] {
            assert!(
                (normal[1] - 1.0).abs() < 1e-6,
                "back normal flattened to +Y: {normal:?}"
            );
        }
    }

    #[test]
    fn test_min_effective_grows_tiny_rivers() {
        let grid = test_grid();
        let width = min_effective(0.1, grid.texel());
        assert!(width >= 1.5 * grid.texel(), "width never below 1.5 texels");
    }

    #[test]
    fn test_waterline_reach_resolves_bowl_crossing() {
        // depth·(1−t²)^1.5 = offset → t = √(1−(offset/depth)^⅔). Default
        // lake (1.5/0.5): ratio ≈ 0.481 → t ≈ 0.721; ·CARVE_MARGIN ≈ 0.90.
        let t = waterline_reach(1.5, 0.5);
        assert!((t - 0.7206).abs() < 2e-3, "shallow lake reach: {t}");
        // Lago fundo: a lâmina passa o contorno nominal (reach > 1
        // após a margem do carve).
        let deep = waterline_reach(3.2, 0.5) * CARVE_MARGIN;
        assert!(deep > 1.0, "deep lake wets past the contour: {deep}");
        // offset ≥ depth: taça não chega a cruzar → espelho colapsa a 0.
        assert!(
            waterline_reach(1.0, 1.0) < 1e-6,
            "no crossing → no mirror reach"
        );
    }

    /// Os estilos de margem têm bandas e perfis distintos: beach espalha a
    /// transição (banda larga, rampa lenta), cliff concentra-a (banda
    /// estreita, queda no terço médio). Testado nos próprios primitivos —
    /// comparar secções esculpidas depende das fases do contorno harmónico.
    #[test]
    fn test_bank_styles_shape_the_shore() {
        // Bandas com o mesmo corte (1.1 m) e feather base (2.5): beach
        // espalha, cliff aperta.
        let band = |s: BankStyle| s.band(2.5, 1.1);
        assert!(band(BankStyle::Beach) > band(BankStyle::Soft));
        assert!(band(BankStyle::Soft) > band(BankStyle::Cliff));
        assert!(band(BankStyle::Terraced) > band(BankStyle::Cliff));

        // Perfis: cliff segura o patamar de cima mais tempo (queda só no
        // terço médio — a 3/8 da banda mal desceu, soft já foi a 28%);
        // beach ainda está mais alto no início (rampa de slope zero).
        assert!(
            BankStyle::Cliff.progress(0.375) < BankStyle::Soft.progress(0.375),
            "cliff holds the crest: {:.3} vs soft: {:.3}",
            BankStyle::Cliff.progress(0.375),
            BankStyle::Soft.progress(0.375)
        );
        assert!(
            BankStyle::Cliff.progress(0.625) > BankStyle::Soft.progress(0.625),
            "cliff drops fast mid-band: {:.3} vs soft: {:.3}",
            BankStyle::Cliff.progress(0.625),
            BankStyle::Soft.progress(0.625)
        );
        assert!(
            BankStyle::Beach.progress(0.2) < BankStyle::Soft.progress(0.2),
            "beach holds the top longer: {:.3} vs {:.3}",
            BankStyle::Beach.progress(0.2),
            BankStyle::Soft.progress(0.2)
        );
        // Terraced: escadaria — progresso passa pelos degraus (≥ 2 níveis
        // distintos dentro da banda).
        let mut levels = 1;
        let mut last = 0.0;
        for i in 1..=10 {
            let p = BankStyle::Terraced.progress(i as f32 / 10.0);
            if p > last + 0.05 {
                levels += 1;
                last = p;
            }
        }
        assert!(levels >= 3, "terraced descends in steps: {levels}");
        // Todos terminam em 0/1 exatos.
        for s in [
            BankStyle::Soft,
            BankStyle::Beach,
            BankStyle::Cliff,
            BankStyle::Terraced,
            BankStyle::Gorge,
            BankStyle::Overhang,
        ] {
            assert!(s.progress(0.0).abs() < 1e-5);
            assert!((s.progress(1.0) - 1.0).abs() < 1e-5);
        }
        // Os estilos voxel declaram-se (e têm banda mínima — sem rampa).
        assert!(BankStyle::Gorge.is_voxel() && BankStyle::Overhang.is_voxel());
        assert!(!BankStyle::Soft.is_voxel() && !BankStyle::Beach.is_voxel());
    }

    /// Poços e rápidos: o LEITO ondula (profundidade variável) e a
    /// meia-largura acompanha, mas a superfície mantém o prefix-min
    /// descendente — a lâmina nunca lê a ondulação.
    #[test]
    fn test_pool_spacing_modulates_the_bed_not_the_surface() {
        let mut grid = flat_grid();
        let spec = RiverSpec {
            path: vec![Vec2::new(-40.0, 0.0), Vec2::new(40.0, 0.0)],
            width: 6.0,
            depth: 2.0,
            pool_spacing: 14.0,
            ..river_spec()
        };
        let body = carve_river(&mut grid, &spec, 0, &[]).expect("river");
        assert_eq!(body.half_width.len(), body.stations.len());
        // A meia-largura varia ao longo do rio (não é tudo o mesmo valor).
        let min_hw = body.half_width.iter().cloned().fold(f32::MAX, f32::min);
        let max_hw = body.half_width.iter().cloned().fold(f32::MIN, f32::max);
        assert!(
            max_hw - min_hw > 0.2,
            "half width must breathe: {min_hw}..{max_hw}"
        );
        // Superfície monotónica descendente (o prefix-min não leu os pools).
        for w in body.surface_y.windows(2) {
            assert!(w[1] <= w[0] + 1e-4, "surface still descends");
        }
        // E o LEITO: a profundidade ao centro varia entre estações.
        let bed = |i: usize| body.surface_y[i] - grid.sample(body.stations[i].x, 0.0);
        let (min_d, max_d) = (0..body.stations.len())
            .map(bed)
            .fold((f32::MAX, f32::MIN), |(a, b), d| (a.min(d), b.max(d)));
        assert!(
            max_d - min_d > 0.5,
            "bed depth must breathe (pools/riffles): {min_d}..{max_d}"
        );
    }

    /// O reach da linha de água segue a profundidade EFETIVA por estação:
    /// nos rápidos (rasos) a ribbon aperta — com o reach da profundidade
    /// NOMINAL estendia-se para além da linha de água real e cortava a
    /// margem — e nos poços abre na mesma proporção.
    #[test]
    fn test_river_ribbon_reach_follows_effective_depth() {
        let mut grid = flat_grid();
        let spec = RiverSpec {
            path: vec![Vec2::new(-40.0, 0.0), Vec2::new(40.0, 0.0)],
            width: 6.0,
            depth: 2.0,
            pool_spacing: 14.0,
            ..river_spec()
        };
        let body = carve_river(&mut grid, &spec, 0, &[]).expect("river");
        let mesh = river_water_mesh(&spec, &body);
        let n = body.stations.len();
        assert_eq!(body.depths.len(), n, "registry carries effective depths");

        // Meia-largura da ribbon por estação (vértice esquerdo).
        let ribbon_half = |i: usize| {
            let st = body.stations[i];
            Vec2::new(
                mesh.positions[i * 3][0] - st.x,
                mesh.positions[i * 3][2] - st.y,
            )
            .length()
        };
        // Meia-largura de DESIGN por estação (a fórmula dos poços/rápidos
        // do carve — superfície lisa, rio plano: sem plunge).
        let half = spec.width * 0.5;
        let phase = river_phase(&spec.path);
        let reach = |d: f32| waterline_reach(d, spec.water_offset).clamp(0.4, 1.0);
        let mut designs = Vec::with_capacity(n);
        let mut shallow = (f32::MAX, 0usize);
        let mut deep = (f32::MIN, 0usize);
        let mut arc = 0.0f32;
        for i in 0..n {
            if i > 0 {
                arc += body.stations[i].distance(body.stations[i - 1]);
            }
            let wave = (std::f32::consts::TAU * arc / spec.pool_spacing + phase).sin();
            designs.push(half * (1.0 + 0.2 * (wave + 0.9).sin()));
            let depth = body.depths[i];
            // A ribbon acaba em design × reach(profundidade EFETIVA).
            assert!(
                (ribbon_half(i) - designs[i] * reach(depth)).abs() < 1e-3,
                "station {i}: ribbon at the effective-depth waterline"
            );
            if depth < shallow.0 {
                shallow = (depth, i);
            }
            if depth > deep.0 {
                deep = (depth, i);
            }
        }
        // Riffle e pool de facto presentes (a profundidade efetiva respira).
        assert!(
            shallow.0 < spec.depth && deep.0 > spec.depth,
            "pool breathing present: riffle {}..pool {}",
            shallow.0,
            deep.0
        );
        let reach_nominal = reach(spec.depth);
        assert!(
            reach(shallow.0) < reach_nominal && reach(deep.0) > reach_nominal,
            "riffle narrows ({:.3} < {reach_nominal}), pool opens ({:.3})",
            reach(shallow.0),
            reach(deep.0)
        );
        // E o riffle apertou FACE ao comportamento antigo (reach nominal):
        // o bordo duro/enterrado dos rápidos desaparece.
        assert!(
            ribbon_half(shallow.1) < designs[shallow.1] * reach_nominal - 1e-3,
            "riffle ribbon tighter than the nominal reach: {:.3} vs {:.3}",
            ribbon_half(shallow.1),
            designs[shallow.1] * reach_nominal
        );
    }

    /// Cascata: queda > CASCADE_DROP marca um lip, o canal aprofunda a
    /// jusante (caldeirão) e o registry expõe a cascata.
    #[test]
    fn test_cascade_marks_lip_and_digs_plunge_pool() {
        // Planalto alto a oeste, baixo a leste: o prefix-min cai de uma vez
        // na escarpa → cascata garantida.
        let mut grid = BrushGrid::new(vec![0; 96 * 96], 96, 96, 96.0, 50.0, 0.0).expect("grid");
        grid.begin_stroke("plateau");
        for z in 0..96 {
            for x in 0..96 {
                let p = grid.cell_center(x, z);
                let h = if p.x < 0.0 { 14.0 } else { 5.0 };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        let spec = RiverSpec {
            path: vec![Vec2::new(-40.0, 0.0), Vec2::new(40.0, 0.0)],
            width: 6.0,
            depth: 1.5,
            ..river_spec()
        };
        let body = carve_river(&mut grid, &spec, 0, &[]).expect("river");
        assert_eq!(body.cascades.len(), 1, "one escarpment, one cascade");
        let lip = body.cascades[0];
        let drop = body.surface_y[lip] - body.surface_y[lip + 1];
        assert!(drop > CASCADE_DROP, "marked lip has the big drop: {drop}");
        // Caldeirão: alguma estação a jusante do lip é claramente mais
        // funda (À PRÓPRIA lâmina) que o canal normal de montante — o
        // caldeirão assenta no FIM da sequência de quedas (last_lip), não
        // no primeiro lip.
        let bed = |k: usize| body.surface_y[k] - grid.sample(body.stations[k].x, 0.0);
        let normal = bed(2); // montante: canal normal
        let last = body.stations.len() - 1;
        let pool = (lip..=(lip + 5).min(last)).map(bed).fold(f32::MIN, f32::max);
        assert!(
            pool > normal * 1.3,
            "plunge pool digs deeper: {pool} vs {normal}"
        );
    }

    /// Confluência: as estações do rio dentro do contorno do lago sobem à
    /// cota do espelho — nenhuma desce abaixo (sem degrau no encontro).
    #[test]
    fn test_confluence_rises_to_lake_level() {
        let mut grid = flat_grid();
        let lake_spec = LakeSpec {
            at: Vec2::new(0.0, 0.0),
            radius: 12.0,
            depth: 3.0,
            ..LakeSpec::default()
        };
        let lake = carve_lake(&mut grid, &lake_spec, 0).expect("lake");
        let river_spec = RiverSpec {
            path: vec![Vec2::new(-40.0, 0.0), Vec2::new(40.0, 0.0)],
            width: 6.0,
            depth: 2.0,
            ..river_spec()
        };
        let river = carve_river(&mut grid, &river_spec, 0, &[lake.clone()]).expect("river");
        // A estação mais próxima do centro do lago está À COTA do lago.
        let (_, i, _) = river
            .stations
            .iter()
            .enumerate()
            .fold((f32::MAX, 0usize, Vec2::ZERO), |(best, bi, _), (i, st)| {
                let d = st.distance(lake.at);
                if d < best {
                    (d, i, *st)
                } else {
                    (best, bi, *st)
                }
            });
        assert!(
            river.surface_y[i] >= lake.water_y - 1e-3,
            "river meets the lake level: {} vs {}",
            river.surface_y[i],
            lake.water_y
        );
    }

    /// Confluência com lago "alto": o raise à cota do espelho não pode
    /// criar rampa ASCENDENTE a montante — a cota propaga-se a montante
    /// (remanso) e a superfície mantém o prefix-min descendente. As
    /// estações dentro do contorno continuam à cota do lago.
    #[test]
    fn test_confluence_backwater_keeps_the_surface_descending() {
        // Vale a montante do lago mais BAIXO que o espelho do lago (7.5):
        // o prefix-min descia a 6.8 e o raise invertia o perfil entre a
        // última estação fora e a primeira dentro do contorno.
        let mut grid = BrushGrid::new(vec![0; 96 * 96], 96, 96, 96.0, 50.0, 0.0).expect("grid");
        grid.begin_stroke("vale");
        for z in 0..96 {
            for x in 0..96 {
                let p = grid.cell_center(x, z);
                grid.set_cell_height(x, z, if p.x < 20.0 { 7.2 } else { 8.0 });
            }
        }
        grid.commit_stroke();
        let lake_spec = LakeSpec {
            at: Vec2::new(40.0, 0.0),
            radius: 10.0,
            depth: 3.0,
            ..LakeSpec::default()
        };
        let lake = carve_lake(&mut grid, &lake_spec, 0).expect("lake");
        let spec = RiverSpec {
            path: vec![Vec2::new(-40.0, 0.0), Vec2::new(40.0, 0.0)],
            width: 6.0,
            depth: 2.0,
            ..river_spec()
        };
        let river = carve_river(&mut grid, &spec, 0, &[lake.clone()]).expect("river");
        // 1) A superfície nunca sobe de estação a estação.
        for w in river.surface_y.windows(2) {
            assert!(
                w[1] <= w[0] + 1e-4,
                "backwater keeps descent: {} -> {}",
                w[0],
                w[1]
            );
        }
        // 2) O raise sobrevive: dentro do contorno a superfície fica à cota
        // do espelho (o remanso não baixa as estações elevadas).
        let phases = shape_phases(lake.at);
        let mut inside = 0;
        for (st, s) in river.stations.iter().zip(&river.surface_y) {
            let theta = (st.y - lake.at.y).atan2(st.x - lake.at.x);
            if st.distance(lake.at) <= lake_shape_radius(lake.radius, theta, phases) {
                inside += 1;
                assert!(
                    *s >= lake.water_y - 1e-3,
                    "station inside the lake stays at the mirror: {s} vs {}",
                    lake.water_y
                );
            }
        }
        assert!(inside > 0, "the path must cross the lake contour");
    }

    /// Ilhas: o domo sobe acima da lâmina, o espelho some sobre ela (alpha
    /// 0 no centro) e ilha fora da bacia é ignorada.
    #[test]
    fn test_island_raises_and_mirror_fades() {
        let mut grid = flat_grid();
        let spec = LakeSpec {
            at: Vec2::new(0.0, 0.0),
            radius: 14.0,
            depth: 3.0,
            islands: vec![crate::terrain::water::IslandSpec {
                at: Vec2::new(0.0, 0.0),
                radius: 4.0,
                height: 1.5,
            }],
            ..LakeSpec::default()
        };
        let body = carve_lake(&mut grid, &spec, 0).expect("lake");
        let top = grid.sample(0.0, 0.0);
        assert!(
            top > body.water_y + 0.5,
            "island dome above the mirror: {top} vs {}",
            body.water_y
        );
        // O espelho desaparece sobre a ilha (centro do leque com alpha 0).
        let mesh = lake_water_mesh(&spec, body.water_y);
        assert!(
            mesh.colors[0][3] < 0.05,
            "mirror alpha over the island: {}",
            mesh.colors[0][3]
        );
        // Sem ilha o centro fica sem máscara (regressão zero).
        let bare = LakeSpec {
            at: Vec2::new(0.0, 0.0),
            radius: 14.0,
            depth: 3.0,
            ..LakeSpec::default()
        };
        let mesh = lake_water_mesh(&bare, body.water_y);
        assert!((mesh.colors[0][3] - 1.0).abs() < 1e-5);
    }

    /// Ilha que não cabe no contorno harmónico é ignorada (o RAISE não
    /// sai para a encosta seca).
    #[test]
    fn test_island_outside_the_bowl_is_skipped() {
        let mut grid = flat_grid();
        let spec = LakeSpec {
            at: Vec2::new(0.0, 0.0),
            radius: 8.0,
            depth: 2.0,
            islands: vec![crate::terrain::water::IslandSpec {
                at: Vec2::new(32.0, 0.0), // 32 m do centro, fora do carve todo
                radius: 5.0,
                height: 2.0,
            }],
            ..LakeSpec::default()
        };
        carve_lake(&mut grid, &spec, 0).expect("lake");
        assert!(
            (grid.sample(32.0, 0.0) - 8.0).abs() < 1e-3,
            "island outside the bowl never raises: {}",
            grid.sample(32.0, 0.0)
        );
    }

    /// Gorge: o heightfield NÃO esculpe a rampa da margem (o sólido natural
    /// fica de pé para o mod voxel cortar) e não há bancos raise.
    #[test]
    fn test_gorge_bank_leaves_the_bank_untouched() {
        let mut grid = flat_grid();
        let spec = RiverSpec {
            path: vec![Vec2::new(-40.0, 0.0), Vec2::new(40.0, 0.0)],
            width: 6.0,
            depth: 2.0,
            bank: BankStyle::Gorge,
            ..river_spec()
        };
        let body = carve_river(&mut grid, &spec, 0, &[]).expect("river");
        // Canal intacto...
        assert!(
            grid.sample(0.0, 0.0) < body.water_y,
            "channel still carves: {} vs {}",
            grid.sample(0.0, 0.0),
            body.water_y
        );
        // ...e a margem para lá do banco continua no terreno natural (8 m).
        let far = body.water_width * 0.5 + 4.0;
        assert!(
            (grid.sample(0.0, far) - 8.0).abs() < 1e-3,
            "gorge keeps the natural bank: {}",
            grid.sample(0.0, far)
        );
    }

    /// Gorge em LAGO: o heightfield carva a taça só até à LINHA DE ÁGUA —
    /// a margem para lá dela fica no terreno natural (o mod voxel é que
    /// corta a parede; ver `voxel::riverbank::lake_shore_band`), e o piso
    /// sob o espelho continua carvado. No caminho soft a rampa existe
    /// (comportamento histórico intacto — coberto por
    /// `test_carve_lake_lowers_inside_leaves_outside`).
    #[test]
    fn test_gorge_lake_carves_only_to_the_waterline() {
        let mut grid = flat_grid();
        let spec = LakeSpec {
            // (10, 10): o centro tem de ficar LONGE da régua do mundo (±48
            // num grid de 96) — em (48,48) o carve cai fora do grid e o
            // sample clampado à célula de borda lia o fiapo da taça.
            at: Vec2::new(10.0, 10.0),
            radius: 12.0,
            depth: 3.0,
            bank: BankStyle::Gorge,
            ..LakeSpec::default()
        };
        let body = carve_lake(&mut grid, &spec, 0).expect("lake");
        // Taça: o centro fica abaixo do espelho.
        assert!(
            grid.sample(10.0, 10.0) < body.water_y,
            "bowl still carves: {} vs {}",
            grid.sample(10.0, 10.0),
            body.water_y
        );
        let phases = shape_phases(spec.at);
        let contour = lake_shape_radius(spec.radius, 0.0, phases);
        let reach = (waterline_reach(spec.depth, spec.water_offset) * CARVE_MARGIN).clamp(0.5, 1.6);
        // Sonda 1 m FORA da linha de água mas ainda bem dentro do carve
        // antigo (contorno·CARVE_MARGIN) — antes do fix aqui estava o piso
        // da taça; agora o sólido natural (8 m) fica de pé.
        let probe = 10.0 + contour * reach + 1.0;
        assert!(
            contour * reach + 1.0 < contour * CARVE_MARGIN - 0.75,
            "probe must sit between the waterline and the old carve: {} vs {}",
            contour * reach + 1.0,
            contour * CARVE_MARGIN
        );
        // A margem natural fica DE PÉ acima do espelho — o bug era o carve
        // antigo pôr o probe DENTRO da taça (lia o piso ~5 m). A sonda fica a
        // ~1 texel da fronteira do carve, por isso o stencil bilinear pode
        // misturar a última célula talhada; o que nunca pode é ler abaixo da
        // lâmina com folga.
        assert!(
            grid.sample(probe, 10.0) > body.water_y + 0.25,
            "gorge lake keeps the natural shore beyond the waterline: {} vs {}",
            grid.sample(probe, 10.0),
            body.water_y
        );
        // Dentro da linha de água o piso continua abaixo da lâmina — o
        // espelho assenta em água, não em seco.
        let inner = 10.0 + contour * reach * 0.8;
        assert!(
            grid.sample(inner, 10.0) <= body.water_y + 1e-3,
            "bowl floor below the mirror inside the waterline: {} vs {}",
            grid.sample(inner, 10.0),
            body.water_y
        );
    }

    /// O espelho do lago cobre a linha de água real: lago fundo estende-se
    /// para lá do contorno; lago raso acaba antes (o contorno já é praia).
    #[test]
    fn test_lake_mirror_tracks_the_waterline() {
        let mirror_extent = |depth: f32| {
            let spec = LakeSpec {
                at: Vec2::new(40.0, 40.0),
                radius: 10.0,
                depth,
                ..LakeSpec::default()
            };
            let mesh = lake_water_mesh(&spec, 8.0);
            let outer0 = 1 + LAKE_FAN_SEGMENTS;
            mesh.positions[outer0..]
                .iter()
                .map(|p| ((p[0] - 40.0).powi(2) + (p[2] - 40.0).powi(2)).sqrt())
                .fold(f32::NEG_INFINITY, f32::max)
        };
        let deep = mirror_extent(3.2);
        let shallow = mirror_extent(0.8);
        // Pico do contorno harmónico × reach (a mesma fórmula da mesh).
        // Os 72 segmentos amostram o contorno — o pico verdadeiro pode
        // cair entre amostras (5.º harmónico oscila a 25°/segmento),
        // pelo que a tolerância é de 3%.
        let peak = |depth: f32| {
            10.0 * CONTOUR_PEAK
                * (waterline_reach(depth, 0.5) * CARVE_MARGIN)
                    .clamp(0.5, CONTOUR_PEAK * CARVE_MARGIN)
        };
        assert!(
            (deep - peak(3.2)).abs() < peak(3.2) * 0.03,
            "deep mirror tracks the waterline: {deep} vs {}",
            peak(3.2)
        );
        assert!(
            (shallow - peak(0.8)).abs() < peak(0.8) * 0.03,
            "shallow mirror tracks the waterline: {shallow} vs {}",
            peak(0.8)
        );
        assert!(
            peak(0.8) < peak(3.2),
            "shallow waterline ends before the deep one"
        );
    }

    /// `distance_to_waterline` do lago mede até à linha de água REAL do
    /// espelho (contorno × reach guardado no carve): zero exactamente onde
    /// a coluna de água vale `water_offset`, negativo dentro, positivo
    /// fora — incluindo no CONTORNO NOMINAL, que em lagos rasos já é
    /// praia seca (a métrica antiga devolvia 0 aí e desancorava a banda
    /// de areia/lama do splat).
    #[test]
    fn test_lake_waterline_distance_tracks_the_mirror() {
        let mut grid = flat_grid();
        let spec = LakeSpec {
            at: Vec2::new(48.0, 48.0),
            radius: 12.0,
            depth: 1.2,
            water_offset: 0.5,
            ..LakeSpec::default()
        };
        let body = carve_lake(&mut grid, &spec, 0).expect("lake");
        // O registry carrega o MESMO fator do mesh do espelho — e o clamp
        // é inerte para este depth/offset (o reach cruza a taça a sério).
        let raw = waterline_reach(spec.depth, spec.water_offset) * CARVE_MARGIN;
        assert!(
            (body.mirror_reach - raw).abs() < 1e-6,
            "mirror reach stored in the body"
        );
        assert!(raw > 0.5 && raw < 1.0, "clamp inert, reach inside: {raw}");
        let theta: f32 = 0.83; // direção arbitrária
        let dir = Vec2::new(theta.cos(), theta.sin());
        let contour = lake_shape_radius(spec.radius, theta, shape_phases(spec.at));
        // Na linha de água (coluna == water_offset): zero exato.
        let edge = spec.at + dir * (contour * body.mirror_reach);
        assert!(
            body.distance_to_waterline(edge).abs() < 1e-3,
            "waterline at the mirror edge: {}",
            body.distance_to_waterline(edge)
        );
        // Dentro (coluna > offset): negativo.
        let inner = spec.at + dir * (contour * body.mirror_reach * 0.5);
        assert!(
            body.distance_to_waterline(inner) < 0.0,
            "inside the mirror is negative"
        );
        // No contorno nominal: terra seca — a métrica antiga devolvia 0
        // aqui (linha de água no lugar errado).
        let nominal = spec.at + dir * contour;
        assert!(
            body.distance_to_waterline(nominal) > 0.0,
            "nominal contour is dry land now: {}",
            body.distance_to_waterline(nominal)
        );
    }
}

