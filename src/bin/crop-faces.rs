use std::env::Args;

use show_image::create_window;

use trombinoscope::crop::{Cropped, crop_interactively};

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = match Options::parse(std::env::args()) {
        Ok(options) => options,
        Err(message) => {
            println!("Failed to parse program arguments: {}", message);
            std::process::exit(1)
        }
    };

    let mut faces = std::fs::read_dir(options.image_path())?
        .take(3)
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .filter_map(Cropped::load)
        .collect::<Vec<_>>();

    let window = create_window("image", Default::default())?;
    crop_interactively(&mut faces, &window).unwrap();

    Ok(())
}

struct Options {
    image_path: String,
}

impl Options {
    fn parse(args: Args) -> Result<Self, String> {
        let args: Vec<String> = args.into_iter().collect();
        if args.len() != 2 {
            return Err(format!("Usage: {} <model-path> <image-path>", args[0]));
        }

        let image_path = args[1].clone();

        Ok(Options {
            image_path,
        })
    }

    fn image_path(&self) -> &str {
        &self.image_path[..]
    }
}
