// TODO centre class name in header
// TODO report locations of generated output PDFs
// TODO quit confirm / save / cancel dialog
// TODO speed slider in help
// TODO multiple faces in one photo
// TODO fix sporadic inability to edit names
// TODO option to rename files from metadata names ?

// TODO Thumbhash or something else from https://lucasmerlin.github.io/hello_egui/

use std::{fs::File, path::{Path, PathBuf}, sync::{mpsc, Arc}, time::{Duration, Instant}, thread};

use eframe::{egui, CreationContext};
use egui::{Color32, ColorImage, Context, Key, PointerButton, Pos2, Rect, Response, RichText, Sense, TextureHandle, TextureOptions, Ui, Vec2};
use image::{codecs::jpeg::JpegEncoder, DynamicImage};
use rayon::prelude::*;

use face::{FaceInImage, ASPECT_RATIO};
use render::{trombinoscope, trombi_file_for_dir};
use util::{
    ensure_empty_dir, find_jpgs_in_dir, Dirs,
    MAITRES_DE_CLASSE_FILENAME, MAITRES_DE_CLASSE_DEFAULT_CONTENT,
    CONFIG_FILENAME, DEFAULT_JPEG_QUALITY, DEFAULT_IMAGE_WIDTH,
    FileType,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = cli::parse();
    let dirs = Dirs::new(cli.class_dir, cli.original_photos_subdir);

    if cli.strip_metadata { face::metadata::strip_from_jpgs_in_dir(&dirs.photo).unwrap(); }

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

#[derive(Debug, Clone)]
enum PdfStatus {
    Missing,
    GeneratingLatest           { started: Instant },
    GeneratingPrevious { started: Instant },
    Ready {
        size_bytes: u64,
        generation_time: Duration
    },
}

#[derive(Debug)]
struct SaveRequest(SaveData);

#[derive(Debug)]
enum SaveResponse {
    Completed { generation_time: Duration },
    Error(String),
}

#[derive(Debug, Clone)]
struct SaveData {
    face_data: Vec<FaceData>,
    maitres_text: String,
    jpeg_quality: u8,
    image_width: u32,
}

#[derive(Debug, Clone)]
struct FaceData {
    path: PathBuf,
    face: FaceInImage,
    image: Arc<DynamicImage>,
}

struct App {
    dirs: Dirs,
    faces: Vec<Face>,
    maitres_text: String,
    jpeg_quality: u8,
    image_width: u32,

    // PDF status tracking
    pdf_status: PdfStatus,
    save_request_tx: mpsc::Sender<SaveRequest>,
    save_response_rx: mpsc::Receiver<SaveResponse>,
}

impl App {

    fn new(dirs: Dirs, cc: &CreationContext) -> Self {
        let (tx, rx) = mpsc::channel::<(PathBuf, mpsc::Sender<face::Result<Data>>)>();
        std::thread::spawn(move || {
            for (path, tx) in rx.iter() {
                tx.send(load_face_data(path)).unwrap();
            }
        });

        let faces = find_jpgs_in_dir(&dirs.photo)
            .into_iter()
            .map(move |path| { Face::load(path, cc, tx.clone()).unwrap() })
            .collect();

        let maitres_file_path = dirs.class.join(MAITRES_DE_CLASSE_FILENAME);
        let maitres_text = std::fs::read_to_string(&maitres_file_path)
            .unwrap_or_else(|_| {
                let default_content = MAITRES_DE_CLASSE_DEFAULT_CONTENT.to_string();
                let _ = std::fs::write(&maitres_file_path, &default_content);
                default_content
            });

        let (jpeg_quality, image_width) = Self::load_config(&dirs);

        // Create save thread
        let (save_request_tx,  save_request_rx)  = mpsc::channel::<SaveRequest>();
        let (save_response_tx, save_response_rx) = mpsc::channel::<SaveResponse>();

        let dirs_clone = dirs.clone();
        thread::spawn(move || {
            save_worker_thread(dirs_clone, save_request_rx, save_response_tx);
        });

        // Check initial PDF status
        let pdf_status = Self::check_pdf_status(&dirs);

        Self {
            dirs,
            faces,
            maitres_text,
            jpeg_quality,
            image_width,
            pdf_status,
            save_request_tx,
            save_response_rx,
        }
    }

    fn check_pdf_status(dirs: &Dirs) -> PdfStatus {
        // Look for trombinoscope PDF in the class directory
        let trombi_path = trombi_file_for_dir(&dirs.class, &dirs.class_name(), FileType::Trombi);

        if let Ok(metadata) = std::fs::metadata(&trombi_path) {
            PdfStatus::Ready {
                size_bytes: metadata.len(),
                generation_time: Duration::from_secs(0), // Unknown for existing files
            }
        } else {
            PdfStatus::Missing
        }
    }

    fn load_config(dirs: &Dirs) -> (u8, u32) {
        let config_path = dirs.class.join(CONFIG_FILENAME);
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut quality = DEFAULT_JPEG_QUALITY;
            let mut width = DEFAULT_IMAGE_WIDTH;

            for line in content.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    match key.trim() {
                        "jpeg_quality" => {
                            if let Ok(q) = value.trim().parse::<u8>() {
                                if q <= 100 {
                                    quality = q;
                                }
                            }
                        }
                        "image_width" => {
                            if let Ok(w) = value.trim().parse::<u32>() {
                                if (50..=2000).contains(&w) {
                                    width = w;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            (quality, width)
        } else {
            let default_config = format!("jpeg_quality={}\nimage_width={}\n", DEFAULT_JPEG_QUALITY, DEFAULT_IMAGE_WIDTH);
            let _ = std::fs::write(&config_path, &default_config);
            (DEFAULT_JPEG_QUALITY, DEFAULT_IMAGE_WIDTH)
        }
    }

    fn request_save(&mut self) {
        let save_data = self.collect_save_data();

        // If a save request arrives while the PDFs are being generated, finish
        // the current generation, and then immediately start the next one. Save
        // requests sent during generation are idempotent, so we need not record
        // more than one.
        use PdfStatus::*;
        match self.pdf_status {
            GeneratingLatest { started } => { self.pdf_status = PdfStatus::GeneratingPrevious { started }; }
            GeneratingPrevious { .. } => {} // Already queued, ignore additional requests
            Missing | Ready { .. } => {
                self.pdf_status = PdfStatus::GeneratingLatest { started: Instant::now() };
                let _ = self.save_request_tx.send(SaveRequest(save_data));
            }
        }
    }

    fn collect_save_data(&self) -> SaveData {
        let face_data: Vec<FaceData> = self.faces.iter()
            .filter_map(|face| {
                if let Data::Ready { face: face_in_image, image } = &face.data {
                    Some(FaceData {
                        path: face.path.clone(),
                        face: face_in_image.clone(),
                        image: image.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        SaveData {
            face_data,
            maitres_text: self.maitres_text.clone(),
            jpeg_quality: self.jpeg_quality,
            image_width: self.image_width,
        }
    }

    fn update_pdf_status(&mut self) {
        // Check for save completion messages
        while let Ok(response) = self.save_response_rx.try_recv() {
            match response {
                SaveResponse::Completed { generation_time } => {
                    match self.pdf_status {
                        PdfStatus::GeneratingPrevious { .. } => {
                            // Start the queued generation
                            let save_data = self.collect_save_data();
                            self.pdf_status = PdfStatus::GeneratingLatest { started: Instant::now() };
                            let _ = self.save_request_tx.send(SaveRequest(save_data));
                        }
                        _ => {
                            // Check the actual PDF file size
                            let size_bytes = self.get_pdf_size().unwrap_or(0);
                            self.pdf_status = PdfStatus::Ready { size_bytes, generation_time };
                        }
                    }
                }
                SaveResponse::Error(err) => {
                    eprintln!("Save error: {}", err);
                    // Reset to previous state
                    self.pdf_status = Self::check_pdf_status(&self.dirs);
                }
            }
        }
    }

    fn get_pdf_size(&self) -> Option<u64> {
        let trombi_path = trombi_file_for_dir(&self.dirs.class, &self.dirs.class_name(), FileType::Trombi);
        std::fs::metadata(&trombi_path).ok()?.len().into()
    }

    fn show_pdf_status(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Trombinoscope PDF:");

            match self.pdf_status {
                PdfStatus::Missing => {
                    ui.colored_label(Color32::LIGHT_RED, "Not generated yet");
                }
                PdfStatus::GeneratingLatest { started } => {
                    ui.colored_label(Color32::YELLOW, format!("Generating... ({:.1}s)", started.elapsed().as_secs_f32()));
                }
                PdfStatus::GeneratingPrevious { started } => {
                    ui.colored_label(Color32::ORANGE, format!("Generating... ({:.1}s) [Queued]", started.elapsed().as_secs_f32()));
                }
                PdfStatus::Ready { size_bytes, generation_time } => {
                    let size_kb = size_bytes as f64 / 1024.0;
                    let time_str = if generation_time.is_zero() { "earlier session".to_string() }
                    else                                        { format!("{:.1}s", generation_time.as_secs_f32()) };
                    ui.colored_label(Color32::LIGHT_GREEN, format!("Ready: {:.1} KB (generated in {})", size_kb, time_str));
                }
            }
        });
    }

    pub fn show(&mut self, ui: &mut Ui, ctx: &Context) {
        self.update_pdf_status();

        // Request repaint for ongoing operations that show time elapsing
        match self.pdf_status {
            PdfStatus::GeneratingLatest { .. } | PdfStatus::GeneratingPrevious { .. } => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            _ => {}
        }

        ui.heading("Classe".to_owned() + &self.dirs.class_name());

        ui.horizontal(|ui| {
            ui.label("Maîtres de classe :");
            ui.text_edit_singleline(&mut self.maitres_text);
        });

        ui.horizontal(|ui| {
            ui.label("Qualité JPEG :");
            ui.add(egui::Slider::new(&mut self.jpeg_quality, 1..=100).suffix("%"));
            ui.separator();
            ui.label("Largeur :");
            ui.add(egui::Slider::new(&mut self.image_width, 50..=1000).suffix("px"));
        });

        self.show_pdf_status(ui);

        ui.separator();

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
            key!{S (CTRL)  { self.request_save(); }}
            key!{Q (CTRL)  { std::process::exit(0) }} // TODO exit less brutally
        });
    }

    fn sort(&mut self) {
        self.faces.sort_by_cached_key(|f| match &f.data {
            Data::Loading(_) => ("zzzzzz".into(), "zzzz".into()),
            Data::Ready { face, .. } => (face.family.to_uppercase(), face.given.to_uppercase()),
        });
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
        image: Arc<DynamicImage>,
    },
}

pub struct Face {
    pub path: PathBuf,
    data: Data,
    texture: TextureHandle,
}

fn load_face_data(path: PathBuf) -> face::Result<Data> {
    let image = Arc::new(image::open(&path)?);
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
        tx_req.send((path.as_ref().into(), tx)).unwrap();
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
                let response = response.on_hover_ui(|ui| {
                    ui.vertical(|ui| {
                        enum X { C(&'static str, Color32), D(&'static str) }
                        fn xxx(ui: &mut Ui, stuff: &[X]) {
                            ui.horizontal(|ui| {
                                for thing in stuff {
                                    match thing {
                                        X::C(txt, col) => ui.colored_label(*col, *txt),
                                        X::D(txt     ) => ui.        label(      *txt),
                                    };
                                }
                            });
                        }
                        use X::*;
                        ui.heading(RichText::new("Déplacer le visage").color(Color32::LIGHT_BLUE));
                        xxx(ui, &[
                            D("• Clic : déplacer ce point au centre de l'image"),
                            C("ASTUCE : clic entre les yeux", Color32::YELLOW),
                        ]);
                        xxx(ui, &[
                            D("• Clavier : ⬅➡⬆⬇ "),
                            C("SHIFT: plus vite", Color32::RED),
                        ]);
                        xxx(ui, &[
                            D("• Glisser avec souris bouton gauche"),
                            C("SHIFT : plus vite", Color32::RED),
                            C("CTRL : plus lentement", Color32::GREEN),
                        ]);
                        ui.separator();
                        ui.heading(RichText::new("Redimensionner le visage").color(Color32::LIGHT_BLUE));
                        xxx(ui, &[
                            D("• Clavier : CTRL ⬆⬇"),
                            C("SHIFT : plus vite", Color32::RED),
                        ]);
                        xxx(ui, &[
                            D("• Glisser avec souris bouton droit"),
                            C("SHIFT : plus vite", Color32::RED),
                            C("CTRL : plus lentement", Color32::GREEN),
                        ]);
                        ui.separator();
                        ui.heading(RichText::new("Tourner l'image").color(Color32::LIGHT_BLUE));
                        ui.label("• Clavier : CTRL ⬅➡");
                        ui.separator();
                        ui.label("• CTRL S : sauvegarder");
                        ui.label("• CTRL Q : quitter sans sauvegarder");
                    });
                });
                if response.clicked_by(PointerButton::Primary) {
                    self.centre_on_pointer(&response)
                }
                if response.dragged() {
                    let Vec2 { mut x, mut y } = response.drag_motion();
                    ctx.input(|i| {
                        if i.modifiers.shift {
                            x *= 3.0;
                            y *= 3.0;
                        }
                        if i.modifiers.ctrl {
                            x /= 5.0;
                            y /= 5.0;
                        }
                    });
                    if response.dragged_by(PointerButton::Primary)   { self.mv(-x, -y); }
                    if response.dragged_by(PointerButton::Secondary) { self.zoom(y); }
                }
                //if response.is_pointer_button_down_on() {
                if response.hovered() {
                    ctx.input(|i| {
                        let mut delta = 5.0;
                        if i.modifiers.shift { delta *= 5.0; }
                        if i.modifiers.ctrl {
                            if i.key_pressed(Key::ArrowRight) { self.rotate( 1); }
                            if i.key_pressed(Key::ArrowLeft ) { self.rotate(-1); }
                            if i.key_pressed(Key::ArrowDown ) { self.zoom( delta); }
                            if i.key_pressed(Key::ArrowUp   ) { self.zoom(-delta); }
                        } else {
                            if i.key_pressed(Key::ArrowRight) { self.mv(-delta, 0.0); }
                            if i.key_pressed(Key::ArrowLeft ) { self.mv( delta, 0.0); }
                            if i.key_pressed(Key::ArrowDown ) { self.mv( 0.0, -delta); }
                            if i.key_pressed(Key::ArrowUp   ) { self.mv( 0.0,  delta); }
                        }
                    });
                }
            });
            match &mut self.data {
                Data::Ready { face, .. } => {
                    let g = ui.text_edit_singleline(&mut face.given ).on_hover_text("Prénom");
                    let f = ui.text_edit_singleline(&mut face.family).on_hover_text("Nom de famille");
                    if g.lost_focus() || f.lost_focus() { request_sort = true; }
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

    pub fn centre_on_pointer(&mut self, response: &Response) {
        if let Data::Ready { face: FaceInImage { cx, cy, w, .. }, .. } = &mut self.data {
            if let Some(Pos2{ x, y }) = response.interact_pointer_pos() {
                let Rect{ min: Pos2 { x: x_min, y: y_min } , max: Pos2 { x: x_max, y: y_max } } = response.rect;
                let x_frac = (x - x_min) / (x_max - x_min);
                let y_frac = (y - y_min) / (y_max - y_min);
                let h = *w * ASPECT_RATIO;
                *cx = *cx - *w / 2.0 + x_frac * *w;
                *cy = *cy -  h / 2.0 + y_frac *  h;
                self.set_texture_from_cropped_image();
            }
        }
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

fn save_worker_thread(
    dirs: Dirs,
    request_rx: mpsc::Receiver<SaveRequest>,
    response_tx: mpsc::Sender<SaveResponse>,
) {
    for request in request_rx {
        match request {
            SaveRequest(save_data) => {
                let start_time = Instant::now();

                match save_and_regenerate(save_data, &dirs) {
                    Ok(()) => { let _ = response_tx.send(SaveResponse::Completed { generation_time : start_time.elapsed() }); }
                    Err(e) => { let _ = response_tx.send(SaveResponse::Error(e.to_string())); }
                }
            }
        }
    }
}

fn save_and_regenerate(save_data: SaveData, dirs: &Dirs) -> face::Result<()> {
    save_many_face_metadata(&save_data.face_data)?;

    let maitres_file_path = dirs.class.join(MAITRES_DE_CLASSE_FILENAME);
    std::fs::write(&maitres_file_path, &save_data.maitres_text)?;

    // Save config settings
    save_config(&save_data, dirs)?;

    ensure_empty_dir(&dirs.work)?;
    ensure_empty_dir(&dirs.render)?;
    write_many_face_images(&save_data.face_data, &dirs.work, save_data.jpeg_quality, save_data.image_width)?;
    trombinoscope(dirs);
    Ok(())
}

fn save_config(save_data: &SaveData, dirs: &Dirs) -> std::io::Result<()> {
    let config_path = dirs.class.join(CONFIG_FILENAME);
    let config_content = format!("jpeg_quality={}\nimage_width={}\n", save_data.jpeg_quality, save_data.image_width);
    std::fs::write(&config_path, &config_content)
}

/// Store the location and name of each face in the JPEG segment of the image
/// containing the face
fn save_many_face_metadata(face_data: &[FaceData]) -> face::Result<()> {
    for data in face_data {
        data.face.embed_in_jpeg(&data.path)?;
    }
    Ok(())
}

/// Save each cropped face in its own image file in `dir`. Assumes `dir` exists.
fn write_many_face_images(face_data: &[FaceData], dir: impl AsRef<Path>, quality: u8, width: u32) -> face::Result<()> {
    let dir_path = dir.as_ref().to_path_buf(); // Convert to owned PathBuf for sharing across threads

    let results: Vec<face::Result<()>> = face_data
        .par_iter()
        .map(|data| write_one_face_image(data, &dir_path, quality, width))
        .collect();

    // Return the first error if any occurred
    for result in results {
        result?;
    }

    Ok(())
}

/// Save one cropped face in its own image file in `dir`. Assumes `dir` exists.
fn write_one_face_image(data: &FaceData, dir: impl AsRef<Path>, quality: u8, target_width: u32) -> face::Result<()> {
    let filename = format!("{} @ {}.jpg", &data.face.given, &data.face.family);
    let path = dir.as_ref().join(&filename);
    let file = &mut File::create(path)?;

    let cropped = crop(&data.image, &data.face);

    let target_height = (target_width as f32 * ASPECT_RATIO) as u32;
    let resized = cropped.resize(target_width, target_height, image::imageops::FilterType::Lanczos3);

    let mut encoder = JpegEncoder::new_with_quality(file, quality);

    encoder.encode(
        resized.as_bytes(),
        target_width,
        target_height,
        image::ExtendedColorType::Rgb8
    )?;
    Ok(())
}
