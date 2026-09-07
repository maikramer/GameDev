//! SplitMix64 — o único RNG determinístico da engine ("mesma seed, mesmo
//! mundo"). Um só módulo para spawner, terreno, IA e clima: NUNCA dupliques
//! o finalizer noutro ficheiro (um `0x9E37` trocado quebra mundos seeded).

/// Small deterministic RNG (SplitMix64) — same seed, same sequence.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0.0..1.0` (24 bits of mantissa — plenty for placement).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in `min..max` (inverted args — `scale-min > scale-max` no
    /// XML — produzem o intervalo trocado em vez de valores fora de gama).
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        min + (max - min) * self.next_f32()
    }

    /// Uniform point in the unit disc (sqrt keeps the area density flat).
    pub fn unit_disc(&mut self) -> bevy::math::Vec2 {
        let angle = self.range(0.0, std::f32::consts::TAU);
        let radius = self.next_f32().sqrt();
        bevy::math::Vec2::new(angle.cos() * radius, angle.sin() * radius)
    }
}

/// Splitmix64 finalizer over an explicit state (counter-style) — the "RNG"
/// do heightmap procedural (pure math, fully deterministic across
/// platforms). Equivalente a [`Rng`] com o estado exposto.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden values da sequência SplitMix64 canónica — se isto mudar,
    /// mundos seeded inteiros mudam. Congelado de propósito.
    #[test]
    fn sequence_is_frozen() {
        let mut rng = Rng::new(0);
        assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
    }

    /// A variante counter-style do heightmap tem de coincidir com [`Rng`].
    #[test]
    fn counter_style_matches_rng() {
        let mut rng = Rng::new(42);
        let mut state = 42u64;
        for _ in 0..8 {
            assert_eq!(rng.next_u64(), splitmix64(&mut state));
        }
    }

    #[test]
    fn next_f32_in_unit_range() {
        let mut rng = Rng::new(7);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v));
        }
    }
}
