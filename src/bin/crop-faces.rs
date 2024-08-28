use std::path::PathBuf;
use clap::Parser;
use show_image::create_window;
use trombinoscope::crop::{crop_interactively, write_cropped_images, Cropped};

#[derive(Parser)]
struct Cli {
    /// Directory containing the class assets
    class_dir: PathBuf,
}

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let full_photo_dir = cli.class_dir.join("Complet");
    let render_dir     = cli.class_dir.join("Recadré");

    let mut faces = std::fs::read_dir(full_photo_dir)?
        .take(100)
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .filter_map(Cropped::load)
        .collect::<Vec<_>>();

    let window = create_window("image", Default::default())?;
    crop_interactively(&mut faces, &window).unwrap();

    std::fs::create_dir_all(&render_dir).unwrap(); // Ensure it exists so next line works
    std::fs::remove_dir_all(&render_dir).unwrap(); // Remove it and its contents
    std::fs::create_dir_all(&render_dir).unwrap(); // Ensure it exists

    write_cropped_images(&faces, render_dir);

    Ok(())
}
