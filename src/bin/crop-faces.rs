use std::path::PathBuf;
use clap::Parser;
use show_image::create_window;
use trombinoscope::crop::{crop_interactively, write_cropped_images, Cropped};

#[derive(Parser)]
struct Cli {
    /// Directory containing the images to be cropped
    image_dir: PathBuf,

    /// Generate cropped images and write to this directory
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut faces = std::fs::read_dir(cli.image_dir)?
        .take(100)
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .filter_map(Cropped::load)
        .collect::<Vec<_>>();

    let window = create_window("image", Default::default())?;
    crop_interactively(&mut faces, &window).unwrap();

    if let Some(dir) = cli.out_dir {
        write_cropped_images(&faces, dir);
    }

    Ok(())
}
