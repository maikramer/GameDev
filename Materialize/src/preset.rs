use std::fmt;

#[derive(Debug, Clone, Copy)]
pub enum Preset {
    Default,
    Skin,
    Floor,
    Metal,
    Fabric,
    Wood,
    Stone,
}

impl fmt::Display for Preset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Preset::Default => write!(f, "default"),
            Preset::Skin => write!(f, "skin"),
            Preset::Floor => write!(f, "floor"),
            Preset::Metal => write!(f, "metal"),
            Preset::Fabric => write!(f, "fabric"),
            Preset::Wood => write!(f, "wood"),
            Preset::Stone => write!(f, "stone"),
        }
    }
}

impl std::str::FromStr for Preset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Preset::Default),
            "skin" => Ok(Preset::Skin),
            "floor" => Ok(Preset::Floor),
            "metal" => Ok(Preset::Metal),
            "fabric" => Ok(Preset::Fabric),
            "wood" => Ok(Preset::Wood),
            "stone" => Ok(Preset::Stone),
            _ => Err(format!(
                "Unknown preset '{}'. Available: default, skin, floor, metal, fabric, wood, stone",
                s
            )),
        }
    }
}

/// GPU-aligned parameters passed to all shaders via uniform buffer.
/// Layout: 12 x f32 = 48 bytes (multiple of 16, satisfies WGSL uniform alignment).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PresetParams {
    // Height shader
    pub height_blur_radius_0: f32,
    pub height_blur_radius_1: f32,
    pub height_blur_radius_2: f32,
    pub height_contrast: f32,

    // Normal shader
    pub normal_strength: f32,
    // Metallic shader (scale applied to detection result; 0.0 = force non-metallic)
    pub metallic_scale: f32,
    // Smoothness shader
    pub smoothness_base: f32,
    pub smoothness_metallic_boost: f32,

    // Edge shader
    pub edge_contrast: f32,
    // AO shader
    pub ao_depth_scale: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl Preset {
    pub fn params(self) -> PresetParams {
        match self {
            Preset::Default => PresetParams {
                height_blur_radius_0: 1.0,
                height_blur_radius_1: 2.0,
                height_blur_radius_2: 4.0,
                height_contrast: 1.5,
                normal_strength: 2.0,
                metallic_scale: 1.0,
                smoothness_base: 0.25,
                smoothness_metallic_boost: 0.65,
                edge_contrast: 2.0,
                ao_depth_scale: 3.0,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            Preset::Skin => PresetParams {
                height_blur_radius_0: 2.0,
                height_blur_radius_1: 4.0,
                height_blur_radius_2: 8.0,
                height_contrast: 0.8,
                normal_strength: 1.0,
                metallic_scale: 0.0,
                smoothness_base: 0.45,
                smoothness_metallic_boost: 0.0,
                edge_contrast: 0.8,
                ao_depth_scale: 1.5,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            Preset::Floor => PresetParams {
                height_blur_radius_0: 1.0,
                height_blur_radius_1: 2.0,
                height_blur_radius_2: 3.0,
                height_contrast: 2.0,
                normal_strength: 2.5,
                metallic_scale: 0.1,
                smoothness_base: 0.15,
                smoothness_metallic_boost: 0.4,
                edge_contrast: 2.5,
                ao_depth_scale: 4.0,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            Preset::Metal => PresetParams {
                height_blur_radius_0: 1.0,
                height_blur_radius_1: 2.0,
                height_blur_radius_2: 4.0,
                height_contrast: 1.2,
                normal_strength: 1.5,
                metallic_scale: 1.5,
                smoothness_base: 0.5,
                smoothness_metallic_boost: 0.45,
                edge_contrast: 3.0,
                ao_depth_scale: 2.5,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            Preset::Fabric => PresetParams {
                height_blur_radius_0: 1.0,
                height_blur_radius_1: 2.0,
                height_blur_radius_2: 4.0,
                height_contrast: 1.0,
                normal_strength: 1.8,
                metallic_scale: 0.0,
                smoothness_base: 0.1,
                smoothness_metallic_boost: 0.0,
                edge_contrast: 1.0,
                ao_depth_scale: 2.0,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            Preset::Wood => PresetParams {
                height_blur_radius_0: 1.0,
                height_blur_radius_1: 3.0,
                height_blur_radius_2: 6.0,
                height_contrast: 1.3,
                normal_strength: 1.8,
                metallic_scale: 0.0,
                smoothness_base: 0.2,
                smoothness_metallic_boost: 0.0,
                edge_contrast: 1.5,
                ao_depth_scale: 2.5,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            Preset::Stone => PresetParams {
                height_blur_radius_0: 1.0,
                height_blur_radius_1: 1.0,
                height_blur_radius_2: 2.0,
                height_contrast: 2.5,
                normal_strength: 3.0,
                metallic_scale: 0.0,
                smoothness_base: 0.08,
                smoothness_metallic_boost: 0.0,
                edge_contrast: 2.0,
                ao_depth_scale: 5.0,
                _pad0: 0.0,
                _pad1: 0.0,
            },
        }
    }

    #[allow(dead_code)]
    pub const ALL: &'static [Preset] = &[
        Preset::Default,
        Preset::Skin,
        Preset::Floor,
        Preset::Metal,
        Preset::Fabric,
        Preset::Wood,
        Preset::Stone,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_params_size() {
        assert_eq!(std::mem::size_of::<PresetParams>(), 48);
    }

    #[test]
    fn test_preset_roundtrip_str() {
        for preset in Preset::ALL {
            let s = preset.to_string();
            let parsed: Preset = s.parse().unwrap();
            assert_eq!(s, parsed.to_string());
        }
    }

    #[test]
    fn test_preset_parse_case_insensitive() {
        let p: Preset = "SKIN".parse().unwrap();
        assert_eq!(p.to_string(), "skin");
    }

    #[test]
    fn test_preset_parse_unknown() {
        let result: Result<Preset, _> = "unknown".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_default_preset_matches_original_hardcoded() {
        let p = Preset::Default.params();
        assert_eq!(p.height_contrast, 1.5);
        assert_eq!(p.normal_strength, 2.0);
        assert_eq!(p.smoothness_base, 0.25);
        assert_eq!(p.smoothness_metallic_boost, 0.65);
        assert_eq!(p.edge_contrast, 2.0);
        assert_eq!(p.ao_depth_scale, 3.0);
    }

    #[test]
    fn test_skin_has_zero_metallic() {
        let p = Preset::Skin.params();
        assert_eq!(p.metallic_scale, 0.0);
    }
}
