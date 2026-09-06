//! Procedural cliffs — the one carve that *wants* the vertical step.
//!
//! Every other ground feature (roads, rivers, lakes) feathers its edges with
//! the adaptive falloff because a stray one-texel wall reads as a rendering
//! artifact. A cliff is the opposite: the step IS the feature. The u16 grid
//! represents near-vertical walls faithfully (a 30 m drop between adjacent
//! ~1 m texels reads ~86°) and the monotone Catmull-Rom sampler preserves
//! the step without ringing, so cliffs live in the heightfield like every
//! other feature — colliders, spawners and `sample_mesh_surface` stay in
//! sync for free.
//!
//! # Geometry
//!
//! The authored `path` is the **crest line**; the face band extends one-sided
//! on the drop side, `[0, width]` meters from the path. Per station the
//! carve samples the terrain just outside both ends of the band:
//!
//! * `top_y` — the crest height (top side, untouched by the carve);
//! * `bot_y` — the natural toe height (just beyond the band).
//!
//! Inside the band the design surface lerps `top_y → bot_y` through the
//! profile curve, so an auto drop (`height` absent) reshapes whatever
//! elevation difference the place already has — the wall adapts to the
//! terrain, not the other way round. An authored `height` overrides the toe:
//! `bot_y = min(natural, top_y − height)`, which caps drops that are too
//! tall and digs a quarry-style pit into flat ground.
//!
//! # Profiles
//!
//! * [`CliffProfile::Vertical`] — smoothstep S-curve across the band: natural
//!   shoulders, the steepest face in the middle.
//! * [`CliffProfile::Concave`] — `t^0.4`: vertical at the crest, leaning
//!   back toward the toe — the quarry/cirque wall, as close to an undercut
//!   as a 2.5D heightfield gets.
//! * [`CliffProfile::Terraced`] — quantized ledges (~3 m tall) with
//!   deterministic per-step jitter: mesas and treads.
//!
//! Determinism: all samples are taken **before** the stroke opens and every
//! variation (band wobble, step jitter) comes from `seed` + position hashes,
//! never from RNG or mid-carve reads — same seed, same world.

use bevy::math::Vec2;

use super::brush::{BrushGrid, BrushMode, BrushRequest, min_effective};
use super::paths::{
    PathHit, chaikin_smooth, distance_to_path, nearest_on_path, resample, station_lerp,
};

/// Face profile of a carved cliff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CliffProfile {
    /// Smoothstep S-curve: flat shoulders top and bottom, steep face centered.
    #[default]
    Vertical,
    /// `t^0.4` — vertical at the crest, leaning back toward the toe.
    Concave,
    /// `t^2.2` — rounded brow at the crest, steepest at the toe: the face
    /// bulges outward like a dome/outcrop when seen from below.
    Convex,
    /// Columnar basalt look (the wargame-reference wall): the crest line
    /// breaks into straight per-column plan-view offsets (protruding fronts,
    /// recessed slots), each column a piecewise-linear stack of planar slabs
    /// ("straight cuts") with a jittered, chunky brow.
    Columnar,
    /// Quantized ledges with per-step jitter.
    Terraced,
    /// Deliberate overhang: the face leans OUT over its own foot, so the brow
    /// shades a usable recess underneath. Voxel-only by nature — a heightfield
    /// cannot hold two surfaces over one point, which is why this profile
    /// could not exist before (`src/terrain/voxel/cliff.rs`).
    Overhang,
    /// Natural rock arch: the contour stays plumb and ONE arched window is
    /// bored through the wall at the band's middle (`src/terrain/voxel/
    /// cliff.rs`) — the void is the feature, and only the voxel field can
    /// hold rock above, air and rock below at the same XZ.
    Arch,
}

/// Exponent of the convex profile: > 1 so the brow rounds over gently and
/// the face steepens toward the toe.
const CONVEX_POWER: f32 = 2.2;

/// One column of a [`CliffProfile::Columnar`] face. Every parameter is
/// hash-derived from `(seed, index)` — deterministic, and discontinuous at
/// the column boundary (the 1-texel jump renders as the dark slit between
/// columns).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    /// Arc range covered by the column (meters along the crest polyline).
    pub start: f32,
    pub end: f32,
    /// Plan-view crest offset along the drop axis as a FRACTION of the local
    /// band width: positive = recessed (crest behind the path, the notch
    /// bites into the plateau edge), negative = protruding front (the face
    /// starts at the path and eats into the lower ground).
    pub off: f32,
    /// Crest height jitter (meters) — the chunky, bouldery brow.
    pub jit: f32,
    /// Piecewise-linear facet kinks (`s` at the 0.42 / 0.72 band fractions):
    /// the straight-cut slab breaks, varied per column.
    pub k1: f32,
    pub k2: f32,
}

impl Column {
    pub fn new(seed: u64, index: u64, start: f32, end: f32) -> Self {
        Self {
            start,
            end,
            off: (hash01(seed, index, 101) - 0.5) * 0.56,
            jit: (hash01(seed, index, 211) - 0.5) * 1.6,
            k1: 0.28 + hash01(seed, index, 307) * 0.16,
            k2: 0.62 + hash01(seed, index, 409) * 0.16,
        }
    }
}

/// Straight-cut face of one column: piecewise-linear with two hard kinks —
/// planar slabs meeting at visible seam lines, no smoothing anywhere.
fn facet_s(t: f32, col: &Column) -> f32 {
    if t <= 0.42 {
        col.k1 * (t / 0.42)
    } else if t <= 0.72 {
        col.k1 + (col.k2 - col.k1) * ((t - 0.42) / 0.30)
    } else {
        col.k2 + (1.0 - col.k2) * ((t - 0.72) / 0.28)
    }
}

impl CliffProfile {
    /// Parses the `profile` attribute
    /// (`vertical|concave|convex|columnar|terraced|overhang|arch`).
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "vertical" => Some(Self::Vertical),
            "concave" => Some(Self::Concave),
            "convex" => Some(Self::Convex),
            "columnar" | "columns" | "cortes" => Some(Self::Columnar),
            "terraced" => Some(Self::Terraced),
            "overhang" | "saliencia" | "saliência" => Some(Self::Overhang),
            "arch" | "arco" => Some(Self::Arch),
            _ => None,
        }
    }
}

/// Which side of the path the face drops toward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CliffSide {
    /// The side opposite the higher crest probe (mean over all stations) —
    /// the wall keeps the high ground and drops toward the low side.
    #[default]
    Auto,
    /// Left of the path direction (first → last station), map view.
    Left,
    /// Right of the path direction.
    Right,
}

impl CliffSide {
    /// Parses the `side` attribute (`auto|left|right`).
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// Declarative cliff parsed from a `<Cliff>` tag.
#[derive(Debug, Clone, PartialEq)]
pub struct CliffSpec {
    /// Crest polyline in world XZ (`"x z x z …"`); the face hangs on the
    /// drop side of it.
    pub path: Vec<Vec2>,
    /// Horizontal run of the face (meters) — `width`. Ignored when both
    /// `height` and `angle` are authored (then `width = height / tan(angle)`).
    pub width: f32,
    /// Total drop (meters). `None` = auto: the natural elevation difference
    /// between crest and toe (the wall adapts to the place).
    pub height: Option<f32>,
    /// Face slope in degrees — with `height` authored, derives the band width
    /// (`height / tan(angle)`); 90° would be zero-width, so the useful range
    /// is ~50–85°.
    pub angle: Option<f32>,
    pub profile: CliffProfile,
    pub side: CliffSide,
    /// Organic band-edge wobble as a fraction of `width` (0 = ruler-straight).
    pub noise: f32,
    /// Erosion gullies (`gullies`): one-sided recesses of the face as a
    /// fraction of `width` (0 = off, useful 0.15–0.4). High-frequency
    /// grooves cut the wall back between buttresses that stay on the
    /// nominal face line — the vertical ribbing of real rock faces.
    pub gullies: f32,
    /// Crest notches (`notches`): one-sided dips of the crest line as a
    /// fraction of the LOCAL drop (0 = off, useful 0.1–0.3). Low-frequency
    /// cols in the silhouette, scaled by the drop so gentle stretches stay.
    pub notches: f32,
    /// Debris apron at the toe (`talus`): an angle-of-repose scree cone
    /// rising from the natural ground to swallow the wall base — almost
    /// every real cliff sheds one.
    pub talus: bool,
    /// Repose angle of the talus surface in degrees (`talus-angle`, 36).
    /// Sets how far the apron spreads: `run ≈ 0.55 · drop / tan(angle)`.
    pub talus_angle: f32,
    /// Seed for the wobble/step jitter (deterministic).
    pub seed: u64,
}

impl Default for CliffSpec {
    fn default() -> Self {
        Self {
            path: Vec::new(),
            width: 6.0,
            height: None,
            angle: None,
            profile: CliffProfile::default(),
            side: CliffSide::default(),
            noise: 0.15,
            gullies: 0.0,
            notches: 0.0,
            talus: false,
            talus_angle: 36.0,
            seed: 0,
        }
    }
}

/// Registry entry produced by a cliff carve (query-side, like
/// [`super::water::WaterBody`]): the carved crest polyline plus the design
/// planes, so road guards / splat work can reason about the wall later.
#[derive(Debug, Clone, PartialEq)]
pub struct CliffLine {
    /// Smoothed crest stations in world XZ.
    pub stations: Vec<Vec2>,
    /// Design band width (meters, before the noise wobble).
    pub width: f32,
    /// Crest height per station (meters).
    pub top_y: Vec<f32>,
    /// Toe height per station (meters, after the authored-height override).
    pub bot_y: Vec<f32>,
    /// Which side of the path the face drops toward: `+1` = left normal,
    /// `-1` = right (the signed-depth convention of the carve).
    pub drop_sign: f32,
    /// Talus apron run per station (meters; all zeros when the cliff has no
    /// `talus`) — rasterized into the mask and skipped by sharpen.
    pub talus_run: Vec<f32>,
}

impl CliffLine {
    /// Horizontal distance from `p` to the crest polyline (meters).
    pub fn distance_to_crest(&self, p: Vec2) -> f32 {
        distance_to_path(&self.stations, p)
    }

