// Embeds the Cursor skill and writes it to the current project's .cursor/skills.

use anyhow::{Context, Result};

const SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/.cursor/skills/materialize-cli/SKILL.md"
));

/// Install the materialize-cli skill into the current working directory's
/// `.cursor/skills/materialize-cli/`.
pub fn run() -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let skill_dir = cwd.join(".cursor").join("skills").join("materialize-cli");
    let skill_file = skill_dir.join("SKILL.md");

    std::fs::create_dir_all(&skill_dir)
        .with_context(|| format!("Failed to create directory: {}", skill_dir.display()))?;

    std::fs::write(&skill_file, SKILL_MD)
        .with_context(|| format!("Failed to write: {}", skill_file.display()))?;

    println!("Installed materialize-cli skill to {}", skill_dir.display());
    println!("  {}", skill_file.display());

    Ok(())
}
