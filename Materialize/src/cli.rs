use clap::{Parser, Subcommand};

use crate::preset::Preset;

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Png,
    Jpg,
    Tga,
    Exr,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "png" => Ok(OutputFormat::Png),
            "jpg" | "jpeg" => Ok(OutputFormat::Jpg),
            "tga" => Ok(OutputFormat::Tga),
            "exr" => Ok(OutputFormat::Exr),
            _ => Err(format!("Unsupported format: {}", s)),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Png => write!(f, "png"),
            OutputFormat::Jpg => write!(f, "jpg"),
            OutputFormat::Tga => write!(f, "tga"),
            OutputFormat::Exr => write!(f, "exr"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "materialize")]
#[command(
    about = "Generate PBR maps (height, normal, metallic, smoothness, edge, AO) from diffuse textures"
)]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(
    after_help = "EXAMPLES:\n  materialize texture.png -o ./out/\n  materialize diffuse.png --format png -v\n  materialize skin.png --preset skin -o ./out/\n  materialize floor.png -p floor -v\n  materialize skill install\n\nPRESETS:\n  default  General-purpose (current behavior)\n  skin     Human/character skin — no metallic, smooth, subtle normals\n  floor    Ground surfaces — deep height, strong AO, rough\n  metal    Metallic surfaces — boosted metallic, sharp edges, polished\n  fabric   Cloth/textile — matte, no metallic, soft edges\n  wood     Wood grain — moderate detail, no metallic\n  stone    Rock/stone — very rough, deep AO, strong normals"
)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<CliSubcommand>,

    /// Input image path (required when not using a subcommand)
    pub input: Option<String>,

    #[arg(short, long, help = "Output directory", default_value = ".")]
    pub output: String,

    #[arg(
        short,
        long,
        help = "Output format (png, jpg, tga, exr)",
        default_value = "png"
    )]
    pub format: OutputFormat,

    #[arg(
        short = 'p',
        long,
        help = "Material preset (default, skin, floor, metal, fabric, wood, stone)",
        default_value = "default"
    )]
    pub preset: Preset,

    #[arg(short, long, help = "JPEG quality 0-100 (ignored for other formats)", default_value = "95", value_parser = clap::value_parser!(u8).range(0..=100))]
    pub quality: u8,

    #[arg(short, long, help = "Verbose output")]
    pub verbose: bool,

    #[arg(long, help = "Suppress 'Generated' file list on success")]
    pub quiet: bool,
}

#[derive(Subcommand, Debug)]
pub enum CliSubcommand {
    /// Manage the materialize-cli Cursor skill
    Skill(SkillCommand),
}

#[derive(Parser, Debug)]
pub struct SkillCommand {
    #[command(subcommand)]
    pub subcommand: SkillSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum SkillSubcommand {
    /// Install the materialize-cli skill into this project's .cursor/skills
    Install,
}
