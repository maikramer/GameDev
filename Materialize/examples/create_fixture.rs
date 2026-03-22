// Creates a small test image for running materialize. Run from repo root:
//   cargo run --example create_fixture
// Then: cargo run -- tests/fixtures/diffuse.png -o tests/out -v

use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new("tests/fixtures/diffuse.png");
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut img = image::ImageBuffer::new(128, 128);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let r = ((x as f32 / 127.0) * 255.0) as u8;
        let g = ((y as f32 / 127.0) * 255.0) as u8;
        let b = 128u8;
        *p = image::Rgba([r, g, b, 255]);
    }
    img.save(path)?;
    println!("Created {}", path.display());
    Ok(())
}
