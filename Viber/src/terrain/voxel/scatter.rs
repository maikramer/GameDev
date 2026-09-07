//! `<RockFeatures>` — arches, caves and rock bridges seeded over a region.
//!
//! Every volumetric feature so far is authored point by point: someone picks
//! the XZ, reads the terrain, and writes the tag. That is the right tool for
//! the landmark you meant, and the wrong one for the twenty background
//! features that make a badlands read as badlands.
//!
//! This tag seeds them. It is **not** a new kind of solid: it resolves into
//! ordinary [`ArchSpec`], [`CaveSpec`] and [`BridgeSpec`] values, which then
//! take exactly the same path through the bootstrap as if they had been
//! typed by hand. Nothing downstream knows the difference, so nothing
//! downstream had to change.
//!
//! # Determinism
//!
//! The seeding is a pure function of `(spec, height field, guards)`: a
//! jittered lattice hashed with [`hash01`], the same generator the cliffs use.
//! No RNG state, no iteration order from a hash map — the same world authored
//! twice seeds the same rocks in the same places, which is the contract every
//! [`VoxelMod`](super::mods::VoxelMod) build already keeps.
//!
//! ```xml
//! <RockFeatures name="badlands" region="-200 -200 200 200" seed="9"
//!               arches="6" caves="3" bridges="2"
//!               min-slope="25" max-slope="70" min-drop="6" spacing="40" />
//! ```

use bevy::math::Vec2;

use super::super::cliffs::hash01;
use super::super::mesh::HeightField;
use super::super::roads::RoadPath;
use super::super::water::WaterBody;
use super::arch::{ArchProfile, ArchSpec};
use super::bridge::{BridgeSpec, BridgeStyle};
use super::cave::CaveSpec;

/// Default spacing of the candidate lattice (meters).
pub const DEFAULT_SCATTER_SPACING: f32 = 40.0;
/// Default slope band a feature will accept, in degrees.
pub const DEFAULT_MIN_SLOPE: f32 = 22.0;
pub const DEFAULT_MAX_SLOPE: f32 = 72.0;
/// Default height difference a site must show over its own neighbourhood.
pub const DEFAULT_MIN_DROP: f32 = 5.0;
/// Default clearance kept from any road ribbon (meters).
pub const DEFAULT_CLEAR_OF_ROADS: f32 = 10.0;

/// Directions probed when looking for a gap a bridge could cross.
const CROSSING_DIRECTIONS: usize = 8;
/// Epsilon for the slope probe (meters).
const SLOPE_EPS: f32 = 1.5;

/// What the seeding is allowed to avoid — the registries the feature pass
/// already produced.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScatterGuards<'a> {
    pub water: &'a [WaterBody],
    pub roads: &'a [RoadPath],
}

/// Declarative `<RockFeatures>` spec.
#[derive(Debug, Clone, PartialEq)]
pub struct RockFeaturesSpec {
    pub name: Option<String>,
    /// World XZ bounds to seed inside.
    pub min: Vec2,
    pub max: Vec2,
    pub seed: u64,
    pub arches: u32,
    pub caves: u32,
    pub bridges: u32,
    /// Slope band a site must fall in, in degrees.
    pub min_slope: f32,
    pub max_slope: f32,
    /// Height difference a site must show over its own neighbourhood.
    pub min_drop: f32,
    /// Candidate lattice pitch, and the minimum distance between two seeded
    /// features (meters).
    pub spacing: f32,
    /// Clearance kept from any road ribbon (meters).
    pub clear_of_roads: f32,
}

impl Default for RockFeaturesSpec {
    fn default() -> Self {
        Self {
            name: None,
            min: Vec2::splat(-100.0),
            max: Vec2::splat(100.0),
            seed: 0,
            arches: 0,
            caves: 0,
            bridges: 0,
            min_slope: DEFAULT_MIN_SLOPE,
            max_slope: DEFAULT_MAX_SLOPE,
            min_drop: DEFAULT_MIN_DROP,
            spacing: DEFAULT_SCATTER_SPACING,
            clear_of_roads: DEFAULT_CLEAR_OF_ROADS,
        }
    }
}

