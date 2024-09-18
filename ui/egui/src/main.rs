#![expect(unused, reason = "egui implementation in progress")]

use std::path::{Path, PathBuf};

use eframe::{egui, CreationContext};

use egui::{ColorImage, Image};

use face::CropEgui;
use ::face::FaceInImage;
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
        Box::new(|cc| Ok(Box::<App>::new(App::new(dirs, cc)))),
    );

    Ok(())
}

struct App {
    dirs: Dirs,
    faces: Vec<face::CropEgui>,
}

fn load_face(path: impl AsRef<Path>, cc: &CreationContext) -> ::face::Result<CropEgui> {
    let image = image::open(&path)?;
    let face = FaceInImage::from_path_or_default_for(&path, image.width() as f32, image.height() as f32)?;
    let path_as_id = path.as_ref().to_string_lossy();
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
    let texture = cc.egui_ctx.load_texture(path_as_id, color_image, egui::TextureOptions::default());
    Ok(CropEgui { face, image, texture })
}

impl App {
    fn new(dirs: Dirs, cc: &CreationContext) -> Self {
        let faces = find_jpgs_in_dir(&dirs.photo)
            .into_iter()
            .map(|path| load_face(path, cc).unwrap())
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
