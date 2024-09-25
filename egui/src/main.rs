// TODO fix highlighting of current face
// TODO stop typing names being picked up as crop commands
// TODO asynchronous I/O
// TODO fine face controls

use std::{fs::File, path::{Path, PathBuf}};

use eframe::{egui, CreationContext};
use egui::{Context, Key, TextureHandle, TextureOptions, Ui, Vec2};
use image::{codecs::jpeg::JpegEncoder, DynamicImage};

use face::{FaceInImage, ASPECT_RATIO};
use render::trombinoscope;
use util::{ensure_empty_dir, find_jpgs_in_dir, move_index_by, Dirs};

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
    )?;

    Ok(())
}

struct App {
    dirs: Dirs,
    faces: Vec<Face>,
    face_n: usize,
}

impl App {
    fn new(dirs: Dirs, cc: &CreationContext) -> Self {
        let faces = find_jpgs_in_dir(&dirs.photo)
            .into_iter()
            .map(|path| Face::load(path, cc).unwrap())
            .collect();
        let mut it = Self {
            dirs,
            faces,
            face_n: 0,
        };
        it.sort();
        it
    }

    pub fn show(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.heading("Trombinoscope");
        egui::Grid::new("face grid").show(ui, |ui| {
            for (n, face) in self.faces.iter_mut().enumerate() {
                face.show(ui, ctx, n == self.face_n);
                if n % 6 == 5 { ui.end_row() }
            }
        });
    }

    fn handle_keys(&mut self, ctx: &Context) {
        use crate::egui::Modifiers;
        ctx.input(|i| {
            macro_rules! key {
                ($key:ident ($($mod:ident)*) $body:tt) => {
                    if i.key_pressed(Key::$key) && i.modifiers.matches_exact($(Modifiers::$mod)|*) $body
                };
            }
            key!{Q          (CTRL)  { std::process::exit(0) }} // TODO exit less brutally
            key!{R          (NONE)  { self.face_rotate( 1 ); }}
            key!{L          (NONE)  { self.face_rotate(-1 ); }}
            key!{G          (NONE)  { self.face_zoom(-30.0); }}
            key!{P          (NONE)  { self.face_zoom( 30.0); }}
            key!{G          (CTRL)  { self.face_zoom(- 3.0); }}
            key!{P          (CTRL)  { self.face_zoom(  3.0); }}
            key!{ArrowRight (NONE)  { self.face_mv_x(-30.0); }}
            key!{ArrowLeft  (NONE)  { self.face_mv_x( 30.0); }}
            key!{ArrowDown  (NONE)  { self.face_mv_y(-30.0); }}
            key!{ArrowUp    (NONE)  { self.face_mv_y( 30.0); }}
            key!{Space      (NONE)  { self.face_select( 1); }}
            key!{Backspace  (NONE)  { self.face_select(-1); }}
            key!{Space      (SHIFT) { self.face_select( 6); }}
            key!{Backspace  (SHIFT) { self.face_select(-6); }}
            key!{S          (CTRL)  { self.sort(); self.save_and_regenerate(); }}
            key!{O          (CTRL)  { self.sort(); }}
        });
    }

    fn face_rotate(&mut self, d_rot: i8) {
        self.faces.get_mut(self.face_n).unwrap().rotate(d_rot);
    }

    fn face_select(&mut self, delta: isize) {
        self.face_n = move_index_by(self.face_n, delta, self.faces.len());
    }

    fn face_zoom(&mut self, delta: f32) {
        let face = &mut self.faces[self.face_n];
        face.face.w += delta;
        face.set_texture_from_cropped_image();
    }

    fn face_mv_x(&mut self, delta: f32) {
        let face = &mut self.faces[self.face_n];
        face.face.cx += delta;
        face.set_texture_from_cropped_image();
    }

    fn face_mv_y(&mut self, delta: f32) {
        let face = &mut self.faces[self.face_n];
        face.face.cy += delta;
        face.set_texture_from_cropped_image();
    }

    fn sort(&mut self) {
        self.faces.sort_by_cached_key(|f| (f.face.family.to_uppercase(), f.face.given.to_uppercase()));
    }