/// The authored-equivalent specs a `<RockFeatures>` resolved into.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScatterResult {
    pub arches: Vec<ArchSpec>,
    pub caves: Vec<CaveSpec>,
    pub bridges: Vec<BridgeSpec>,
}

impl ScatterResult {
    pub fn is_empty(&self) -> bool {
        self.arches.is_empty() && self.caves.is_empty() && self.bridges.is_empty()
    }

    pub fn len(&self) -> usize {
        self.arches.len() + self.caves.len() + self.bridges.len()
    }
}

/// One accepted lattice site.
#[derive(Debug, Clone, Copy)]
struct Site {
    at: Vec2,
    /// Lattice cell, part of the deterministic ordering key.
    cell: (i64, i64),
    /// Terrain slope in degrees.
    slope: f32,
    /// Slope falls in the authored band. Arches and caves need it; a bridge
    /// does not — the site of a crossing is the floor of the gap, which is
    /// flat by definition.
    slope_ok: bool,
    /// Unit XZ direction of steepest ascent.
    uphill: Vec2,
    /// Shuffle key — deterministic, but not correlated with position.
    score: f32,
}

impl RockFeaturesSpec {
    /// How many features this spec asks for in total.
    pub fn requested(&self) -> u32 {
        self.arches + self.caves + self.bridges
    }

    /// Seeds the region and returns ordinary feature specs.
    ///
    /// Reads the height field and the guards up front; like every mod build,
    /// it never touches mutable state.
    pub fn resolve(&self, base: &dyn HeightField, guards: &ScatterGuards) -> ScatterResult {
        let mut out = ScatterResult::default();
        if self.requested() == 0 || self.spacing <= 0.0 || !self.spacing.is_finite() {
            return out;
        }
        let sites = self.sites(base, guards);
        // Bridges are the fussiest — they need a gap to cross — so they pick
        // first. Arches next, caves last: a cave only needs a hillside, which
        // is the most common site.
        let mut taken: Vec<Vec2> = Vec::new();
        let mut used = vec![false; sites.len()];
        for (i, site) in sites.iter().enumerate() {
            if out.bridges.len() as u32 >= self.bridges {
                break;
            }
            if self.too_close(&taken, site.at) {
                continue;
            }
            if let Some((a, b)) = self.crossing(base, site) {
                out.bridges.push(self.bridge_at(site, a, b, out.bridges.len()));
                taken.push(site.at);
                used[i] = true;
            }
        }
        for (i, site) in sites.iter().enumerate() {
            if out.arches.len() as u32 >= self.arches {
                break;
            }
            if used[i] || !site.slope_ok || self.too_close(&taken, site.at) {
                continue;
            }
            out.arches.push(self.arch_at(site, out.arches.len()));
            taken.push(site.at);
            used[i] = true;
        }
        for (i, site) in sites.iter().enumerate() {
            if out.caves.len() as u32 >= self.caves {
                break;
            }
            if used[i] || !site.slope_ok || self.too_close(&taken, site.at) {
                continue;
            }
            out.caves.push(self.cave_at(site, out.caves.len()));
            taken.push(site.at);
        }
        out
    }

    fn too_close(&self, taken: &[Vec2], p: Vec2) -> bool {
        taken.iter().any(|q| q.distance(p) < self.spacing)
    }

