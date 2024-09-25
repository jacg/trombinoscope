use std::{fs::File, path::Path};

use eframe::{egui, CreationContext};

use ::face::{FaceInImage, ASPECT_RATIO};
use image::{codecs::jpeg::JpegEncoder, DynamicImage};
use render::trombinoscope;
use util::{ensure_empty_dir, find_jpgs_in_dir, Dirs};

mod face;
use face::CropEgui;

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
    let cropped_image = crop(&image, &face);
    let data = crop_image_for_texture(&cropped_image);
    let texture = cc.egui_ctx.load_texture(&texture_name, data, egui::TextureOptions::default());
    Ok(CropEgui { face, image, texture, path: path.as_ref().into() })
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

// TODO dynamic polymorphism for faces
/// Save each cropped face in its own image file in `dir`. Assumes `dir` exists.
pub fn write_many_face_images(faces: &[CropEgui], dir: impl AsRef<Path>) -> ::face::Result<()> {
    for face in faces { write_one_face_image(face, &dir)?; }
    Ok(())
}

// TODO replace this with dynamic polymorphism
/// Store the location and name of each face in the JPEG segment of the image
/// containing the face
pub fn save_many_face_metadata(faces: &[CropEgui]) -> ::face::Result<()> {
    for face in faces { face.save_metadata_xxx()?; }
    Ok(())
}

pub fn save_and_regenerate(faces: &[CropEgui], dirs: &Dirs) -> ::face::Result<()> {
    save_many_face_metadata(faces)?;
    ensure_empty_dir(&dirs.work)?;
    ensure_empty_dir(&dirs.render)?;
    write_many_face_images(faces, &dirs.work)?;
    trombinoscope(dirs);
    Ok(())
}

/// Save one cropped face in its own image file in `dir`. Assumes `dir` exists.
fn write_one_face_image(it: &CropEgui, dir: impl AsRef<Path>) -> ::face::Result<()> {
    // let filename = format!("{} @ {}.jpg", &face.given, &face.family);
    let filename = it.path.file_name().unwrap().to_string_lossy();
    let path = dir.as_ref().join(&*filename);
    let file = &mut File::create(path)?;
    let mut encoder = JpegEncoder::new(file);
    dbg!((it.face.w, it.face.h()));
    encoder.encode(
        &it.as_bytes(), //&ui.as_bytes(face),
        it.face.w as u32,
        it.face.h() as u32,
        image::ExtendedColorType::Rgb8
    ).unwrap();
    Ok(())
}

pub fn crop_image_for_texture(cropped: &DynamicImage) -> egui::ColorImage {
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
            key!{R          (NONE)  { self.face_rotate( 1, ctx ); }}
            key!{L          (NONE)  { self.face_rotate(-1, ctx ); }}
            key!{G          (NONE)  { self.face_zoom(-30.0); }}
            key!{P          (NONE)  { self.face_zoom( 30.0); }}
            key!{G          (CTRL)  { self.face_zoom(- 3.0); }}
            key!{P          (CTRL)  { self.face_zoom(  3.0); }}
            key!{ArrowRight (NONE)  { self.face_mv_x(-30.0); }}
            key!{ArrowLeft  (NONE)  { self.face_mv_x( 30.0); }}
            key!{ArrowDown  (NONE)  { self.face_mv_y(-30.0); }}
            key!{ArrowUp    (NONE)  { self.face_mv_y( 30.0); }}
            key!{Space      (NONE)  { self.face_select(Delta::R(1)); }}
            key!{Backspace  (NONE)  { self.face_select(Delta::L(1)); }}
            key!{Space      (SHIFT) { self.face_select(Delta::R(6)); }}
            key!{Backspace  (SHIFT) { self.face_select(Delta::L(6)); }}
            key!{S          (CTRL)  { self.save_and_regenerate(); }}

        });
    }

    fn face_rotate(&mut self, d_rot: i8, ctx: &Context) {
        self.faces.get_mut(self.face_n).unwrap().rotate(d_rot);
    }

    fn face_select(&mut self, delta: Delta) {
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

    fn save_and_regenerate(&self) -> ::face::Result<()> {
        save_many_face_metadata_xxx(&self.faces)?;
        ensure_empty_dir(&self.dirs.work)?;
        ensure_empty_dir(&self.dirs.render)?;
        write_many_face_images_xxx(&self.faces, &self.dirs.work)?;
        dbg!(&self.dirs.photo);
        trombinoscope(&self.dirs);
        Ok(())
    }

}

// TODO replace this with dynamic polymorphism
/// Store the location and name of each face in the JPEG segment of the image
/// containing the face
fn save_many_face_metadata_xxx(faces: &[face::CropEgui]) -> ::face::Result<()> {
    for face in faces { face.save_metadata_xxx()?; }
    Ok(())
}

// TODO dynamic polymorphism for faces
/// Save each cropped face in its own image file in `dir`. Assumes `dir` exists.
fn write_many_face_images_xxx(faces: &[face::CropEgui], dir: impl AsRef<Path>) -> ::face::Result<()> {
    for face in faces { write_one_face_image_xxx(face, &dir)?; }
    Ok(())
}

/// Save one cropped face in its own image file in `dir`. Assumes `dir` exists.
//fn write_one_face_image_xxx(FaceType { face, ui, path }: &CropEgui, dir: impl AsRef<Path>) -> ::face::::face::Result<()> {
fn write_one_face_image_xxx(f: &CropEgui, dir: impl AsRef<Path>) -> ::face::Result<()> {
    let filename = format!("{} @ {}.jpg", &f.face.given, &f.face.family);
    let path = dir.as_ref().join(&*filename);
    let file = &mut File::create(path)?;
    let mut encoder = JpegEncoder::new(file);
    dbg!((f.face.w, f.face.h()));
    encoder.encode(
        &f.as_bytes(),
        f.face.w as u32,
        f.face.h() as u32,
        image::ExtendedColorType::Rgb8
    ).unwrap();
    Ok(())
}

impl CropEgui {
    pub fn save_metadata_xxx(&self) -> ::face::Result<()> { self.face.embed_in_jpeg(&self.path) }
}



enum Delta {
    R(usize),
    L(usize),
}

fn move_index_by(index: usize, delta: Delta, size: usize) -> usize {
    match delta {
        Delta::R(d) => index.wrapping_add(       d),
        Delta::L(d) => index.wrapping_add(size - d),
    }.rem_euclid(size)

}

use egui::{Context, Key, Ui};

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_keys(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show(ui, ctx);
        });
    }
}