    /// Point is within `margin` of the face band (crest ± band + margin).
    pub fn contains(&self, p: Vec2, margin: f32) -> bool {
        self.distance_to_crest(p) <= self.width + margin
    }
}

/// Exponent of the concave profile (`t^CONCAVE_POWER`): < 1 so the face is
/// steepest right under the crest and leans back toward the toe.
const CONCAVE_POWER: f32 = 0.4;
/// The vertical profile concentrates the drop in this middle band of `t`.
const VERTICAL_EDGES: (f32, f32) = (1.0 / 3.0, 2.0 / 3.0);
/// Terraced ledges aim for this tread RUN (meters of band per step), 2..=8
/// steps — the flat benches that catch the grass splat (wargame reference).
pub(crate) const TERRACE_TREAD: f32 = 2.5;
/// Weight fade beyond the band toe (in texels) so the wall lands on the
/// natural ground without a second step.
const TOE_FADE_TEXELS: f32 = 2.0;
/// Dilation (texels) of the public cliff-query layer: grass/spawners skip
/// the wall AND this verge.
pub const CLIFF_MASK_MARGIN_TEXELS: usize = 2;
/// Half-window (texels) of the wall-space estimate in
/// [`CliffMask::compute_wall`]: the local min/max search around a marked
/// texel. 8 texels ≈ the band width of a mid-size wall, enough to catch
/// both brow and toe without reaching over to the neighboring landform.
const WALL_WINDOW_TEXELS: usize = 8;
/// Neutral wall-space byte (flat/terraced context — no weathering gradient).
const WALL_NEUTRAL: u8 = 128;

/// 24-bit deterministic hash → `[0, 1)`.
pub(crate) fn hash01(seed: u64, a: u64, b: u64) -> f32 {
    let mut x =
        seed ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let span = (edge1 - edge0).max(1e-6);
    let t = ((x - edge0) / span).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Profile curve `s(t)`, `0` at the crest and `1` at the toe.
fn profile_s(t: f32, profile: CliffProfile, steps: f32, station: f32, seed: u64) -> f32 {
    match profile {
        CliffProfile::Vertical => smoothstep(VERTICAL_EDGES.0, VERTICAL_EDGES.1, t),
        CliffProfile::Concave => t.powf(CONCAVE_POWER),
        // Bulging face: flat brow at the crest (s'(0)=0) steepening toward
        // the toe — the mirror of the concave quarry wall.
        CliffProfile::Convex => t.powf(CONVEX_POWER),
        CliffProfile::Columnar => facet_s(t, &Column::new(seed, station as u64, 0.0, 1.0)),
        // The 2.5D curve cannot express an overhang; if anything still asks
        // for one it gets the steepest single-valued face available. Nothing
        // in the live path does — the profile is consumed by the voxel mod.
        CliffProfile::Overhang => t.powf(CONCAVE_POWER),
        // Same story as the overhang: the void of an arch cannot live in a
        // single-valued profile. The contour is the plumb wall; the window
        // is a voxel mod.
        CliffProfile::Arch => smoothstep(VERTICAL_EDGES.0, VERTICAL_EDGES.1, t),
        CliffProfile::Terraced => {
            // Steps quantized by RUN (the caller passes band/TERRACE_TREAD):
            // every tread is ~TERRACE_TREAD meters of horizontal bench
            // whatever the drop, so the splat paints grass on the flat part
            // and stone on the riser — the grassy benches of the reference.
            let x = t * steps;
            let k = x.floor();
            let frac = x - k;
            // Deterministic jitter moves each riser edge a little along the
            // band so the ledges don't read as machine-ruled.
            let edge = 0.72 + 0.2 * hash01(seed, station.to_bits() as u64, k as u64);
            ((k + smoothstep(edge, 1.0, frac)) / steps).clamp(0.0, 1.0)
        }
    }
}

/// Left normal of the segment starting at `i` (map view: looking along the
/// path from the first to the last station).
pub(crate) fn segment_left(stations: &[Vec2], i: usize) -> Vec2 {
    let a = stations[i];
    let b = stations[(i + 1).min(stations.len() - 1)];
    let dir = (b - a).normalize_or_zero();
    Vec2::new(-dir.y, dir.x)
}

/// Carves one cliff into the grid (journal owner `"cliff:{index}"`) and
/// returns its registry line. `None` when the spec is degenerate
/// (fewer than 2 path points, non-positive width/angle).
///
/// All terrain sampling happens before the stroke opens, so the carve is
/// independent of write order and byte-identical across runs.
pub fn carve_cliff(grid: &mut BrushGrid, spec: &CliffSpec, index: usize) -> Option<CliffLine> {
    if spec.path.len() < 2 {
        return None;
    }
    if !spec.width.is_finite() || spec.width <= 0.0 {
        return None;
    }
    if let Some(a) = spec.angle {
        if !a.is_finite() || a <= 0.0 {
            return None;
        }
    }
    let texel = grid.texel();

    // Crest stations: round the corners, then even spacing so per-station
    // arrays index continuously (same treatment as rivers/roads).
    let spacing = (texel * 2.0).max(1.0);
    let smoothed = chaikin_smooth(&spec.path, 2, false);
    let stations = resample(&smoothed, spacing);
    if stations.len() < 2 {
        return None;
    }

    // Authored angle (with height) overrides the width: the face slope is
    // what the author cares about, the run follows from the drop.
    let base_width = match (spec.height, spec.angle) {
        (Some(h), Some(a)) if h.is_finite() && h > 0.0 => {
            (h / a.to_radians().tan()).max(min_effective(1.0, texel))
        }
        _ => spec.width,
    };
    if !base_width.is_finite() || base_width <= 0.0 {
        return None;
    }

    // Per-station normals (left of the local tangent) and the four probes
    // that decide side, crest and toe: just inside the band and just beyond
    // it, on both sides of the path.
    let inner_off = texel * 1.5;
    let outer_off = base_width + texel * 2.0;
    let mut inner_plus = Vec::with_capacity(stations.len());
    let mut outer_plus = Vec::with_capacity(stations.len());
    let mut inner_minus = Vec::with_capacity(stations.len());
    let mut outer_minus = Vec::with_capacity(stations.len());
    let last = stations.len() - 1;
    for (i, st) in stations.iter().enumerate() {
        // The final station has no next segment — reuse the previous one.
        let n = segment_left(&stations, if i == last { i - 1 } else { i });
        inner_plus.push(grid.sample(st.x + n.x * inner_off, st.y + n.y * inner_off));
        outer_plus.push(grid.sample(st.x + n.x * outer_off, st.y + n.y * outer_off));
        inner_minus.push(grid.sample(st.x - n.x * inner_off, st.y - n.y * inner_off));
        outer_minus.push(grid.sample(st.x - n.x * outer_off, st.y - n.y * outer_off));
    }
    // Side resolution. `auto` compares the crest probes ACROSS the path (the
    // side that stands higher owns the crest; the face drops toward the
    // other side) — a per-side drop probe ties whenever the natural step
    // sits exactly on the path, because both probes read their own plateau.
    // Forced `Left`/`Right` trust the author and pair their own side.
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    let crest_higher_plus = mean(&inner_plus) >= mean(&inner_minus);
    let (top_y, bot_y): (Vec<f32>, Vec<f32>);
    let drop_sign: f32;
    match spec.side {
        CliffSide::Left => {
            top_y = inner_plus;
            bot_y = outer_plus;
            drop_sign = 1.0;
        }
        CliffSide::Right => {
            top_y = inner_minus;
            // v7 fix: the toe probe pairs with the CREST side (+x here) —
            // `outer_plus` read the OPPOSITE ground, so a forced Right on a
            // real step took the toe height from the plateau and could
            // carve the wall upside-down. (Masked until now: every world
            // used `auto`/`left`, or authored `height` pinned the toe.)
            bot_y = outer_minus;
            drop_sign = -1.0;
        }
        CliffSide::Auto if crest_higher_plus => {
            top_y = inner_plus;
            bot_y = outer_minus;
            drop_sign = -1.0;
        }
        CliffSide::Auto => {
            top_y = inner_minus;
            bot_y = outer_plus;
            drop_sign = 1.0;
        }
    }

    // Authored height pins the toe: caps taller natural drops and digs the
    // toe side down on flat ground (quarry wall).
    let mut top_y = top_y;
    let mut bot_y = bot_y;
    if let Some(h) = spec.height.filter(|h| h.is_finite() && *h > 0.0) {
        for i in 0..bot_y.len() {
            bot_y[i] = bot_y[i].min(top_y[i] - h);
        }
    }

    // Crest notches: low-frequency ONE-SIDED dips of the crest line — cols
    // in the silhouette. Scaled by the LOCAL drop so gentle stretches of
    // the path keep their brow.
    if spec.notches > 0.0 {
        let (n1p, n2p) = (
            hash01(spec.seed, 6, 23) * std::f32::consts::TAU,
            hash01(spec.seed, 7, 29) * std::f32::consts::TAU,
        );
        let mut n_arc = 0.0;
        for i in 0..top_y.len() {
            if i > 0 {
                n_arc += stations[i].distance(stations[i - 1]);
            }
            let a = 0.5 + 0.5 * (n_arc * 0.16 + n1p).sin();
            let b = 0.5 + 0.5 * (n_arc * 0.29 + n2p).sin();
            let dip = (a * a * 0.7 + b * b * 0.3).powf(1.4);
            top_y[i] -= spec.notches * (top_y[i] - bot_y[i]).max(0.0) * dip;
        }
    }

    // Band wobble: three harmonics over the arc length, seeded phases —
    // the face edge breathes instead of running ruler-straight. Gullies add
    // high-frequency ONE-SIDED recesses on top: erosion grooves that cut the
    // wall back between buttresses left on the nominal face line.
    let mut width_local = Vec::with_capacity(stations.len());
    let mut arc = 0.0;
    let (p1, p2, p3) = (
        hash01(spec.seed, 1, 7) * std::f32::consts::TAU,
        hash01(spec.seed, 2, 11) * std::f32::consts::TAU,
        hash01(spec.seed, 3, 13) * std::f32::consts::TAU,
    );
    let (g1p, g2p) = (
        hash01(spec.seed, 4, 17) * std::f32::consts::TAU,
        hash01(spec.seed, 5, 19) * std::f32::consts::TAU,
    );
    let gully_cut = |arc: f32| -> f32 {
        if spec.gullies <= 0.0 {
            return 0.0;
        }
        let a = 0.5 + 0.5 * (arc * 0.55 + g1p).sin();
        let b = 0.5 + 0.5 * (arc * 0.83 + g2p).sin();
        // Squared harmonics = narrow grooves; the blend weights how much of
        // each frequency shows. Never negative → recess only, no bulge.
        (a * a * 0.65 + b * b * 0.35).powf(1.6)
    };
    for w in stations.windows(2) {
        width_local.push(
            (wave(arc, p1, p2, p3, spec.noise, base_width)
                - spec.gullies * base_width * gully_cut(arc))
            .max(min_effective(1.0, texel)),
        );
        arc += w[0].distance(w[1]);
    }
    width_local.push(
        (wave(arc, p1, p2, p3, spec.noise, base_width)
            - spec.gullies * base_width * gully_cut(arc))
        .max(min_effective(1.0, texel)),
    );

    // Arc length per station — the columnar partition indexes by ARC
    // (physical meters along the crest), not by station index.
    let mut arc_stations = Vec::with_capacity(stations.len());
    arc_stations.push(0.0);
    for w in stations.windows(2) {
        let last = arc_stations.last().copied().unwrap_or(0.0);
        arc_stations.push(last + w[0].distance(w[1]));
    }
    // Columnar columns: irregular widths (1.6–4.2 m), each with its own
    // plan-view offset, crest jitter and facet kinks. The table extends
    // past both path ends so clamped hits always resolve a column.
    let columns: Vec<Column> = if spec.profile == CliffProfile::Columnar {
        let total = arc_stations.last().copied().unwrap_or(0.0) + base_width * 4.0;
        let mut cols = Vec::new();
        let mut start = -base_width;
        let mut i = 0u64;
        while start < total {
            let end = start + 2.2 + hash01(spec.seed, i, 7) * 3.3;
            cols.push(Column::new(spec.seed, i, start, end));
            start = end;
            i += 1;
        }
        cols
    } else {
        Vec::new()
    };

    // Talus: debris apron at the toe. `run` scales with the LOCAL drop at
    // the repose angle; `bury` (how much of the wall base the pile swallows)
    // is capped so the apron never eats the face. The natural ground past
    // the toe is probed per station BEFORE the stroke — the talus target is
    // analytic, built from `bot_y → toe_ground`, never from mid-carve reads.
    let mut talus_run = vec![0.0f32; stations.len()];
    if spec.talus {
        let cot = 1.0 / spec.talus_angle.clamp(5.0, 60.0).to_radians().tan();
        for (i, tr) in talus_run.iter_mut().enumerate() {
            let drop = (top_y[i] - bot_y[i]).max(0.0);
            *tr = (drop * cot * 0.55).min(40.0);
        }
    }
    let talus_max = talus_run.iter().copied().fold(0.0, f32::max);
    let toe_ground: Vec<f32> = if talus_max > 0.0 {
        stations
            .iter()
            .enumerate()
            .map(|(i, st)| {
                let n = segment_left(&stations, if i == last { i - 1 } else { i });
                let d = width_local[i] + talus_run[i] + texel * 2.0;
                grid.sample(st.x + n.x * drop_sign * d, st.y + n.y * drop_sign * d)
            })
            .collect()
    } else {
        Vec::new()
    };

    // Brush AABB over the stations plus the widest band, fade and margin —
    // and the talus apron when there is one.
    let mut min_x = f32::INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for st in &stations {
        min_x = min_x.min(st.x);
        min_z = min_z.min(st.y);
        max_x = max_x.max(st.x);
        max_z = max_z.max(st.y);
    }
    let reach = base_width * (1.0 + spec.noise.abs()) + texel * (TOE_FADE_TEXELS + 4.0) + talus_max;
    min_x -= reach;
    min_z -= reach;
    max_x += reach;
    max_z += reach;

    // Signed drop-side depth of `p`: `d` in meters along the local drop
    // normal (> 0 on the face side). `None` = nowhere near the band.
    let signed_depth = |p: Vec2| -> Option<(f32, PathHit)> {
        let hit = nearest_on_path(&stations, p)?;
        let n = segment_left(&stations, hit.segment);
        let d = (p - hit.point).dot(n * drop_sign);
        Some((d, hit))
    };
    let stations_len = stations.len();
    // Per-texel resolve: signed depth, local band width and — columnar only —
    // the depth shifted by the column's plan-view crest offset. The offset
    // moves the whole face (crest line zigzags with straight cuts; the
    // 1-texel jump between columns reads as the dark slit).
    let resolve = |p: Vec2| -> Option<(f32, f32, PathHit, Option<Column>)> {
        let (d, hit) = signed_depth(p)?;
        let wl = station_lerp(&width_local, &hit);
        if columns.is_empty() {
            return Some((d, wl, hit, None));
        }
        let arc_p = station_lerp(&arc_stations, &hit);
        let idx = columns
            .iter()
            .position(|c| arc_p >= c.start && arc_p < c.end)
            .unwrap_or(columns.len() - 1);
        let mut col = columns[idx];
        // Transition blend: within BLEND_TEXELS of a column boundary the
        // offset/jitter interpolate between the two neighbors — the wall
        // reads as one connected mass with soft notches instead of 1-texel
        // shard jumps at close range.
        let blend_m = texel * 2.0;
        let from_start = arc_p - col.start;
        let to_end = col.end - arc_p;
        if from_start < blend_m && idx > 0 {
            let f = smoothstep(0.0, blend_m, from_start);
            let prev = columns[idx - 1];
            col.off = prev.off + (col.off - prev.off) * f;
            col.jit = prev.jit + (col.jit - prev.jit) * f;
        } else if to_end < blend_m && idx + 1 < columns.len() {
            let f = smoothstep(0.0, blend_m, to_end);
            let next = columns[idx + 1];
            col.off = next.off + (col.off - next.off) * f;
            col.jit = next.jit + (col.jit - next.jit) * f;
        }
        let d_eff = d - col.off * wl;
        Some((d_eff, wl, hit, Some(col)))
    };
    let mut weight = |p: Vec2| -> f32 {
        let Some((d, wl, _hit, _col)) = resolve(p) else {
            return 0.0;
        };
        let fade = texel * TOE_FADE_TEXELS;
        if d < 0.0 {
            // Ramp into the crest so the top-side vertex is not dragged.
            return (1.0 + d / (texel * 2.0)).clamp(0.0, 1.0);
        }
        if d > wl + fade {
            return 0.0;
        }
        if d <= wl { 1.0 } else { 1.0 - (d - wl) / fade }
    };
    let mut target = |p: Vec2| -> f32 {
        let Some((d, wl, hit, col)) = resolve(p) else {
            return 0.0;
        };
        let t = (d / wl.max(1e-4)).clamp(0.0, 1.0);
        let top = station_lerp(&top_y, &hit);
        let bot = station_lerp(&bot_y, &hit);
        let station = hit.station(stations_len);
        match col {
            None => {
                let steps = ((wl / TERRACE_TREAD).round() as i32).clamp(2, 8) as f32;
                let s = profile_s(t, spec.profile, steps, station, spec.seed);
                top + (bot - top) * s
            }
            // Columnar face: straight-cut slabs on a column with its own
            // crest offset and jittered brow; drop comes from the planes.
            Some(col) => {
                let top_c = top + col.jit;
                top_c + (bot - top_c) * facet_s(t, &col)
            }
        }
    };

    grid.begin_stroke(&format!("cliff:{index}"));
    grid.apply(BrushRequest {
        mode: BrushMode::Blend,
        min_x,
        min_z,
        max_x,
        max_z,
        target: &mut target,
        weight: &mut weight,
    });

    // Talus stroke (same journal owner): RAISE-only — debris piles ON TOP
    // of the ground, it never quarries the rim away where the analytic
    // apron would dip under existing terrain (authored-height pit walls).
    if talus_max > 0.0 {
        let talus_start = texel * TOE_FADE_TEXELS;
        let mut talus_weight = |p: Vec2| -> f32 {
            let Some((d, wl, hit, _col)) = resolve(p) else {
                return 0.0;
            };
            if d <= wl {
                return 0.0; // the wall owns its own band
            }
            let run = station_lerp(&talus_run, &hit);
            if run <= 0.0 {
                return 0.0;
            }
            let t = (d - wl) / run;
            // Ramps in across the wall's toe fade (the seam blends into
            // the rock), dies before the apron's far edge.
            let rise = smoothstep(0.0, (talus_start * 1.5) / run, t);
            rise * (1.0 - smoothstep(0.7, 1.0, t))
        };
        let mut talus_target = |p: Vec2| -> f32 {
            let Some((d, wl, hit, _col)) = resolve(p) else {
                return 0.0;
            };
            let run = station_lerp(&talus_run, &hit);
            if run <= 0.0 {
                return 0.0;
            }
            let top = station_lerp(&top_y, &hit);
            let bot = station_lerp(&bot_y, &hit);
            let t = (d - wl).clamp(0.0, run) / run;
            // Debris surface over the natural ground: anchored on the toe
            // plane, rising `bury` against the rock face (dithered ±15%),
            // profile (1−t)^1.5 — flat spread at the far edge.
            let ground = bot + (station_lerp(&toe_ground, &hit) - bot) * t;
            let bury = (run * spec.talus_angle.clamp(5.0, 60.0).to_radians().tan())
                .min((top - bot).max(0.0) * 0.35);
            let station = hit.station(stations_len);
            let wobble =
                0.85 + 0.3 * hash01(spec.seed, station.to_bits() as u64, (t * 64.0) as u64);
            ground + bury * (1.0 - t).powf(1.5) * wobble
        };
        let talus_reach = texel * (TOE_FADE_TEXELS + 4.0) + talus_max;
        grid.begin_stroke(&format!("cliff:{index}"));
        grid.apply(BrushRequest {
            mode: BrushMode::Raise,
            min_x: min_x - talus_max,
            min_z: min_z - talus_max,
            max_x: max_x + talus_max,
            max_z: max_z + talus_max,
            target: &mut talus_target,
            weight: &mut talus_weight,
        });
    }

    Some(CliffLine {
        stations,
        width: base_width,
        top_y,
        bot_y,
        drop_sign,
        talus_run,
    })
}

/// Band width at an arc position: three seeded harmonics, ±`noise` of the
/// base width (0 noise = constant).
pub(crate) fn wave(arc: f32, p1: f32, p2: f32, p3: f32, noise: f32, base: f32) -> f32 {
    let w = 0.5 * (arc * 0.11 + p1).sin()
        + 0.3 * (arc * 0.27 + p2).sin()
        + 0.2 * (arc * 0.53 + p3).sin();
    base * (1.0 + noise.clamp(0.0, 1.0) * w)
}

/// Post-carve cliff regions — the INTELLIGENT trigger. A raw slope scan
/// alone cannot tell a cliff from a spurious bump: any single 1-texel jump
/// would pass. So `build` runs a pipeline with neighborhood awareness:
///
/// 1. raw scan (neighbor drop steeper than `cliff-angle`);
/// 2. morphological opening (erode + dilate, kills 1-texel speckle);
/// 3. connected-component labeling (deterministic BFS);
/// 4. per-component stats (area, drop, bbox extent) and a FILTER: a
///    component is a cliff only if area ≥ `min_area` m², extent ≥
///    `min_extent` m, and the drop (own range or the range of its ~4 m
///    context) ≥ `min_drop` m — terraced riser lines carry zero internal
///    drop, their context carries the real one;
/// 5. two layers: **core** (accepted components) and **dilated** (core grown
///    by `margin_texels` — the query layer with the margin baked in, so
///    grass/spawners skip the wall AND its verge).
///
/// Consumers: the mesh bakes `factor()` into vertex color alpha (the wall
/// shader multiplies its triplanar gate by it — spurious bumps get 0), the
/// splat paints solid stone over the core, sharpen only terraces the core,
/// the LOD peak-preserving snaps only inside regions, grass avoids the
/// dilated layer. Built once over the FINAL field.
#[derive(Debug, Clone, Default, bevy::ecs::prelude::Resource)]
pub struct CliffMask {
    /// Accepted cliff components (the filter output).
    core: Vec<u64>,
    /// Core dilated by the margin — the public query layer.
    bits: Vec<u64>,
    /// Authored talus aprons (rasterized from the carve registry) — painted
    /// as gravel by the splat, avoided by grass/spawners via `bits`.
    talus: Vec<u64>,
    /// Wall space per texel of the public layer, `0..=255`: the height
    /// fraction inside the local drop window (0 = brow, 255 = toe, 128 =
    /// neutral). Baked into vertex color R by the mesh builder — the wall
    /// shader reads it for the weathering gradient, the toe contact shadow
    /// and the streak/moss distribution.
    wall: Vec<u8>,
    width: usize,
    depth: usize,
    world_size: f32,
}

/// Raw steep scan of `grid` (neighbor drop steeper than `angle_deg`).
fn scan_steep(grid: &BrushGrid, angle_deg: f32, bits: &mut Vec<u64>) {
    let (width, depth) = (grid.width(), grid.depth());
    bits.iter_mut().for_each(|w| *w = 0);
    if !angle_deg.is_finite() || angle_deg <= 0.0 {
        return;
    }
    let limit = angle_deg.to_radians().tan() * grid.texel();
    for z in 0..depth {
        for x in 0..width {
            let h = grid.cell_height(x, z);
            let steep_x = x + 1 < width && (grid.cell_height(x + 1, z) - h).abs() > limit;
            let steep_z = z + 1 < depth && (grid.cell_height(x, z + 1) - h).abs() > limit;
            if steep_x || steep_z {
                set_bit(bits, width, x, z);
            }
        }
    }
}

fn get_bit(bits: &[u64], width: usize, x: usize, z: usize) -> bool {
    let idx = z * width + x;
    bits[idx / 64] & (1 << (idx % 64)) != 0
}

fn set_bit(bits: &mut [u64], width: usize, x: usize, z: usize) {
    let idx = z * width + x;
    bits[idx / 64] |= 1 << (idx % 64);
}

/// One dilation pass (4-neighborhood + self) over a bitset.
fn dilate_once(bits: &mut Vec<u64>, width: usize, depth: usize) {
    let src = bits.clone();
    for z in 0..depth {
        for x in 0..width {
            if get_bit(&src, width, x, z) {
                continue;
            }
            let any = (x > 0 && get_bit(&src, width, x - 1, z))
                || (x + 1 < width && get_bit(&src, width, x + 1, z))
                || (z > 0 && get_bit(&src, width, x, z - 1))
                || (z + 1 < depth && get_bit(&src, width, x, z + 1));
            if any {
                set_bit(bits, width, x, z);
            }
        }
    }
}

/// One erosion pass: keep a texel only if it and its 4 neighbors are set —
/// removes 1-texel speckle before labeling. Out-of-bounds neighbors count as
/// clear (border texels erode), which is what makes the opening eat thin
/// needles.
fn erode_once(bits: &mut Vec<u64>, width: usize, depth: usize) {
    let src = bits.clone();
    let at = |x: isize, z: isize| -> bool {
        x >= 0
            && z >= 0
            && (x as usize) < width
            && (z as usize) < depth
            && get_bit(&src, width, x as usize, z as usize)
    };
    for z in 0..depth {
        for x in 0..width {
            let (xi, zi) = (x as isize, z as isize);
            let keep =
                at(xi, zi) && at(xi - 1, zi) && at(xi + 1, zi) && at(xi, zi - 1) && at(xi, zi + 1);
            if !keep {
                let idx = z * width + x;
                bits[idx / 64] &= !(1 << (idx % 64));
            }
        }
    }
}

impl CliffMask {
    /// Full pipeline over `grid` with the filter thresholds of the spec.
    pub fn build(grid: &BrushGrid, spec: &super::spec::TerrainSpec) -> Self {
        Self::build_with(
            grid,
            spec.cliff_angle,
            spec.cliff_min_area,
            spec.cliff_min_drop,
            spec.cliff_min_extent,
        )
    }

    /// Pipeline with explicit thresholds.
    pub fn build_with(
        grid: &BrushGrid,
        angle_deg: f32,
        min_area: f32,
        min_drop: f32,
        min_extent: f32,
    ) -> Self {
        let (width, depth) = (grid.width(), grid.depth());
        let mut mask = Self {
            core: vec![0; width.saturating_mul(depth).div_ceil(64)],
            bits: vec![0; width.saturating_mul(depth).div_ceil(64)],
            talus: vec![0; width.saturating_mul(depth).div_ceil(64)],
            wall: vec![WALL_NEUTRAL; width.saturating_mul(depth)],
            width,
            depth,
            world_size: grid.world_size(),
        };
        // 1. Raw scan + 2. opening.
        let mut raw = vec![0u64; width.saturating_mul(depth).div_ceil(64)];
        scan_steep(grid, angle_deg, &mut raw);
        erode_once(&mut raw, width, depth);
        dilate_once(&mut raw, width, depth);

        // 3+4. Connected components with stats + the region filter → core.
        Self::label_into_core(
            grid, &raw, width, depth, min_area, min_drop, min_extent, &mut mask,
        );
        // 5. Dilated query layer.
        mask.bits = mask.core.clone();
        for _ in 0..CLIFF_MASK_MARGIN_TEXELS {
            dilate_once(&mut mask.bits, width, depth);
        }
        // 6. Wall space (weathering gradient input for the wall shader).
        mask.compute_wall(grid, min_drop);
        mask
    }

    /// Height fraction inside the local drop per marked texel: a min/max
    /// window around each texel of the public layer spans brow→toe on a
    /// wall, so `(h − min) / (max − min)` reads "how far down the wall am
    /// I". Flat context (range under half the region's min drop — terrace
    /// treads, gentle verge) stays neutral.
    fn compute_wall(&mut self, grid: &BrushGrid, min_drop: f32) {
        let (width, depth) = (self.width, self.depth);
        let floor_drop = min_drop.max(0.5) * 0.5;
        for z in 0..depth {
            for x in 0..width {
                if !get_bit(&self.bits, width, x, z) {
                    continue;
                }
                let x0 = x.saturating_sub(WALL_WINDOW_TEXELS);
                let x1 = (x + WALL_WINDOW_TEXELS).min(width - 1);
                let z0 = z.saturating_sub(WALL_WINDOW_TEXELS);
                let z1 = (z + WALL_WINDOW_TEXELS).min(depth - 1);
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                for zz in z0..=z1 {
                    for xx in x0..=x1 {
                        let h = grid.cell_height(xx, zz);
                        lo = lo.min(h);
                        hi = hi.max(h);
                    }
                }
                if hi - lo < floor_drop {
                    continue; // flat/terraced context keeps WALL_NEUTRAL
                }
                // 0 = brow (high), 1 = toe (low) — the shader's weathering
                // gradient and toe contact shadow read "how far down".
                let frac = ((hi - grid.cell_height(x, z)) / (hi - lo)).clamp(0.0, 1.0);
                self.wall[z * width + x] = (frac * 255.0).round() as u8;
            }
        }
    }

    /// Rasterizes authored talus aprons into the mask and rebuilds the
    /// public layer as `dilate(core ∪ talus)` — grass, spawners and the
    /// LOD gate inherit the apron exclusion for free. Talus texels keep a
    /// NEUTRAL wall value (the apron is a pile, not a face — no fake
    /// weathering gradient over it).
    /// Marks the footprint of the authored `<Cliff>` bands.
    ///
    /// The 2.5D system had to *find* its cliffs: the carve wrote a steeper
    /// patch of `u16` and `build_with` recovered the regions afterwards with
    /// erode/dilate/BFS and five tunable thresholds, because by then nothing
    /// remembered where the wall was.
    ///
    /// A band knows exactly. This marks its core and its apron directly, so
    /// the splat paints stone on the wall, the grass stays off it and the
    /// spawners keep their clearance — with no heuristic in the path and
    /// nothing for `cliff-min-area`/`-drop`/`-extent` to get wrong. The
    /// morphological scan stays for NATURAL steep ground coming out of the
    /// heightmap, which really does have to be discovered.
    pub fn add_authored_bands(&mut self, bands: &[super::voxel::CliffBand]) {
        if self.width < 2 || self.depth < 2 || self.world_size <= 0.0 {
            return;
        }
        let (width, depth) = (self.width, self.depth);
        let texel = self.texel_size();
        if texel <= 0.0 {
            return;
        }
        let half = self.world_size * 0.5;
        for band in bands {
            if band.stations.len() < 2 {
                continue;
            }
            let max_w = band.width.iter().copied().fold(0.0, f32::max);
            let max_run = band.talus_run_max();
            let reach = max_w + max_run + texel * 4.0;
            let (mut min_x, mut min_z) = (f32::INFINITY, f32::INFINITY);
            let (mut max_x, mut max_z) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
            for st in &band.stations {
                min_x = min_x.min(st.x);
                min_z = min_z.min(st.y);
                max_x = max_x.max(st.x);
                max_z = max_z.max(st.y);
            }
            let clampx = |v: f32| v.clamp(0.0, (width - 1) as f32) as usize;
            let clampz = |v: f32| v.clamp(0.0, (depth - 1) as f32) as usize;
            let x0 = clampx(((min_x - reach) + half) / texel);
            let x1 = clampx((((max_x + reach) + half) / texel).ceil());
            let z0 = clampz(((min_z - reach) + half) / texel);
            let z1 = clampz((((max_z + reach) + half) / texel).ceil());

            for z in z0..=z1 {
                for x in x0..=x1 {
                    let p = Vec2::new(x as f32 * texel - half, z as f32 * texel - half);
                    let Some(hit) = nearest_on_path(&band.stations, p) else {
                        continue;
                    };
                    let n = band.drop_normal[hit.segment.min(band.drop_normal.len() - 1)];
                    let d = (p - hit.point).dot(n);
                    if d < -texel {
                        continue; // behind the crest: the plateau is not wall
                    }
                    let w = station_lerp(&band.width, &hit);
                    if d <= w {
                        set_bit(&mut self.core, width, x, z);
                        // Wall space straight from the geometry: 0 at the
                        // brow, 255 at the toe. The 2.5D path had to estimate
                        // this from a +/-8 texel min/max window over the
                        // finished heights.
                        let frac = (d / w.max(1e-3)).clamp(0.0, 1.0);
                        self.wall[z * width + x] = (frac * 255.0).round() as u8;
                    } else if max_run > 0.0 && d < w + band.talus_run_at(&hit) {
                        set_bit(&mut self.talus, width, x, z);
                        self.wall[z * width + x] = WALL_NEUTRAL;
                    }
                }
            }
        }
        self.bits = self.core.clone();
        for i in 0..self.bits.len() {
            self.bits[i] |= self.talus[i];
        }
        for _ in 0..CLIFF_MASK_MARGIN_TEXELS {
            dilate_once(&mut self.bits, width, depth);
        }
    }

    pub fn add_talus(&mut self, cliffs: &[CliffLine]) {
        if self.width < 2 || self.depth < 2 || self.world_size <= 0.0 {
            return;
        }
        let (width, depth) = (self.width, self.depth);
        let texel = self.texel_size();
        if texel <= 0.0 {
            return;
        }
        let half = self.world_size * 0.5;
        for line in cliffs {
            let max_run = line.talus_run.iter().copied().fold(0.0, f32::max);
            if max_run <= 0.0 || line.stations.len() < 2 {
                continue;
            }
            let (mut min_x, mut min_z) = (f32::INFINITY, f32::INFINITY);
            let (mut max_x, mut max_z) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
            for st in &line.stations {
                min_x = min_x.min(st.x);
                min_z = min_z.min(st.y);
                max_x = max_x.max(st.x);
                max_z = max_z.max(st.y);
            }
            let reach = line.width + max_run + texel * 4.0;
            let x0 = (((min_x - reach) + half) / texel)
                .floor()
                .clamp(0.0, (width - 1) as f32) as usize;
            let x1 = ((((max_x + reach) + half) / texel)
                .ceil()
                .clamp(0.0, (width - 1) as f32) as usize)
                .min(width - 1);
            let z0 = (((min_z - reach) + half) / texel)
                .floor()
                .clamp(0.0, (depth - 1) as f32) as usize;
            let z1 = ((((max_z + reach) + half) / texel)
                .ceil()
                .clamp(0.0, (depth - 1) as f32) as usize)
                .min(depth - 1);
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let p = Vec2::new(x as f32 * texel - half, z as f32 * texel - half);
                    let Some(hit) = nearest_on_path(&line.stations, p) else {
                        continue;
                    };
                    let n = segment_left(&line.stations, hit.segment);
                    let d = (p - hit.point).dot(n * line.drop_sign);
                    // Apron band: just past the face line up to the local
                    // run (the wall core itself stays out of the bitset).
                    if d <= line.width - texel {
                        continue;
                    }
                    if d >= line.width + station_lerp(&line.talus_run, &hit) {
                        continue;
                    }
                    set_bit(&mut self.talus, width, x, z);
                }
            }
        }
        self.bits = self.core.clone();
        for i in 0..self.bits.len() {
            self.bits[i] |= self.talus[i];
        }
        for _ in 0..CLIFF_MASK_MARGIN_TEXELS {
            dilate_once(&mut self.bits, width, depth);
        }
        for z in 0..depth {
            for x in 0..width {
                if get_bit(&self.talus, width, x, z) {
                    self.wall[z * width + x] = WALL_NEUTRAL;
                }
            }
        }
    }