    /// Every lattice cell that clears the relief, water and road tests,
    /// ordered deterministically by its hash.
    ///
    /// The slope band is recorded per site rather than filtered here: an arch
    /// or a cave needs a hillside, but a bridge's site is the FLOOR of the gap
    /// it crosses, which is flat by definition. Filtering on slope up front
    /// would have made rock spans impossible to seed.
    fn sites(&self, base: &dyn HeightField, guards: &ScatterGuards) -> Vec<Site> {
        let (lo, hi) = (self.min.min(self.max), self.min.max(self.max));
        let cols = (((hi.x - lo.x) / self.spacing).floor() as i64).max(0);
        let rows = (((hi.y - lo.y) / self.spacing).floor() as i64).max(0);
        let mut out = Vec::new();
        for iz in 0..=rows {
            for ix in 0..=cols {
                // Jitter inside the cell so the field does not read as a grid.
                let jx = hash01(self.seed, ix as u64, (iz as u64) ^ 0x9e37);
                let jz = hash01(self.seed ^ 0x51ed, ix as u64, iz as u64);
                let at = Vec2::new(
                    lo.x + (ix as f32 + jx) * self.spacing,
                    lo.y + (iz as f32 + jz) * self.spacing,
                );
                if at.x > hi.x || at.y > hi.y {
                    continue;
                }
                if guards.water.iter().any(|w| w.contains(at)) {
                    continue;
                }
                if guards
                    .roads
                    .iter()
                    .any(|r| r.distance_to_road(at) < self.clear_of_roads)
                {
                    continue;
                }
                if self.local_drop(base, at) < self.min_drop {
                    continue;
                }
                let n = base.sample_normal(at.x, at.y, SLOPE_EPS);
                let slope = n.y.clamp(-1.0, 1.0).acos().to_degrees();
                // The surface normal tilts downhill, so its XZ part points the
                // way water runs; negate it to face the climb.
                let uphill = Vec2::new(-n.x, -n.z).try_normalize().unwrap_or(Vec2::X);
                out.push(Site {
                    at,
                    cell: (ix, iz),
                    slope,
                    slope_ok: slope >= self.min_slope && slope <= self.max_slope,
                    uphill,
                    score: hash01(self.seed ^ 0xa53f, ix as u64, iz as u64),
                });
            }
        }
        // Deterministic order: the hash decides, the cell breaks ties. Never
        // the iteration order of anything.
        out.sort_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then_with(|| a.cell.cmp(&b.cell))
        });
        out
    }

    /// Height range over the site's own neighbourhood — how much relief there
    /// is to work with.
    fn local_drop(&self, base: &dyn HeightField, at: Vec2) -> f32 {
        let r = self.spacing * 0.5;
        if let Some((lo, hi)) = base.range_over(at.x - r, at.y - r, at.x + r, at.y + r) {
            return hi - lo;
        }
        // No O(1) range: probe the four corners rather than give up.
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for (dx, dz) in [(-r, -r), (r, -r), (-r, r), (r, r), (0.0, 0.0)] {
            let h = base.sample(at.x + dx, at.y + dz);
            lo = lo.min(h);
            hi = hi.max(h);
        }
        hi - lo
    }

    /// Looks for a gap the site could bridge: a direction where the ground
    /// climbs by `min_drop` on BOTH sides. That signature is a ravine or a
    /// river channel, and it is the only place a rock span makes sense.
    fn crossing(&self, base: &dyn HeightField, site: &Site) -> Option<(Vec2, Vec2)> {
        let here = base.sample(site.at.x, site.at.y);
        let reach = self.spacing * 0.6;
        let mut best: Option<(f32, Vec2, Vec2)> = None;
        for k in 0..CROSSING_DIRECTIONS {
            let theta = std::f32::consts::PI * k as f32 / CROSSING_DIRECTIONS as f32;
            let d = Vec2::new(theta.cos(), theta.sin());
            let a = site.at - d * reach;
            let b = site.at + d * reach;
            let (ha, hb) = (base.sample(a.x, a.y), base.sample(b.x, b.y));
            let rise = (ha - here).min(hb - here);
            if rise < self.min_drop {
                continue;
            }
            if best.is_none_or(|(r, _, _)| rise > r) {
                best = Some((rise, a, b));
            }
        }
        best.map(|(_, a, b)| (a, b))
    }

    fn label(&self, kind: &str, i: usize) -> Option<String> {
        Some(format!(
            "{}:{kind}:{i}",
            self.name.as_deref().unwrap_or("rocks")
        ))
    }

    /// A rock span across the gap the site found.
    fn bridge_at(&self, site: &Site, a: Vec2, b: Vec2, i: usize) -> BridgeSpec {
        let (h0, h1) = self.pair(site, 0x11);
        let width = 5.0 + 6.0 * h0;
        BridgeSpec {
            name: self.label("bridge", i),
            path: vec![a, site.at, b],
            width,
            // Enough camber to clear the channel, scaled to the relief the
            // site actually has.
            rise: (self.min_drop * (0.6 + 0.5 * h1)).max(1.5),
            thickness: (width * 0.45).max(2.0),
            style: BridgeStyle::Natural,
            parapet: 0.0,
            ..BridgeSpec::default()
        }
    }

    /// A natural arch standing across the contour, so it reads as a fin the
    /// weather punched through rather than a gate someone built.
    fn arch_at(&self, site: &Site, i: usize) -> ArchSpec {
        let (h0, h1) = self.pair(site, 0x23);
        // Along the contour: perpendicular to the climb.
        let along = Vec2::new(-site.uphill.y, site.uphill.x);
        let half = (self.spacing * (0.18 + 0.14 * h0)).max(4.0);
        ArchSpec {
            name: self.label("arch", i),
            path: vec![site.at - along * half, site.at + along * half],
            profile: ArchProfile::Natural,
            height: (self.min_drop * (0.8 + 0.7 * h1)).max(4.0),
            thickness: (half * 0.28).max(1.2),
            ..ArchSpec::default()
        }
    }

    /// A tunnel burrowing into the hillside, mouth on the slope the site sits
    /// on and the far end deeper into the hill.
    fn cave_at(&self, site: &Site, i: usize) -> CaveSpec {
        let (h0, h1) = self.pair(site, 0x37);
        let run = self.spacing * (1.0 + 0.8 * h0);
        // Bend it: a dead-straight tunnel reads as a pipe.
        let side = Vec2::new(-site.uphill.y, site.uphill.x) * (run * 0.18 * (h1 - 0.5));
        let mid = site.at + site.uphill * (run * 0.5) + side;
        let end = site.at + site.uphill * run;
        let r = 2.2 + 1.4 * h1;
        CaveSpec {
            name: self.label("cave", i),
            path: vec![site.at, mid, end],
            radius: vec![r, r * 1.6, r * 0.9],
            depth: (r * 1.6 + 3.0 + self.min_drop * 0.4).max(6.0),
            // Steeper ground gives a taller mouth without breaching the roof.
            mouth_flare: 1.0 + 0.5 * (site.slope / 90.0),
            ..CaveSpec::default()
        }
    }

    /// Two more decorrelated hashes for one site.
    fn pair(&self, site: &Site, salt: u64) -> (f32, f32) {
        let (ix, iz) = (site.cell.0 as u64, site.cell.1 as u64);
        (
            hash01(self.seed ^ salt, ix, iz),
            hash01(self.seed ^ (salt << 8), iz, ix),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    /// A ridge running along Z: flat top at x = 0, falling away to both sides,
    /// so every site has slope and relief and the middle is a crossable gap.
    #[derive(Debug)]
    struct Ridge;

    impl HeightField for Ridge {
        fn sample(&self, x: f32, _z: f32) -> f32 {
            // A narrow valley at x = 0 between two 30 m shoulders.
            let t = (x.abs() / 25.0).clamp(0.0, 1.0);
            30.0 * t * t
        }
        fn sample_normal(&self, x: f32, _z: f32, e: f32) -> Vec3 {
            let dx = (self.sample(x + e, 0.0) - self.sample(x - e, 0.0)) / (2.0 * e);
            Vec3::new(-dx, 1.0, 0.0).normalize()
        }
        fn max_height(&self) -> f32 {
            30.0
        }
    }

    fn field(arches: u32, caves: u32, bridges: u32) -> RockFeaturesSpec {
        RockFeaturesSpec {
            name: Some("rochedos".into()),
            min: Vec2::new(-120.0, -120.0),
            max: Vec2::new(120.0, 120.0),
            seed: 7,
            arches,
            caves,
            bridges,
            min_slope: 5.0,
            max_slope: 80.0,
            min_drop: 3.0,
            spacing: 30.0,
            clear_of_roads: 0.0,
        }
    }

    #[test]
    fn test_seeding_is_a_pure_function_of_the_seed() {
        let spec = field(3, 2, 1);
        let guards = ScatterGuards::default();
        assert_eq!(spec.resolve(&Ridge, &guards), spec.resolve(&Ridge, &guards));
        let other = RockFeaturesSpec {
            seed: 8,
            ..spec.clone()
        };
        assert_ne!(spec.resolve(&Ridge, &guards), other.resolve(&Ridge, &guards));
    }

    #[test]
    fn test_it_never_exceeds_the_requested_budget() {
        let spec = field(3, 2, 1);
        let got = spec.resolve(&Ridge, &ScatterGuards::default());
        assert!(got.arches.len() <= 3);
        assert!(got.caves.len() <= 2);
        assert!(got.bridges.len() <= 1);
        assert!(!got.is_empty(), "a ridge with relief must seed something");
        assert_eq!(got.len(), got.arches.len() + got.caves.len() + got.bridges.len());
    }

    #[test]
    fn test_asking_for_nothing_builds_nothing() {
        assert!(field(0, 0, 0).resolve(&Ridge, &ScatterGuards::default()).is_empty());
        let bad = RockFeaturesSpec {
            spacing: 0.0,
            ..field(5, 5, 5)
        };
        assert!(bad.resolve(&Ridge, &ScatterGuards::default()).is_empty());
    }

    #[test]
    fn test_flat_ground_offers_no_sites() {
        #[derive(Debug)]
        struct Flat;
        impl HeightField for Flat {
            fn sample(&self, _x: f32, _z: f32) -> f32 {
                12.0
            }
            fn sample_normal(&self, _x: f32, _z: f32, _e: f32) -> Vec3 {
                Vec3::Y
            }
            fn max_height(&self) -> f32 {
                12.0
            }
        }
        // No slope, no relief: the slope band and the drop test both reject
        // every cell, and the field seeds nothing rather than littering a
        // plain with arches.
        assert!(field(4, 4, 4).resolve(&Flat, &ScatterGuards::default()).is_empty());
    }

    #[test]
    fn test_seeded_features_are_spread_out_not_stacked() {
        let spec = field(3, 3, 0);
        let got = spec.resolve(&Ridge, &ScatterGuards::default());
        let mut anchors: Vec<Vec2> = got.arches.iter().map(|a| a.path[0]).collect();
        anchors.extend(got.caves.iter().map(|c| c.path[0]));
        for (i, a) in anchors.iter().enumerate() {
            for b in &anchors[i + 1..] {
                assert!(
                    a.distance(*b) > 1.0,
                    "two features landed on top of each other at {a:?} / {b:?}"
                );
            }
        }
    }

    #[test]
    fn test_a_seeded_bridge_actually_spans_a_gap() {
        let spec = field(0, 0, 1);
        let got = spec.resolve(&Ridge, &ScatterGuards::default());
        let bridge = got.bridges.first().expect("the valley is crossable");
        assert_eq!(bridge.style, BridgeStyle::Natural);
        let (a, b) = (bridge.path[0], bridge.path[bridge.path.len() - 1]);
        let mid = bridge.path[1];
        let here = Ridge.sample(mid.x, mid.y);
        assert!(
            Ridge.sample(a.x, a.y) >= here + spec.min_drop
                && Ridge.sample(b.x, b.y) >= here + spec.min_drop,
            "both abutments must be above the gap they cross"
        );
    }
}
