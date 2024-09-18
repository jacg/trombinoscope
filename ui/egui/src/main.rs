#![expect(unused, reason = "egui implementation in progress")]

use std::path::{Path, PathBuf};

use eframe::{egui, CreationContext};

use egui::{ColorImage, Image};

use util::{find_jpgs_in_dir, Dirs};

mod face;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    let cli = cli::parse();
    let dirs = Dirs::new(cli.class_dir);


    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Trombinoscope",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::<App>::new(App::new(dirs, cc)))
        }),
    );

    Ok(())
}

struct App {
    dirs: Dirs,
    faces: Vec<face::CropEgui>,
}

fn load_image_from_path(path: &std::path::Path) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::ImageReader::open(path)?.decode()?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}

impl App {
    fn new(dirs: Dirs, cc: &CreationContext) -> Self {
        let faces = find_jpgs_in_dir(&dirs.photo)
            .into_iter()
            .filter_map(|path| {
                let uri = path.to_string_lossy().to_string();
                util::filename_to_given_family(&path)
                    .map(move |(given, family)| face::CropEgui {
                        given, family,
                        image: cc.egui_ctx.load_texture(
                            uri,
                            load_image_from_path(&path).unwrap(),
                            egui::TextureOptions::default(),
                        ),
                    })
            })
            .collect();
        Self {
            dirs,
            faces,
        }
    }

    pub fn show(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.heading("Trombinoscope");
        egui::Grid::new("face grid").show(ui, |ui| {
            for (n, face) in self.faces.iter_mut().enumerate() {
                face.show(ui, ctx);
                if n % 6 == 5 { ui.end_row() }
            }
        });
    }

    fn handle_keys(&mut self, ctx: &Context) {
        ctx.input(|i| {
            if i.key_pressed(Key::Q) && i.modifiers.ctrl {
                // TODO exit less brutally
                std::process::exit(0);
            }
        });
    }

}


use egui::{Context, Grid, Key, Response, Sense, Ui};

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_keys(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show(ui, ctx);
        });
    }
}