    fn label_into_core(
        grid: &BrushGrid,
        raw: &[u64],
        width: usize,
        depth: usize,
        min_area: f32,
        min_drop: f32,
        min_extent: f32,
        mask: &mut Self,
    ) {
        let texel = grid.texel();
        let total = width * depth;
        let mut visited = vec![false; total];
        let mut members: Vec<usize> = Vec::new();
        for start in 0..total {
            if visited[start] || !get_bit(raw, width, start % width, start / width) {
                continue;
            }
            visited[start] = true;
            members.clear();
            members.push(start);
            let mut cursor = 0;
            let (mut min_h, mut max_h) = (f32::INFINITY, f32::NEG_INFINITY);
            let (mut min_x, mut max_x) = (usize::MAX, 0usize);
            let (mut min_z, mut max_z) = (usize::MAX, 0usize);
            while cursor < members.len() {
                let idx = members[cursor];
                cursor += 1;
                let (x, z) = (idx % width, idx / width);
                let h = grid.cell_height(x, z);
                min_h = min_h.min(h);
                max_h = max_h.max(h);
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_z = min_z.min(z);
                max_z = max_z.max(z);
                for (nx, nz) in [
                    (x.wrapping_sub(1), z),
                    (x + 1, z),
                    (x, z.wrapping_sub(1)),
                    (x, z + 1),
                ] {
                    if nx < width && nz < depth {
                        let n = nz * width + nx;
                        if !visited[n] && get_bit(raw, width, nx, nz) {
                            visited[n] = true;
                            members.push(n);
                        }
                    }
                }
            }
            let extent = (max_x - min_x).max(max_z - min_z) as f32 * texel;
            // Drop: the component's OWN range OR the range of its context
            // (bbox + ~4 m apron). Terraced fields are riser LINES with zero
            // internal drop — the context (the treads around them) carries
            // the real elevation change.
            let apron = (4.0 / texel).ceil() as usize;
            let cx0 = min_x.saturating_sub(apron);
            let cx1 = (max_x + apron).min(width - 1);
            let cz0 = min_z.saturating_sub(apron);
            let cz1 = (max_z + apron).min(depth - 1);
            let mut ctx_min = f32::INFINITY;
            for z in cz0..=cz1 {
                for x in cx0..=cx1 {
                    ctx_min = ctx_min.min(grid.cell_height(x, z));
                }
            }
            let accept = members.len() as f32 * texel * texel >= min_area
                && extent >= min_extent
                && (max_h - min_h >= min_drop || max_h - ctx_min >= min_drop);
            #[cfg(test)]
            if std::env::var("CLIFF_DEBUG").is_ok() {
                eprintln!(
                    "comp: texels {} area {:.0} m² extent {:.0} m own_drop {:.2} ctx_drop {:.2} → {}",
                    members.len(),
                    members.len() as f32 * texel * texel,
                    extent,
                    max_h - min_h,
                    max_h - ctx_min,
                    accept
                );
            }
            if accept {
                for &idx in &members {
                    set_bit(&mut mask.core, width, idx % width, idx / width);
                }
            }
        }
    }

