#![expect(unused, reason = "egui implementation in progress")]

use std::path::{Path, PathBuf};

use eframe::{egui, CreationContext};

use egui::{ColorImage, Image, TextureHandle};

use face::CropEgui;
use ::face::{ui::one::Face, FaceInImage, ASPECT_RATIO};
use image::DynamicImage;
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
    face_n: usize,
}

fn load_face(path: impl AsRef<Path>, cc: &CreationContext) -> ::face::Result<CropEgui> {
    let image = image::open(&path)?;
    let face = FaceInImage::from_path_or_default_for(&path, image.width() as f32, image.height() as f32)?;
    let texture_name = path.as_ref().to_string_lossy().to_string();
    let cropped_image = crop_image_for_texture(&image, &face);
    let texture = cc.egui_ctx.load_texture(&texture_name, cropped_image, egui::TextureOptions::default());
    Ok(CropEgui { face, image, texture, texture_name })
}

fn crop_image_for_texture(image: &DynamicImage, &FaceInImage { cx, cy, w, rot, .. }: &FaceInImage) -> ColorImage {
    let h = w * ASPECT_RATIO;
    let cropped = match rot.rem_euclid(4) {
        0 => image.clone(),
        1 => image.rotate90(),
        2 => image.rotate180(),
        3 => image.rotate270(),
        _ => unreachable!(),
    }.crop_imm((cx-w/2.0) as _, (cy-h/2.0) as _, w as _, h as _);
    let size = [cropped.width() as _, cropped.height() as _];
    let image_buffer = cropped.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())
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
            face_n: 0,
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
            macro_rules! key {
                ($key:ident ($($mod:ident)?) $body:tt) => {
                    if i.key_pressed(Key::$key) $(&& i.modifiers.$mod)? $body
                };
            }
            key!{Q (ctrl) { std::process::exit(0) }} // TODO exit less brutally
            key!(R ()     { self.rotate_current_face( 1, ctx ); })
        });
    }

    fn rotate_current_face(&mut self, r: u8, ctx: &Context) {
        dbg!("grrr");
        self.faces.get_mut(self.face_n).unwrap().rotate( 1, ctx );
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
