mod cli;
mod gpu;
mod io;
mod pipeline;
mod skill_install;

use clap::Parser;
use cli::{CliSubcommand, SkillSubcommand};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let args = cli::Cli::parse();

    if let Some(CliSubcommand::Skill(skill)) = args.subcommand {
        if matches!(skill.subcommand, SkillSubcommand::Install) {
            return skill_install::run();
        }
    }

    let input = args.input.ok_or_else(|| {
        anyhow::anyhow!("Missing required argument: <INPUT>. Use 'materialize --help' for usage.")
    })?;
    let image = io::load_image(&input)?;
    let (width, height) = (image.width(), image.height());

    if args.verbose {
        println!("Loaded: {} ({}x{})", input, width, height);
    }

    let pipeline = pipeline::Pipeline::new().await?;

    if args.verbose {
        println!("Processing...");
    }

    let maps = pipeline.process(&image).await?;

    if args.verbose {
        println!("Processing complete");
    }

    let height_img = io::height_to_image(width, height, &maps.height);
    let normal_img = io::normal_to_image(width, height, &maps.normal);
    let metallic_img = io::metallic_to_image(width, height, &maps.metallic);
    let smoothness_img = io::smoothness_to_image(width, height, &maps.smoothness);
    let edge_img = io::edge_to_image(width, height, &maps.edge);
    let ao_img = io::ao_to_image(width, height, &maps.ao);

    let format_str = format!("{}", args.format);
    let paths = io::get_output_paths(&input, &args.output, &format_str);
    let image_format = io::output_format_to_image_format(&args.format);

    io::save_image(&height_img, &paths.height_path, image_format, args.quality)?;
    io::save_image(&normal_img, &paths.normal_path, image_format, args.quality)?;
    io::save_image(&metallic_img, &paths.metallic_path, image_format, args.quality)?;
    io::save_image(&smoothness_img, &paths.smoothness_path, image_format, args.quality)?;
    io::save_image(&edge_img, &paths.edge_path, image_format, args.quality)?;
    io::save_image(&ao_img, &paths.ao_path, image_format, args.quality)?;

    if !args.quiet {
        println!("Generated:");
        println!("  - {}", paths.height_path);
        println!("  - {}", paths.normal_path);
        println!("  - {}", paths.metallic_path);
        println!("  - {}", paths.smoothness_path);
        println!("  - {}", paths.edge_path);
        println!("  - {}", paths.ao_path);
    }

    Ok(())
}