    fn set(&mut self, x: usize, z: usize) {
        set_bit(&mut self.bits, self.width, x, z);
    }

    /// Texel `(x, z)` is INSIDE an accepted cliff component (no margin).
    pub fn is_core(&self, x: usize, z: usize) -> bool {
        if x >= self.width || z >= self.depth {
            return false;
        }
        get_bit(&self.core, self.width, x, z)
    }

    /// Texel `(x, z)` is cliff terrain or within the margin of one.
    pub fn is_cliff(&self, x: usize, z: usize) -> bool {
        if x >= self.width || z >= self.depth {
            return false;
        }
        get_bit(&self.bits, self.width, x, z)
    }

    /// Texel coordinates of a world position.
    fn texel_of(&self, p: Vec2) -> (f32, f32) {
        let half = self.world_size * 0.5;
        let step_x = self.world_size / (self.width - 1).max(1) as f32;
        let step_z = self.world_size / (self.depth - 1).max(1) as f32;
        ((p.x + half) / step_x, (p.y + half) / step_z)
    }

    /// World-space query — INSIDE an accepted component (no margin).
    pub fn is_core_at(&self, p: Vec2) -> bool {
        if self.width < 2 || self.world_size <= 0.0 {
            return false;
        }
        let (fx, fz) = self.texel_of(p);
        let x = fx.round().clamp(0.0, (self.width - 1) as f32) as usize;
        let z = fz.round().clamp(0.0, (self.depth - 1) as f32) as usize;
        self.is_core(x, z)
    }