    fn save_and_regenerate(&self) -> face::Result<()> {
        save_many_face_metadata(&self.faces)?;
        ensure_empty_dir(&self.dirs.work)?;
        ensure_empty_dir(&self.dirs.render)?;
        write_many_face_images(&self.faces, &self.dirs.work)?;
        trombinoscope(&self.dirs);
        Ok(())
    }

}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keys(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show(ui, ctx);
        });
    }
}

pub struct Face {
    pub path: PathBuf,
    pub face: FaceInImage,
    pub image: DynamicImage,
    pub texture: TextureHandle,
}

impl Face {

    fn load(path: impl AsRef<Path>, cc: &CreationContext) -> face::Result<Face> {
        let image = image::open(&path)?;
        let face = FaceInImage::from_path_or_default_for(&path, image.width() as f32, image.height() as f32)?;
        let texture_name = path.as_ref().to_string_lossy().to_string();
        let cropped_image = crop(&image, &face);
        let data = adapt_for_texture(&cropped_image);
        let texture = cc.egui_ctx.load_texture(&texture_name, data, egui::TextureOptions::default());
        Ok(Face { face, image, texture, path: path.as_ref().into() })
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &Context, selected: bool) {
        let w = ctx.available_rect().width();
        egui::Frame::none()
            .fill(if selected {egui::Color32::RED} else { egui::Color32::BLACK })
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_width(w / 6.5);
                    ui.vertical(|ui| {
                        let w = ui.available_width();
                        ui.add(egui::Image::new(&self.texture)
                               .max_size(Vec2 { x: w, y: w * ASPECT_RATIO }));
                    });
                    ui.horizontal(|ui| {
                        ui.label("prénom : ");
                        ui.text_edit_singleline(&mut self.face.given);
                    });
                    ui.horizontal(|ui| {
                        ui.label("nom : ");
                        ui.text_edit_singleline(&mut self.face.family);
                    });
                });
            });
    }

    pub fn rotate(&mut self, d_rot: i8) {
        self.face.rot = (self.face.rot + d_rot).rem_euclid(4);
        self.set_texture_from_cropped_image();
    }

    pub fn set_texture_from_cropped_image(&mut self) {
        let cropped_image = crop(&self.image, &self.face);
        let data = adapt_for_texture(&cropped_image);
        self.texture.set(data, TextureOptions::default());
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        crop(&self.image, &self.face).as_bytes().to_owned()
    }

    pub fn save_metadata(&self) -> face::Result<()> { self.face.embed_in_jpeg(&self.path) }
}

pub fn crop(image: &DynamicImage, &FaceInImage { cx, cy, w, rot, .. }: &FaceInImage) -> DynamicImage {
    let h = w * ASPECT_RATIO;
    match rot.rem_euclid(4) {
        0 => image.clone(),
        1 => image.rotate90(),
        2 => image.rotate180(),
        3 => image.rotate270(),
        _ => unreachable!(),
    }.crop_imm(
        (cx - w / 2.0) as _,
        (cy - h / 2.0) as _,
        w              as _,
        h              as _,
    )
}

/// Convert `DynamicImage` to format needed by  `egui::TextureHandle`
pub fn adapt_for_texture(cropped: &DynamicImage) -> egui::ColorImage {
    let size = [cropped.width() as _, cropped.height() as _];
    let image_buffer = cropped.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())
}

/// Store the location and name of each face in the JPEG segment of the image
/// containing the face
fn save_many_face_metadata(faces: &[Face]) -> face::Result<()> {
    for face in faces { face.save_metadata()?; }
    Ok(())
}

/// Save each cropped face in its own image file in `dir`. Assumes `dir` exists.
fn write_many_face_images(faces: &[Face], dir: impl AsRef<Path>) -> face::Result<()> {
    for face in faces { write_one_face_image(face, &dir)?; }
    Ok(())
}

/// Save one cropped face in its own image file in `dir`. Assumes `dir` exists.
fn write_one_face_image(f: &Face, dir: impl AsRef<Path>) -> face::Result<()> {
    let filename = format!("{} @ {}.jpg", &f.face.given, &f.face.family);
    let path = dir.as_ref().join(&*filename);
    let file = &mut File::create(path)?;
    let mut encoder = JpegEncoder::new(file);
    encoder.encode(
        &f.as_bytes(),
        f.face.w as u32,
        f.face.h() as u32,
        image::ExtendedColorType::Rgb8
    ).unwrap();
    Ok(())
}
