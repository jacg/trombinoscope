// TODO rotation
// TODO precise controls
// TODO asynchronous writing
// TODO add class name to header
// TODO display help
// TODO option to rename files from metadata names ?

use std::{fs::File, path::{Path, PathBuf}, sync::mpsc};

use eframe::{egui, CreationContext};
use egui::{Color32, ColorImage, Context, Key, PointerButton, Sense, TextureHandle, TextureOptions, Ui, Vec2};
use image::{codecs::jpeg::JpegEncoder, DynamicImage};

use face::{FaceInImage, ASPECT_RATIO};
use render::trombinoscope;
use util::{ensure_empty_dir, find_jpgs_in_dir, Dirs};

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = cli::parse();
    let dirs = Dirs::new(cli.class_dir);

    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true),
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
        let (tx, rx) = mpsc::channel::<(PathBuf, mpsc::Sender<face::Result<Data>>)>();
        std::thread::spawn(move || {
            for (path, tx) in rx.iter() {
                tx.send(load_face_data(path));
            }
        });

        let faces = find_jpgs_in_dir(&dirs.photo)
            .into_iter()
            .map(move |path| { Face::load(path, cc, tx.clone()).unwrap() })
            .collect();
        Self {
            dirs,
            faces,
            face_n: 0,
        }
    }

    pub fn show(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.heading("Trombinoscope");
        let mut sort = false;
        let n_rows = self.faces.len() / 6 + 1;
        egui::Grid::new("face grid").show(ui, |ui| {
            for (n, face) in self.faces.iter_mut().enumerate() {
                if face.show(ui, ctx, n_rows) {
                    sort = true;
                }
                if n % 6 == 5 { ui.end_row() }
            }
        });
        if sort {self. sort();}
    }

    fn handle_keys(&mut self, ctx: &Context) {
        use crate::egui::Modifiers;
        ctx.input(|i| {
            macro_rules! key {
                ($key:ident ($($mod:ident)*) $body:tt) => {
                    if i.key_pressed(Key::$key) && i.modifiers.matches_exact($(Modifiers::$mod)|*) $body
                };
            }
            key!{S (CTRL)  { self.save_and_regenerate(); }}
            key!{Q (CTRL)  { std::process::exit(0) }} // TODO exit less brutally
        });
    }

    fn sort(&mut self) {
        // Identify which face was selected before sorting, by its path
        let selected_path = self.faces[self.face_n].path.clone();
        self.faces.sort_by_cached_key(|f| match &f.data {
            Data::Loading(_) => ("zzzzzz".into(), "zzzz".into()),
            Data::Ready { face, .. } => (face.family.to_uppercase(), face.given.to_uppercase()),
        });
        // Re-focus on the face selected before solting
        for (n, face) in self.faces.iter().enumerate() {
            if face.path == *selected_path { self.face_n = n; }
        }
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

#[derive(Debug)]
enum Data {
    Loading(mpsc::Receiver<face::Result<Data>>),
    Ready {
        face: FaceInImage,
        image: DynamicImage,
    },
}

pub struct Face {
    pub path: PathBuf,
    data: Data,
    texture: TextureHandle,
}

fn load_face_data(path: PathBuf) -> face::Result<Data> {
    let image = image::open(&path)?;
    let face = FaceInImage::from_path_or_default_for(&path, image.width() as f32, image.height() as f32)?;
    Ok(Data::Ready { face, image })
}

impl Face {

    fn load(
        path: impl AsRef<Path>,
        cc: &CreationContext,
        tx_req: mpsc::Sender<(PathBuf, mpsc::Sender<face::Result<Data>>)>,
    ) -> face::Result<Face> {
        let (tx, rx) = mpsc::channel();
        tx_req.send((path.as_ref().into(), tx));
        let texture_name = path.as_ref().to_string_lossy().to_string();
        let dummy_image = ColorImage::new([400,500], Color32::GRAY);
        let texture = cc.egui_ctx.load_texture(&texture_name, dummy_image, egui::TextureOptions::default());
        Ok(Face {
            path: path.as_ref().into(),
            data: Data::Loading(rx),
            texture,
        })
    }

    fn install_data(&mut self, data: Data) {
        if let Data::Ready { ref face, ref image } = data {
            let cropped_image = crop(image, face);
            let image_data = adapt_for_texture(&cropped_image);
            self.texture.set(image_data, egui::TextureOptions::default());
            self.data = data;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &Context, n_rows: usize) -> bool {
        let w = ctx.available_rect().width();
        let h = ctx.available_rect().height();
        let top_margin = 10.0;
        let mut request_sort = false;
        ui.vertical_centered(|ui| {
            ui.set_width((w / 6.6).min((h-top_margin) / (n_rows as f32 * 5.0 / 3.0)));
            ui.vertical_centered(|ui| {
                let w = ui.available_width();
                let response = ui.add(
                    egui::Image::new(&self.texture)
                        .max_size(Vec2 { x: w, y: w * ASPECT_RATIO })
                        .sense(Sense::click_and_drag())
                );
                if response.dragged() {
                    let Vec2 { x, y } = response.drag_motion();
                    if response.dragged_by(PointerButton::Primary)   { self.mv(-x, -y); }
                    if response.dragged_by(PointerButton::Secondary) { self.zoom(y); }
                }
            });
            match &mut self.data {
                Data::Ready { face, .. } => {
                    let a = ui.text_edit_singleline(&mut face.given ).lost_focus();
                    let b = ui.text_edit_singleline(&mut face.family).lost_focus();
                    if a || b { request_sort = true; }
                }
                Data::Loading(rx) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(10));
                    match rx.try_recv() {
                        Ok(Ok(d)) => { self.install_data(d); request_sort = true; }
                        Ok(Err(face::Error::FaceNotLoaded)) => (),
                        _ => (),
                    }
                }
            }
        });
        request_sort
    }

    pub fn rotate(&mut self, d_rot: i8) {
        if let Data::Ready { face, .. } = &mut self.data {
            face.rot = (face.rot + d_rot).rem_euclid(4);
            self.set_texture_from_cropped_image();
        }
    }

    pub fn zoom(&mut self, delta: f32) {
        if let Data::Ready { face, .. } = &mut self.data {
            face.w += delta;
            self.set_texture_from_cropped_image();
        }
    }

    pub fn mv(&mut self, dx: f32, dy: f32) {
        if let Data::Ready { face, .. } = &mut self.data {
            face.cx += dx;
            face.cy += dy;
            self.set_texture_from_cropped_image();
        }
    }

    pub fn set_texture_from_cropped_image(&mut self) {
        if let Data::Ready { face, image } = &mut self.data {
            let cropped_image = crop(image, face);
            let data = adapt_for_texture(&cropped_image);
            self.texture.set(data, TextureOptions::default());
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        match &self.data {
            Data::Ready { face, image } => crop(image, face).as_bytes().to_owned(),
            Data::Loading(_) => vec![],
        }
    }

    pub fn save_metadata(&self) -> face::Result<()> {
        match &self.data {
            Data::Ready { face, .. } => Ok(face.embed_in_jpeg(&self.path)?),
            Data::Loading(_) => Err(face::Error::FaceNotLoaded)
        }

    }
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
    let face = if let Data::Ready { face, .. } = &f.data {face} else { return Err(face::Error::FaceNotLoaded)};
    let filename = format!("{} @ {}.jpg", &face.given, &face.family);
    let path = dir.as_ref().join(&*filename);
    let file = &mut File::create(path)?;
    let mut encoder = JpegEncoder::new(file);
    encoder.encode(
        &f.as_bytes(),
        face.w as u32,
        face.h() as u32,
        image::ExtendedColorType::Rgb8
    ).unwrap();
    Ok(())
}