    /// World-space query — the texel nearest to `p`, dilated layer.
    pub fn is_cliff_at(&self, p: Vec2) -> bool {
        if self.width < 2 || self.world_size <= 0.0 {
            return false;
        }
        let (fx, fz) = self.texel_of(p);
        let x = fx.round().clamp(0.0, (self.width - 1) as f32) as usize;
        let z = fz.round().clamp(0.0, (self.depth - 1) as f32) as usize;
        self.is_cliff(x, z)
    }

    /// World-space size of one mask texel (vertex convention of `texel_of`).
    pub fn texel_size(&self) -> f32 {
        if self.width < 2 || self.depth < 2 || self.world_size <= 0.0 {
            return 0.0;
        }
        self.world_size / (self.width - 1).max(1) as f32
    }

    /// Wall space at a world position, `0.0` (brow) → `1.0` (toe); `0.5`
    /// off-mask or on flat context. The mesh builder bakes this into vertex
    /// color R for the wall shader.
    pub fn wall_at(&self, p: Vec2) -> f32 {
        if self.wall.len() != self.width * self.depth || self.width < 2 {
            return 0.5;
        }
        let (fx, fz) = self.texel_of(p);
        let x = fx.round().clamp(0.0, (self.width - 1) as f32) as usize;
        let z = fz.round().clamp(0.0, (self.depth - 1) as f32) as usize;
        self.wall[z * self.width + x] as f32 / 255.0
    }

    /// Texel `(x, z)` is inside an authored talus apron.
    pub fn is_talus(&self, x: usize, z: usize) -> bool {
        if x >= self.width || z >= self.depth || self.talus.len() != self.core.len() {
            return false;
        }
        get_bit(&self.talus, self.width, x, z)
    }

    /// World-space query — inside an authored talus apron (the splat paints
    /// gravel over it).
    pub fn is_talus_at(&self, p: Vec2) -> bool {
        if self.width < 2 || self.world_size <= 0.0 {
            return false;
        }
        let (fx, fz) = self.texel_of(p);
        let x = fx.round().clamp(0.0, (self.width - 1) as f32) as usize;
        let z = fz.round().clamp(0.0, (self.depth - 1) as f32) as usize;
        self.is_talus(x, z)
    }

    /// World-space proximity query — cliff terrain (the dilated layer) within
    /// `meters` of `p`? Scans the texel window around the position, so spawn
    /// gates can keep props a REAL distance off the wall instead of the fixed
    /// 2-texels bake of `is_cliff_at`. Non-positive `meters` degenerates to
    /// `is_cliff_at`; the radius is capped so a pathological margin on a fine
    /// mask cannot stall the placement pass.
    pub fn is_cliff_within(&self, p: Vec2, meters: f32) -> bool {
        if self.width < 2 || self.depth < 2 || self.world_size <= 0.0 {
            return false;
        }
        let step_x = self.world_size / (self.width - 1).max(1) as f32;
        let step_z = self.world_size / (self.depth - 1).max(1) as f32;
        let (fx, fz) = self.texel_of(p);
        let r = (meters / step_x.min(step_z)).ceil().max(0.0).min(128.0) as i64;
        let (cx, cz) = (fx.round() as i64, fz.round() as i64);
        let half = self.world_size * 0.5;
        // Metade-largura do texel: a distância mede ao RETÂNGULO do texel,
        // não ao seu centro — "a 2 m do cliff" mede à superfície da parede,
        // e o texel mais próximo conta sempre (meters 0 degenera para
        // `is_cliff_at`).
        let (hx, hz) = (step_x * 0.5, step_z * 0.5);
        let m2 = meters * meters;
        for dz in -r..=r {
            let z = cz + dz;
            if z < 0 || z as usize >= self.depth {
                continue;
            }
            let oz = ((z as f32 * step_z - half - p.y).abs() - hz).max(0.0);
            for dx in -r..=r {
                let x = cx + dx;
                if x < 0 || x as usize >= self.width {
                    continue;
                }
                let ox = ((x as f32 * step_x - half - p.x).abs() - hx).max(0.0);
                if ox * ox + oz * oz > m2 {
                    continue;
                }
                if get_bit(&self.bits, self.width, x as usize, z as usize) {
                    return true;
                }
            }
        }
        false
    }

    /// Bilinear cliff factor `0..1` at a world position over the DILATED
    /// layer — the soft edge baked into vertex color alpha. Off-mask
    /// terrain reads 0, region interior 1, the margin blends.
    pub fn factor(&self, p: Vec2) -> f32 {
        if self.width < 2 || self.depth < 2 || self.world_size <= 0.0 {
            return 0.0;
        }
        let (fx, fz) = self.texel_of(p);
        let x0 = fx.floor().clamp(0.0, (self.width - 1) as f32) as usize;
        let z0 = fz.floor().clamp(0.0, (self.depth - 1) as f32) as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let z1 = (z0 + 1).min(self.depth - 1);
        let tx = (fx - x0 as f32).clamp(0.0, 1.0);
        let tz = (fz - z0 as f32).clamp(0.0, 1.0);
        let v = |x: usize, z: usize| {
            if get_bit(&self.bits, self.width, x, z) {
                1.0
            } else {
                0.0
            }
        };
        let top = v(x0, z0) * (1.0 - tx) + v(x1, z0) * tx;
        let bot = v(x0, z1) * (1.0 - tx) + v(x1, z1) * tx;
        top * (1.0 - tz) + bot * tz
    }

    /// Number of marked texels in the dilated layer (debug/stats).
    pub fn marked(&self) -> usize {
        self.bits.iter().map(|w| w.count_ones() as usize).sum()
    }
}

/// Opt-in sharpen pass (`<Terrain sharpen="1">`): rewrites smooth steep
/// ramps of the FINAL field into terraced cliff bands. Texels flatter than
/// `sharpen_angle` are untouched; steep ones quantize to ~3 m treads whose
/// band edges dither with seeded noise (no machine-ruled contour lines).
/// Sub-water texels are skipped so lake/river bowls stay glassy, roads
/// never trigger (their grade is limited below the default angle anyway), and
/// only texels inside ACCEPTED cliff components of the [`CliffMask`] are
/// terraced — spurious ramps in open ground keep their smooth shape, and the
/// bands of authored `<Cliff>` walls keep their own profiles (never
/// re-quantized into stairs).
///
/// The journal owner `"sharpen"` keeps revert + re-carve idempotent.
/// Deterministic: the dither hashes texel position + `sharpen-seed`
/// (`0` derives from the terrain `seed`), never RNG.
pub fn sharpen_terrain(
    grid: &mut BrushGrid,
    spec: &super::spec::TerrainSpec,
    water: &[super::water::WaterBody],
    mask: &CliffMask,
    cliffs: &[CliffLine],
) -> usize {
    let angle = if spec.sharpen_angle.is_finite() && spec.sharpen_angle > 0.0 {
        spec.sharpen_angle
    } else {
        super::spec::DEFAULT_SHARPEN_ANGLE
    };
    let limit = angle.to_radians().tan() * grid.texel();
    let seed = if spec.sharpen_seed != 0 {
        spec.sharpen_seed
    } else {
        spec.seed ^ 0xA11CE
    };

    let (width, depth) = (grid.width(), grid.depth());
    let texel = grid.texel();
    // Authored cliff bands are already shaped by their own profile — the
    // auto-terracer must not re-quantize them into stairs.
    let mut carved = vec![0u64; width.saturating_mul(depth).div_ceil(64)];
    for line in cliffs {
        let reach =
            line.width * 1.1 + texel * 2.0 + line.talus_run.iter().copied().fold(0.0, f32::max);
        let (mut min_x, mut min_z) = (f32::INFINITY, f32::INFINITY);
        let (mut max_x, mut max_z) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for st in &line.stations {
            min_x = min_x.min(st.x);
            min_z = min_z.min(st.y);
            max_x = max_x.max(st.x);
            max_z = max_z.max(st.y);
        }
        let x0 = (((min_x - reach) + grid.world_size() * 0.5) / texel)
            .floor()
            .clamp(0.0, (width - 1) as f32) as usize;
        let x1 = ((((max_x + reach) + grid.world_size() * 0.5) / texel)
            .ceil()
            .clamp(0.0, (width - 1) as f32) as usize)
            .min(width - 1);
        let z0 = (((min_z - reach) + grid.world_size() * 0.5) / texel)
            .floor()
            .clamp(0.0, (depth - 1) as f32) as usize;
        let z1 = ((((max_z + reach) + grid.world_size() * 0.5) / texel)
            .ceil()
            .clamp(0.0, (depth - 1) as f32) as usize)
            .min(depth - 1);
        for z in z0..=z1 {
            for x in x0..=x1 {
                let p = grid.cell_center(x, z);
                if line.distance_to_crest(p) <= reach {
                    set_bit(&mut carved, width, x, z);
                }
            }
        }
    }
    grid.begin_stroke("sharpen");
    for z in 0..depth {
        for x in 0..width {
            let h = grid.cell_height(x, z);
            let steep_x = x + 1 < width && (grid.cell_height(x + 1, z) - h).abs() > limit;
            let steep_z = z + 1 < depth && (grid.cell_height(x, z + 1) - h).abs() > limit;
            if !steep_x && !steep_z {
                continue;
            }
            if get_bit(&carved, width, x, z) {
                continue;
            }
            let p = grid.cell_center(x, z);
            // Underwater ground keeps its carve: terraced bowls would
            // staircase through the water mirror.
            if water.iter().any(|w| w.contains(p) && h < w.water_y) {
                continue;
            }
            // Region gate: only terrace INSIDE accepted cliff components —
            // a spurious steep ramp in open ground keeps its smooth shape.
            if !mask.is_core(x, z) {
                continue;
            }
            let dither = (hash01(seed, p.x.to_bits() as u64, p.y.to_bits() as u64) - 0.5) * 0.6;
            let q = (h / TERRACE_TREAD + dither).floor() * TERRACE_TREAD;
            grid.set_cell_height(x, z, q);
        }
    }
    grid.commit_stroke()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 128x128 grid, 128 m world (XZ in [-64, 64]), max_height 50.
    fn flat_grid(height: f32) -> BrushGrid {
        let mut grid =
            BrushGrid::new(vec![0; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("base");
        for z in 0..128 {
            for x in 0..128 {
                grid.set_cell_height(x, z, height);
            }
        }
        grid.commit_stroke();
        grid
    }

    /// Crest along the z axis at x = 0 (path direction +z → left normal = −x,
    /// so `Left` drops toward −x and `Right` toward +x).
    fn wall_x0() -> CliffSpec {
        CliffSpec {
            path: vec![Vec2::new(0.0, -50.0), Vec2::new(0.0, 50.0)],
            width: 6.0,
            height: Some(18.0),
            angle: None,
            profile: CliffProfile::Vertical,
            side: CliffSide::Right,
            noise: 0.0,
            gullies: 0.0,
            notches: 0.0,
            talus: false,
            talus_angle: 36.0,
            seed: 7,
        }
    }

    #[test]
    fn test_degenerate_specs_return_none() {
        let mut grid = flat_grid(20.0);
        let mut spec = wall_x0();
        spec.path.clear();
        assert!(carve_cliff(&mut grid, &spec, 0).is_none(), "no path");
        spec.path = vec![Vec2::ZERO];
        assert!(carve_cliff(&mut grid, &spec, 0).is_none(), "one point");
        spec.path = vec![Vec2::ZERO, Vec2::new(10.0, 0.0)];
        spec.width = 0.0;
        assert!(carve_cliff(&mut grid, &spec, 0).is_none(), "zero width");
        spec.width = 6.0;
        spec.angle = Some(0.0);
        assert!(carve_cliff(&mut grid, &spec, 0).is_none(), "zero angle");
    }

    #[test]
    fn test_profile_curves_are_exact() {
        // Vertical S-curve: symmetric, midpoint at half drop.
        assert!((profile_s(0.5, CliffProfile::Vertical, 18.0, 0.0, 0) - 0.5).abs() < 1e-5);
        assert_eq!(profile_s(0.0, CliffProfile::Vertical, 18.0, 0.0, 0), 0.0);
        assert_eq!(profile_s(1.0, CliffProfile::Vertical, 18.0, 0.0, 0), 1.0);
        // Concave: the cumulative drop is front-loaded near the crest
        // (vertical under the crest, leaning back toward the toe).
        assert!(profile_s(0.25, CliffProfile::Concave, 18.0, 0.0, 0) > 0.5);
        assert!(profile_s(0.75, CliffProfile::Concave, 18.0, 0.0, 0) < 1.0);
        // Convex: the mirror — rounded brow (nearly flat under the crest),
        // steepest in the last stretch toward the toe.
        assert!(
            profile_s(0.25, CliffProfile::Convex, 18.0, 0.0, 0) < 0.1,
            "convex brow stays high near the crest, got {}",
            profile_s(0.25, CliffProfile::Convex, 18.0, 0.0, 0)
        );
        assert!(
            profile_s(0.95, CliffProfile::Convex, 18.0, 0.0, 0) > 0.85,
            "convex face dives toward the toe, got {}",
            profile_s(0.95, CliffProfile::Convex, 18.0, 0.0, 0)
        );
        // Terraced: lands on quantized treads (18 m at ~3 m/tread = 6 steps).
        let s = profile_s(1.0, CliffProfile::Terraced, 18.0, 0.0, 0);
        assert!(
            (s - 1.0).abs() < 1e-4,
            "the toe lands the last tread, got {s}"
        );
        // Determinism for the jittered profile.
        assert_eq!(
            profile_s(0.45, CliffProfile::Terraced, 18.0, 3.0, 9),
            profile_s(0.45, CliffProfile::Terraced, 18.0, 3.0, 9)
        );
    }

    #[test]
    fn test_wall_carves_drop_and_keeps_the_top_side() {
        let mut grid = flat_grid(20.0);
        let line = carve_cliff(&mut grid, &wall_x0(), 0).expect("carve");
        assert_eq!(line.stations.len(), line.top_y.len());
        // The design planes are exact: crest at 20, toe pinned to 20 - 18.
        assert!((line.top_y[line.top_y.len() / 2] - 20.0).abs() < 1e-4);
        assert!((line.bot_y[line.bot_y.len() / 2] - 2.0).abs() < 1e-4);
        // Top side untouched (x = -3 is outside the band).
        assert!(
            (grid.sample(-3.0, 0.0) - 20.0).abs() < 0.3,
            "crest side keeps its height, got {}",
            grid.sample(-3.0, 0.0)
        );
        // Inside the band the face descends monotonically crest → toe.
        let mut prev = f32::INFINITY;
        for x in [0.5, 1.5, 2.5, 3.5, 4.5, 5.5] {
            let h = grid.sample(x, 0.0);
            assert!(
                h <= prev + 0.3,
                "face never climbs at x={x}: {h} after {prev}"
            );
            prev = h;
        }
        // At the end of the band the face has landed on the toe plane
        // (authoring a drop on flat ground digs a quarry-style pit; past the
        // fade ring the natural ground continues).
        let landed = grid.sample(5.0, 0.0);
        assert!(
            (landed - 2.0).abs() < 0.8,
            "face landed on the toe, got {landed}"
        );
        assert!(
            (grid.sample(12.0, 0.0) - 20.0).abs() < 0.3,
            "past the band the ground is untouched, got {}",
            grid.sample(12.0, 0.0)
        );
    }

    #[test]
    fn test_concave_profile_steepens_toward_the_crest() {
        let mut grid = flat_grid(20.0);
        let mut spec = wall_x0();
        spec.profile = CliffProfile::Concave;
        carve_cliff(&mut grid, &spec, 0).expect("carve");
        // Slope comparison: drop-per-meter nearer the crest (x 1→2) beats
        // drop-per-meter nearer the toe (x 4→5) by a clear margin.
        let upper = grid.sample(1.0, 0.0) - grid.sample(2.0, 0.0);
        let lower = grid.sample(4.0, 0.0) - grid.sample(5.0, 0.0);
        assert!(
            upper > lower * 1.4,
            "concave face is steeper under the crest: upper {upper} vs lower {lower}"
        );
    }

    #[test]
    fn test_terraced_profile_quantizes_the_face() {
        let mut grid = flat_grid(20.0);
        let mut spec = wall_x0();
        spec.profile = CliffProfile::Terraced;
        carve_cliff(&mut grid, &spec, 0).expect("carve");
        // Walk down the band and count the distinct plateaus: 18 m drop at
        // ~3 m/tread = 6 steps, allow jitter slack.
        let mut levels: Vec<i32> = Vec::new();
        for x in [1.0, 2.0, 3.0, 4.0, 5.0] {
            levels.push((grid.sample(x, 0.0) / 3.0).round() as i32);
        }
        levels.dedup();
        assert!(
            levels.len() <= 7,
            "terraced face has ~6 treads, saw {levels:?}"
        );
        // Monotone descent inside the band.
        let mut prev = 20.0;
        for x in [0.5, 1.5, 2.5, 3.5, 4.5, 5.5] {
            let h = grid.sample(x, 0.0);
            assert!(
                h <= prev + 0.4,
                "face never climbs back up at x={x}: {h} after {prev}"
            );
            prev = h;
        }
    }

    #[test]
    fn test_columnar_profile_cuts_straight_slabs() {
        // Unit: the facet curve is monotone with kinks, endpoints exact.
        let col = Column::new(9, 3, 0.0, 4.0);
        assert_eq!(facet_s(0.0, &col), 0.0);
        assert!((facet_s(1.0, &col) - 1.0).abs() < 1e-5);
        let mut prev = -1.0;
        for i in 0..=20 {
            let v = facet_s(i as f32 / 20.0, &col);
            assert!(v >= prev - 1e-6, "facet curve must be monotone");
            prev = v;
        }
        // Kinks really kink (piecewise linear: the derivative changes sign of
        // slope across 0.42/0.72 for most hash draws).
        assert!(facet_s(0.42, &col) - col.k1 < 1e-5);
        assert!(facet_s(0.72, &col) - col.k2 < 1e-5);
        // Columns differ (offset/jitter/kinks are hash draws).
        let a = Column::new(9, 0, 0.0, 4.0);
        let b = Column::new(9, 1, 0.0, 4.0);
        assert!((a.k1 - b.k1).abs() > 0.001 || (a.off - b.off).abs() > 0.001);
    }

    /// A columnar wall on flat ground: the crest silhouette is crenellated
    /// (per-column offset + jitter give distinct brow levels), the face
    /// descends monotonically, and the same seed reproduces byte-identical
    /// walls.
    #[test]
    fn test_columnar_wall_reads_as_columns() {
        let run = |seed: u64| {
            let mut grid = flat_grid(20.0);
            let mut spec = wall_x0();
            spec.profile = CliffProfile::Columnar;
            spec.width = 9.0;
            spec.height = Some(18.0);
            spec.seed = seed;
            carve_cliff(&mut grid, &spec, 0).expect("carve");
            grid.raw().to_vec()
        };
        let mut grid = flat_grid(20.0);
        let mut spec = wall_x0();
        spec.profile = CliffProfile::Columnar;
        spec.width = 9.0;
        spec.height = Some(18.0);
        spec.seed = 12;
        carve_cliff(&mut grid, &spec, 0).expect("carve");
        // Crest brow: heights just below the crest line (x = 1) vary along z
        // — the crenellated silhouette. A smooth wall would be near-constant.
        let brow: Vec<f32> = (-40..=-20).map(|z| grid.sample(1.0, z as f32)).collect();
        let lo = brow.iter().cloned().fold(f32::INFINITY, f32::min);
        let hi = brow.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            hi - lo > 1.2,
            "columnar brow should vary along the wall, range {}",
            hi - lo
        );
        // Monotone descent across the band (column averages, not per texel —
        // the slabs are flat but the steps only go down).
        let mut prev = 20.0;
        for x in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0] {
            let mut h = 0.0;
            for z in -40..=40 {
                h += grid.sample(x, z as f32);
            }
            let h = h / 81.0;
            assert!(
                h <= prev + 0.6,
                "column-averaged face never climbs at x={x}: {h} after {prev}"
            );
            prev = h;
        }
        // Determinism.
        assert_eq!(run(12), run(12), "same seed, same columns");
    }

    /// Terraced v2 quantizes by RUN: a 6 m band gets ~2 steps of ~3 m bench
    /// (the flat treads that catch the grass splat), whatever the drop.
    #[test]
    fn test_terraced_steps_quantize_by_run() {
        let mut grid = flat_grid(20.0);
        let mut spec = wall_x0();
        spec.profile = CliffProfile::Terraced;
        spec.width = 6.0;
        spec.height = Some(18.0);
        spec.seed = 5;
        carve_cliff(&mut grid, &spec, 0).expect("carve");
        // Distinct plateaus along the band at mid-wall: ~2 steps → the face
        // visits ≤ 4 rounded levels.
        let mut levels: Vec<i32> = Vec::new();
        for x in [1.5, 2.5, 3.5, 4.5, 5.5] {
            levels.push((grid.sample(x, 0.0) / 2.0).round() as i32);
        }
        levels.sort();
        levels.dedup();
        assert!(
            levels.len() <= 4,
            "run-quantized treads: few plateaus, saw {levels:?}"
        );
    }

    #[test]
    fn test_authored_angle_derives_the_band_width() {
        let mut grid = flat_grid(20.0);
        let mut spec = wall_x0();
        // 45° with 18 m of drop → 18 m run: the toe sits at x ≈ 18.
        spec.angle = Some(45.0);
        let line = carve_cliff(&mut grid, &spec, 0).expect("carve");
        assert!((line.width - 18.0).abs() < 1e-3, "width from angle");
        let toe = grid.sample(16.0, 0.0);
        assert!(toe < 19.5, "the wide band reaches x=16 at 45°, got {toe}");
    }

    /// Auto height on a stepped hillside: the wall keeps whatever drop the
    /// place already has (here 12 m) instead of demanding an authored one.
    #[test]
    fn test_auto_height_uses_the_natural_drop() {
        let mut grid =
            BrushGrid::new(vec![0; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("base");
        for z in 0..128 {
            for x in 0..128 {
                let h = if grid.cell_center(x, z).x < 0.0 {
                    26.0
                } else {
                    14.0
                };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        let mut spec = wall_x0();
        spec.side = CliffSide::Auto;
        spec.height = None;
        let line = carve_cliff(&mut grid, &spec, 0).expect("carve");
        // The crest/toe planes follow the terrain (probes read the plateaus).
        assert!(
            (line.top_y[line.top_y.len() / 2] - 26.0).abs() < 0.5,
            "crest reads the top side, got {}",
            line.top_y[line.top_y.len() / 2]
        );
        assert!(
            (line.bot_y[line.bot_y.len() / 2] - 14.0).abs() < 0.5,
            "toe reads the drop side, got {}",
            line.bot_y[line.bot_y.len() / 2]
        );
        // Mid face halfway between the planes (12 m drop → 20 m).
        let mid = grid.sample(3.0, 0.0);
        assert!(
            (mid - 20.0).abs() < 1.5,
            "mid face halfway between 26 and 14, got {mid}"
        );
    }

    #[test]
    fn test_auto_side_picks_the_lower_side() {
        let mut grid =
            BrushGrid::new(vec![0; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("base");
        for z in 0..128 {
            for x in 0..128 {
                let h = if grid.cell_center(x, z).x < 0.0 {
                    30.0
                } else {
                    12.0
                };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        let mut spec = wall_x0();
        spec.side = CliffSide::Auto;
        spec.height = None;
        let line = carve_cliff(&mut grid, &spec, 0).expect("carve");
        // +x is lower → the crest plane reads 30 and the face drops toward
        // +x (toe plane 12): identical to the forced `Right` pairing.
        assert!(
            (line.top_y[line.top_y.len() / 2] - 30.0).abs() < 0.5,
            "crest reads the high side, got {}",
            line.top_y[line.top_y.len() / 2]
        );
        assert!(
            (line.bot_y[line.bot_y.len() / 2] - 12.0).abs() < 0.5,
            "auto side drops toward +x, got {}",
            line.bot_y[line.bot_y.len() / 2]
        );
        // And −x (the untouched crest side) keeps 30.
        assert!((grid.sample(-3.0, 0.0) - 30.0).abs() < 0.4);
    }

    #[test]
    fn test_revert_and_recarve_is_idempotent() {
        let mut grid = flat_grid(20.0);
        let before = grid.raw().to_vec();
        carve_cliff(&mut grid, &wall_x0(), 0).expect("carve");
        let carved = grid.raw().to_vec();
        assert_ne!(carved, before, "the wall moved texels");
        assert!(
            grid.revert_last_stroke("cliff:0"),
            "the journal knows the stroke"
        );
        assert_eq!(grid.raw(), before, "revert restores the field");
        carve_cliff(&mut grid, &wall_x0(), 0).expect("recarve");
        assert_eq!(grid.raw(), carved, "same seed, same wall");
    }

    #[test]
    fn test_noise_wobbles_but_stays_deterministic() {
        let run = |noise: f32, seed: u64| {
            let mut grid = flat_grid(20.0);
            let mut spec = wall_x0();
            spec.noise = noise;
            spec.seed = seed;
            carve_cliff(&mut grid, &spec, 0).expect("carve");
            grid.raw().to_vec()
        };
        assert_eq!(run(0.3, 7), run(0.3, 7), "same seed, same wobble");
        assert_eq!(run(0.0, 7), run(0.0, 99), "no noise ignores the seed");
        // With noise the band breathes, but the face still descends from the
        // crest plane toward the toe plane and the crest side stays intact.
        let mut grid = flat_grid(20.0);
        let mut spec = wall_x0();
        spec.noise = 0.3;
        carve_cliff(&mut grid, &spec, 0).expect("carve");
        assert!((grid.sample(-3.0, 0.0) - 20.0).abs() < 0.3, "crest kept");
        let mut prev = f32::INFINITY;
        for x in [0.5, 1.5, 2.5, 3.5, 4.5, 5.5] {
            let h = grid.sample(x, 0.0);
            assert!(h <= prev + 0.4, "wobbled face never climbs at x={x}");
            prev = h;
        }
    }

    /// The mask marks a 4-texel wall block (a real cliff region) and rejects
    /// BOTH a 1-texel needle (speckle, removed by the opening) and a gentle
    /// 12 m ramp spread over 40 m (well under the 50° default trigger).
    #[test]
    fn test_cliff_mask_marks_steep_drops_only() {
        let mut grid =
            BrushGrid::new(vec![0u16; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("field");
        for z in 0..128 {
            for x in 0..128 {
                let cx = grid.cell_center(x, z).x;
                let h = if cx > 30.0 && cx <= 50.0 {
                    30.0 - (cx - 30.0) * 1.43 // ~55° face, ~20 texels wide
                } else if (cx - 62.5).abs() < 1.0 && (grid.cell_center(x, 70).y).abs() < 3.0 {
                    30.0 // SHORT needle (2×4 texels, ~8 m²) — speckle
                } else {
                    10.0 + cx.clamp(0.0, 40.0) * (12.0 / 40.0) // gentle ramp
                };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        let mask = CliffMask::build_with(&grid, 50.0, 120.0, 4.0, 8.0);
        // The wide face survives opening + the region filter (interior texels).
        assert!(
            mask.is_core(95, 64) && mask.is_core(103, 64) && mask.is_core(110, 64),
            "the wide face is a cliff region"
        );
        // A short needle is speckle: the AREA filter removes it (the opening
        // was removed — it would also erase terraced risers, which are
        // legitimate 1-texel features).
        assert!(
            !mask.is_core(126, 70) && !mask.is_cliff(126, 70),
            "short needle is speckle, not a cliff"
        );
        // Gentle ramp texels far from the wall are clear.
        assert!(!mask.is_cliff(80, 20), "12 m over 40 m is ~17°, not cliff");
        // World-space query maps back to the face region.
        assert!(mask.is_cliff_at(Vec2::new(40.0, 0.0)));
        assert!(!mask.is_cliff_at(Vec2::new(-30.0, 0.0)));
        // Angle 0 disables the scan entirely.
        assert_eq!(
            CliffMask::build_with(&grid, 0.0, 120.0, 4.0, 8.0).marked(),
            0
        );
    }

    /// `is_cliff_within` measures real meters off the wall — the spawn-gate
    /// margin — instead of the fixed 2-texels of `is_cliff_at`.
    #[test]
    fn test_is_cliff_within_measures_real_distance() {
        let mut grid =
            BrushGrid::new(vec![0u16; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("field");
        for z in 0..128 {
            for x in 0..128 {
                let cx = grid.cell_center(x, z).x;
                let h = if cx > 30.0 && cx <= 50.0 {
                    30.0 - (cx - 30.0) * 1.43 // ~55° face, ~20 texels wide
                } else {
                    10.0 + cx.clamp(0.0, 40.0) * (12.0 / 40.0) // gentle ramp
                };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        let mask = CliffMask::build_with(&grid, 50.0, 120.0, 4.0, 8.0);
        assert!(mask.marked() > 0, "the wide face registers as cliff");
        assert!(
            (mask.texel_size() - 128.0 / 127.0).abs() < 1e-3,
            "texel size follows the vertex convention, got {}",
            mask.texel_size()
        );
        // On the face every margin hits; zero meters matches `is_cliff_at`.
        let face = Vec2::new(40.0, 0.0);
        assert!(mask.is_cliff_within(face, 2.0));
        assert_eq!(mask.is_cliff_within(face, 0.0), mask.is_cliff_at(face));
        // Walk +x past the toe and find where the 2 m query flips.
        let mut flip = None;
        let mut x = 52.0;
        while x < 90.0 {
            if !mask.is_cliff_within(Vec2::new(x, 0.0), 2.0) {
                flip = Some(x);
                break;
            }
            x += 0.25;
        }
        let flip = flip.expect("the wall ends; the 2 m ring clears");
        assert!(!mask.is_cliff_at(Vec2::new(flip, 0.0)), "flip is off-mask");
        // 1.5 m back toward the wall must be inside the 2 m ring again.
        assert!(
            mask.is_cliff_within(Vec2::new(flip - 1.5, 0.0), 2.0),
            "just past the flip the wall is within 2 m"
        );
        // Margins scale: far ground only enters wide rings.
        let far = Vec2::new(-30.0, 0.0); // ~60 m from the face
        assert!(!mask.is_cliff_within(far, 40.0));
        assert!(mask.is_cliff_within(far, 200.0));
    }

    /// A steep but SMALL bump (under the min area) is filtered out: neither
    /// the mask marks it nor does the sharpen pass terrace it — the spurious
    /// "cliff in the middle of nowhere" the region filter exists for.
    #[test]
    fn test_small_steep_bump_is_filtered_everywhere() {
        let mut grid =
            BrushGrid::new(vec![0u16; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("field");
        for z in 0..128 {
            for x in 0..128 {
                grid.set_cell_height(x, z, 10.0);
            }
        }
        // A 3x3-texel, 20 m tall spike: steep everywhere, tiny area.
        for z in 63..=65 {
            for x in 63..=65 {
                grid.set_cell_height(x, z, 30.0);
            }
        }
        grid.commit_stroke();
        let before = grid.raw().to_vec();
        let spec = crate::terrain::TerrainSpec {
            sharpen: true,
            sharpen_angle: 35.0,
            sharpen_seed: 5,
            ..crate::terrain::TerrainSpec::default()
        };
        let mask = CliffMask::build(&grid, &spec);
        assert_eq!(mask.marked(), 0, "3x3 spike is speckle, not a cliff");
        let changed = sharpen_terrain(&mut grid, &spec, &[], &mask, &[]);
        assert_eq!(changed, 0, "sharpen respects the region gate");
        assert_eq!(grid.raw(), before);
    }

    /// The sharpen terraces a smooth 20 m ramp into ~3 m treads,
    /// leaves the flat plateau exact, and reverts byte-identically.
    #[test]
    fn test_sharpen_terraces_steep_ramps_only() {
        let mut grid =
            BrushGrid::new(vec![0u16; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("field");
        for z in 0..128 {
            for x in 0..128 {
                let cx = grid.cell_center(x, z).x;
                // Flat plateau up to x=0, then a 45° ramp for 20 m, then flat.
                let h = if cx <= 0.0 {
                    25.0
                } else {
                    25.0 - (cx.min(20.0)) * 1.0
                };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        let before = grid.raw().to_vec();
        let mut spec = crate::terrain::TerrainSpec::default();
        spec.sharpen = true;
        spec.sharpen_angle = 35.0;
        spec.sharpen_seed = 5;
        let pre_mask = CliffMask::build_with(&grid, 35.0, 120.0, 4.0, 8.0);
        let changed = sharpen_terrain(&mut grid, &spec, &[], &pre_mask, &[]);
        assert!(changed > 0, "the ramp is 45° — well over the trigger");
        // Plateau untouched (flat texels never trigger; 25 m is not a tread
        // multiple on purpose — it survives because nothing was steep).
        let plateau = grid.cell_height(20, 64);
        assert!(
            (plateau - 25.0).abs() < 0.01,
            "flat plateau keeps its height, got {plateau}"
        );
        // Ramp texels land on tread multiples (±dither never crosses a tread).
        for x in 70..80 {
            let h = grid.cell_height(x, 64);
            if h > 4.5 {
                let off = (h / TERRACE_TREAD) - (h / TERRACE_TREAD).floor();
                assert!(
                    off < 1e-3 || off > 0.999,
                    "ramp texel {x} sits mid-tread: {h}"
                );
            }
        }
        // Idempotency via the journal: revert → re-run lands the same bytes.
        assert!(grid.revert_last_stroke("sharpen"));
        assert_eq!(grid.raw(), before, "revert restores the field");
        let mask = CliffMask::build_with(&grid, 35.0, 120.0, 4.0, 8.0);
        sharpen_terrain(&mut grid, &spec, &[], &mask, &[]);
        // The pre-sharpen mask stays valid as the FINAL mask: sharpening
        // only adds steps inside its core (rebuilding over the terraced
        // field would fragment into riser slivers). The terraced walls
        // register through it.
        assert!(mask.marked() > 0, "terraced walls register as cliff");
    }

    /// Sub-water texels never terrace: lake bowls stay glassy.
    #[test]
    fn test_sharpen_skips_underwater_texels() {
        let mut grid =
            BrushGrid::new(vec![0u16; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("field");
        for z in 0..128 {
            for x in 0..128 {
                let cx = grid.cell_center(x, z).x;
                let h = if cx <= 0.0 { 25.0 } else { 25.0 - cx.min(20.0) };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        // A water body covering the whole ramp with its mirror above ground.
        let water = vec![super::super::water::WaterBody {
            kind: super::super::water::WaterKind::Lake,
            at: Vec2::new(10.0, 0.0),
            radius: 300.0,
            carve_radius: 300.0,
            water_y: 30.0,
            stations: Vec::new(),
            surface_y: Vec::new(),
            water_width: 0.0,
            half_width: Vec::new(),
            cascades: Vec::new(),
        }];
        let mut spec = crate::terrain::TerrainSpec::default();
        spec.sharpen = true;
        spec.sharpen_angle = 35.0;
        spec.sharpen_seed = 5;
        let pre_mask = CliffMask::build(&grid, &spec);
        let changed = sharpen_terrain(&mut grid, &spec, &water, &pre_mask, &[]);
        assert_eq!(changed, 0, "every steep texel is underwater here");
    }

    /// Gullies cut the face ONE-SIDEDLY (the band never grows past the
    /// nominal toe) but never erase the wall, and the same seed carves
    /// byte-identical faces.
    #[test]
    fn test_gullies_recess_the_face_and_stay_deterministic() {
        let carve = |gullies: f32, seed: u64| {
            let mut grid = flat_grid(20.0);
            let mut spec = wall_x0();
            spec.gullies = gullies;
            spec.seed = seed;
            carve_cliff(&mut grid, &spec, 0).expect("carve");
            grid.raw().to_vec()
        };
        // Determinism.
        assert_eq!(carve(0.3, 7), carve(0.3, 7), "same seed, same world");
        assert_ne!(carve(0.3, 7), carve(0.3, 8), "seed varies the gullies");

        let extent = |raw: &[u16], z: usize| -> f32 {
            // Farthest carved texel from the crest along +x (drop side),
            // converted from cell index to world meters (cells start at
            // −world/2).
            let mut last = 0.0f32;
            for x in 0..128 {
                let h = raw[z * 128 + x];
                if h < 19_000 {
                    last = x as f32 - 64.0;
                }
            }
            last
        };
        let with_gullies: Vec<u16> = carve(0.3, 7);
        // Scan a band of rows: at gully arcs the toe recedes well before
        // the nominal x=6; at buttress arcs it stays close to it.
        let mut min_toe = f32::INFINITY;
        let mut max_toe = 0.0f32;
        for z in 40..88 {
            let toe = extent(&with_gullies, z);
            min_toe = min_toe.min(toe);
            max_toe = max_toe.max(toe);
        }
        assert!(
            max_toe < 8.5,
            "gullies never extend the band, got toe at x={max_toe}"
        );
        assert!(
            min_toe < 6.8,
            "gullies actually recess the face somewhere, min toe {min_toe}"
        );
        assert!(
            min_toe > 2.5,
            "buttresses keep most of the wall, min toe {min_toe}"
        );
    }

    /// Crest notches dip the brow by a bounded fraction of the LOCAL drop.
    #[test]
    fn test_notches_dip_the_crest_within_budget() {
        // Across seeds the harmonics align somewhere: at least one cliff
        // grows a real col (the budget itself is checked per station).
        let mut best_dip = 0.0f32;
        for seed in 0..8u64 {
            let mut grid = flat_grid(20.0);
            let mut spec = wall_x0();
            spec.notches = 0.25;
            spec.seed = seed;
            let line = carve_cliff(&mut grid, &spec, 0).expect("carve");
            for (i, top) in line.top_y.iter().enumerate() {
                let dip = 20.0 - top;
                best_dip = best_dip.max(dip);
                let drop = (top + dip - line.bot_y[i]).max(0.0);
                assert!(
                    dip <= 0.26 * drop,
                    "dip {dip} exceeds the notch budget at station {i}"
                );
                assert!(dip >= -1e-4, "notches only dip, never raise: {dip}");
            }
        }
        assert!(
            best_dip > 1.0,
            "some seed grows a real col in the silhouette, max dip {best_dip}"
        );

        // The carve follows the registry: somewhere the crest terrain sits
        // below the un-notched 20.
        let mut grid = flat_grid(20.0);
        let mut spec = wall_x0();
        spec.notches = 0.25;
        spec.seed = 3;
        carve_cliff(&mut grid, &spec, 0).expect("carve");
        let mut min_crest = f32::INFINITY;
        for z in -48..48 {
            let z = z as f32;
            min_crest = min_crest.min(grid.sample(0.0, z));
        }
        assert!(
            min_crest < 19.0,
            "the crest terrain shows a notch, min {min_crest}"
        );
    }

    /// Talus: the apron piles debris against the toe (raise-only — it never
    /// quarries the ground down), never stacks past `bury`, and joins the
    /// mask's public layer (gravel/exclusion).
    #[test]
    fn test_talus_builds_an_apron_at_the_toe() {
        // A natural step: high ground at x<0, valley floor at x>=0 — the
        // auto wall takes the 18 m drop, the talus spreads over the valley.
        let stepped = || {
            let mut grid =
                BrushGrid::new(vec![0u16; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
            grid.begin_stroke("step");
            for z in 0..128 {
                for x in 0..128 {
                    let cx = grid.cell_center(x, z).x;
                    let h = if cx < 0.0 { 20.0 } else { 2.0 };
                    grid.set_cell_height(x, z, h);
                }
            }
            grid.commit_stroke();
            grid
        };
        let mut spec = wall_x0();
        spec.height = None;
        spec.noise = 0.0;
        spec.side = CliffSide::Auto; // probes pair across the natural step
        spec.talus = true;
        let mut grid = stepped();
        let line = carve_cliff(&mut grid, &spec, 0).expect("carve");
        let max_run = line.talus_run.iter().copied().fold(0.0, f32::max);
        assert!(
            max_run > 8.0 && max_run <= 40.0,
            "an 18 m drop at 36° spreads ~13 m of scree, got {max_run}"
        );

        // Without talus the valley floor stays at 2.
        let mut plain = stepped();
        let mut plain_spec = wall_x0();
        plain_spec.height = None;
        plain_spec.noise = 0.0;
        plain_spec.side = CliffSide::Auto;
        carve_cliff(&mut plain, &plain_spec, 0).expect("carve");

        // Debris piled against the wall base, dying out toward the run.
        let mut piled = 0.0f32;
        for x in [7.0, 8.5, 10.0, 12.0, 15.0, 18.0] {
            let h = grid.sample(x, 0.0);
            let p = plain.sample(x, 0.0);
            assert!(
                h >= p - 0.3,
                "the apron never quarries the valley, got {h} vs {p} at x={x}"
            );
            assert!(
                h <= 9.6,
                "the pile stays under bury (0.35·drop ≈ 6.3 m), got {h} at x={x}"
            );
            piled = piled.max(h - p);
        }
        assert!(
            piled > 1.5,
            "the apron actually piles debris, max raise {piled}"
        );
        assert!(
            (grid.sample(24.0, 0.0) - 2.0).abs() < 0.4,
            "past the apron the valley floor is untouched, got {}",
            grid.sample(24.0, 0.0)
        );

        // Mask: the apron bitset feeds the public layer (splat gravel +
        // grass/spawner exclusion), NOT the core (the wall keeps its own
        // solid-rock rule).
        let mask = CliffMask::build_with(&grid, 50.0, 120.0, 4.0, 8.0);
        let mut mask = mask;
        mask.add_talus(&[line.clone()]);
        assert!(
            mask.is_talus_at(Vec2::new(8.0, 0.0)),
            "apron texels are talus"
        );
        assert!(
            !mask.is_talus_at(Vec2::new(-8.0, 0.0)),
            "the crest side has no apron"
        );
        assert!(
            mask.is_cliff_at(Vec2::new(8.0, 0.0)),
            "talus joins the public exclusion layer"
        );
        assert!(
            !mask.is_core_at(Vec2::new(8.0, 0.0)),
            "the apron is not wall core"
        );
        assert!(
            (mask.wall_at(Vec2::new(8.0, 0.0)) - 0.5).abs() < 0.01,
            "the apron keeps a neutral wall space"
        );
    }

    /// Wall space reads the local drop: ~0 at the brow, ~1 at the toe,
    /// neutral off-mask.
    #[test]
    fn test_wall_space_spans_brow_to_toe() {
        // A natural step: high ground at x<0, valley at x>=0 — the auto
        // wall adapts to the 18 m difference.
        let mut grid =
            BrushGrid::new(vec![0u16; 128 * 128], 128, 128, 128.0, 50.0, 1.0).expect("grid");
        grid.begin_stroke("step");
        for z in 0..128 {
            for x in 0..128 {
                let cx = grid.cell_center(x, z).x;
                let h = if cx < 0.0 { 20.0 } else { 2.0 };
                grid.set_cell_height(x, z, h);
            }
        }
        grid.commit_stroke();
        let mut spec = wall_x0();
        spec.height = None; // auto: the wall takes the natural drop
        spec.noise = 0.0;
        spec.side = CliffSide::Auto; // let the probes pair across the step
        let line = carve_cliff(&mut grid, &spec, 0).expect("carve");
        assert!((line.top_y[25] - 20.0).abs() < 0.5, "brow on the plateau");
        assert!((line.bot_y[25] - 2.0).abs() < 0.5, "toe in the valley");

        let spec = crate::terrain::TerrainSpec {
            cliff_angle: 50.0,
            ..crate::terrain::TerrainSpec::default()
        };
        let mask = CliffMask::build(&grid, &spec);
        let brow = mask.wall_at(Vec2::new(0.8, 0.0));
        let toe = mask.wall_at(Vec2::new(4.8, 0.0));
        assert!(
            brow < 0.4,
            "the brow sits near the top of the local drop, got {brow}"
        );
        assert!(
            toe > 0.6,
            "the toe sits near the bottom of the local drop, got {toe}"
        );
        assert!(
            (mask.wall_at(Vec2::new(-30.0, 0.0)) - 0.5).abs() < 0.01,
            "off-mask terrain reads neutral"
        );
    }
}
