// TODO centre class name in header
// TODO report locations of generated output PDFs
// TODO quit confirm / save / cancel dialog
// TODO speed slider in help
// TODO multiple faces in one photo
// TODO fix sporadic inability to edit names
// TODO option to rename files from metadata names ?

// TODO Thumbhash or something else from https://lucasmerlin.github.io/hello_egui/

use std::{fs::File, path::{Path, PathBuf}, sync::{mpsc, Arc}, time::{Duration, Instant}, thread};

use eframe::egui;
use egui::{Color32, ColorImage, Context, Key, PointerButton, Popup, PopupCloseBehavior, Pos2, Rect, Response, RichText, Sense, TextureHandle, TextureOptions, Ui, Vec2};
use image::{codecs::jpeg::JpegEncoder, DynamicImage};
use rayon::prelude::*;
use snafu::ResultExt;

use face::{FaceInImage, ASPECT_RATIO};
use render::{trombinoscope, trombi_file_for_dir};
use util::{
    ensure_empty_dir, find_jpgs_in_dir, Dirs,
    MAITRES_DE_CLASSE_FILENAME, MAITRES_DE_CLASSE_DEFAULT_CONTENT,
    CONFIG_FILENAME, DEFAULT_JPEG_QUALITY, DEFAULT_IMAGE_WIDTH,
    scan_class_dir, bootstrap_originaux, Situation,
    set_aside, SetAsideDestination,
};

mod error;
use error::{CreateFileSnafu, WriteFileSnafu, RenameFileSnafu, EncodeJpegSnafu};
pub use error::{Error, Result};

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = cli::parse();
    let originals_subdir = cli.original_photos_subdir.to_string_lossy().into_owned();
    let dirs = Dirs::new(&cli.class_dir, &originals_subdir)?;

    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    // Decide whether this class directory has already been set up (photos already
    // living in `originals_subdir`), needs bootstrapping (photos freshly dropped at
    // top level, nothing done to them yet), or looks too unusual to touch automatically.
    let situation = scan_class_dir(&dirs.class, &originals_subdir)?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true),
        ..Default::default()
    };

    let strip_metadata = cli.strip_metadata;

    eframe::run_native(
        "Trombinoscope",
        options,
        Box::new(move |cc| {
            Ok(Box::new(TopApp::new(dirs, originals_subdir, situation, strip_metadata, &cc.egui_ctx)?))
        }),
    )?;

    Ok(())
}

/// Top-level application state: before the operator has confirmed a first-run
/// bootstrap, after we've decided the directory doesn't look workable, or the usual
/// running trombinoscope editor.
enum TopApp {
    Bootstrap(BootstrapConfirm),
    Weird(String),
    Running(App),
}

impl TopApp {
    fn new(
        dirs: Dirs,
        originals_subdir: String,
        situation: Situation,
        strip_metadata: bool,
        egui_ctx: &Context,
    ) -> Result<Self> {
        Ok(match situation {
            Situation::Fresh { jpgs } => TopApp::Bootstrap(BootstrapConfirm {
                dirs, originals_subdir, jpgs, strip_metadata,
                error: None,
            }),
            Situation::Established => {
                if strip_metadata { face::metadata::strip_from_jpgs_in_dir(&dirs.photo)?; }
                TopApp::Running(App::try_new(dirs, egui_ctx)?)
            }
            Situation::Weird(message) => {
                // The operator this is aimed at won't be watching a terminal, but we
                // print here too in case this is being run/diagnosed non-interactively.
                println!("{message}");
                TopApp::Weird(message)
            }
        })
    }
}

impl eframe::App for TopApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let transition = match self {
            TopApp::Bootstrap(state) => state.show(ctx),
            TopApp::Weird(message)   => { show_weird_screen(ctx, message.as_str()); None }
            TopApp::Running(app)     => { <App as eframe::App>::update(app, ctx, frame); None }
        };
        if let Some(next) = transition {
            *self = next;
        }
    }
}

/// Awaiting operator confirmation before moving freshly-arrived photos (loose at the
/// top level of the class directory) into the originals subdirectory. See
/// `util::Situation::Fresh`.
struct BootstrapConfirm {
    dirs: Dirs,
    originals_subdir: String,
    jpgs: Vec<String>,
    strip_metadata: bool,
    /// Set if a previous attempt to confirm failed, so it can be shown alongside a
    /// retry of the same big button.
    error: Option<String>,
}

impl BootstrapConfirm {
    fn show(&mut self, ctx: &Context) -> Option<TopApp> {
        let mut confirmed = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Première utilisation dans ce répertoire").size(34.0).strong());
                ui.add_space(24.0);
                ui.label(RichText::new(format!(
                    "{} photo(s) trouvée(s) au premier niveau de ce répertoire.",
                    self.jpgs.len(),
                )).size(20.0));
                ui.label(RichText::new(format!(
                    "Elles vont être déplacées dans un nouveau sous-répertoire : « {} ».",
                    self.originals_subdir,
                )).size(20.0));

                ui.add_space(16.0);
                egui::CollapsingHeader::new(format!("Voir les {} photo(s) concernée(s)", self.jpgs.len()))
                    .show(ui, |ui| {
                        for jpg in &self.jpgs {
                            ui.label(jpg.as_str());
                        }
                    });

                if let Some(error) = &self.error {
                    ui.add_space(24.0);
                    ui.colored_label(Color32::LIGHT_RED, RichText::new(error.as_str()).size(18.0));
                }

                ui.add_space(48.0);
                let confirm = egui::Button::new(RichText::new("Continuer").size(30.0).strong())
                    .min_size(Vec2::new(280.0, 70.0))
                    .fill(Color32::from_rgb(40, 130, 60));
                if ui.add(confirm).clicked() {
                    confirmed = true;
                }

                ui.add_space(16.0);
                let cancel = egui::Button::new(RichText::new("Annuler").size(30.0).strong())
                    .min_size(Vec2::new(200.0, 70.0))
                    .fill(Color32::from_rgb(160, 40, 40));
                if ui.add(cancel).clicked() {
                    // Nothing has been touched yet at this point, so quitting is enough.
                    std::process::exit(0);
                }
            });
        });

        if confirmed { self.confirm(ctx) } else { None }
    }

    /// Perform the (transactional) move, then either hand back a `Running` app, a
    /// `Weird` explanation if something went wrong after the move itself succeeded, or
    /// (by returning `None`) stay put with `self.error` set so the same screen can be
    /// shown again with the failure and an unchanged "Continuer" button to retry.
    fn confirm(&mut self, ctx: &Context) -> Option<TopApp> {
        if let Err(e) = bootstrap_originaux(&self.dirs.class, &self.originals_subdir, &self.jpgs) {
            self.error = Some(e.to_string());
            return None;
        }

        if self.strip_metadata {
            if let Err(e) = face::metadata::strip_from_jpgs_in_dir(&self.dirs.photo) {
                return Some(TopApp::Weird(format!(
                    "Les photos ont bien été déplacées vers « {} », mais le nettoyage des métadonnées a échoué : {e}",
                    self.originals_subdir,
                )));
            }
        }

        Some(match App::try_new(self.dirs.clone(), ctx) {
            Ok(app) => TopApp::Running(app),
            Err(e)  => TopApp::Weird(format!(
                "Les photos ont bien été déplacées vers « {} », mais l'application n'a pas pu démarrer : {e}",
                self.originals_subdir,
            )),
        })
    }
}

/// Situation 3: doesn't cleanly look like a first run or an established one. Explained
/// both here (for the non-technical operator) and on stdout (see `main`), then quits.
fn show_weird_screen(ctx: &Context, message: &str) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Ce répertoire ne semble pas prêt").size(34.0).strong());
            ui.add_space(24.0);
            ui.label(RichText::new(message).size(20.0));
            ui.add_space(48.0);
            let button = egui::Button::new(RichText::new("Compris").size(30.0).strong())
                .min_size(Vec2::new(240.0, 70.0));
            if ui.add(button).clicked() {
                std::process::exit(0);
            }
        });
    });
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

    fn try_new(dirs: Dirs, egui_ctx: &Context) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<(PathBuf, mpsc::Sender<face::Result<Data>>)>();
        std::thread::spawn(move || {
            for (path, tx) in rx.iter() {
                let _ = tx.send(load_face_data(path));
            }
        });

        let faces = find_jpgs_in_dir(&dirs.photo)?
            .into_iter()
            .map(move |path| Face::load(path, egui_ctx, tx.clone()))
            .collect::<face::Result<Vec<_>>>()?;

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

        Ok(Self {
            dirs,
            faces,
            maitres_text,
            jpeg_quality,
            image_width,
            pdf_status,
            save_request_tx,
            save_response_rx,
        })
    }

    fn check_pdf_status(dirs: &Dirs) -> PdfStatus {
        // Look for trombinoscope PDF in the class directory
        let trombi_path = trombi_file_for_dir(&dirs.class, &dirs.class_name);

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

    fn resolve_duplicate_name(&self, given: &str, family: &str, exclude_index: usize) -> (String, String) {
        let mut candidate_family = family.to_string();

        loop {
            // Check if this name pair is used by any other face
            let is_duplicate = self.faces.iter().enumerate()
                .any(|(i, face)| {
                    i != exclude_index &&
                    if let Data::Ready { face, .. } = &face.data {
                        face.given == given && face.family == candidate_family
                    } else {
                        false
                    }
                });

            if !is_duplicate {
                return (given.to_string(), candidate_family);
            }

            candidate_family.push_str("-dup");
        }
    }

    fn apply_name_change(&mut self, face_index: usize, given: &str, family: &str) -> Result<()> {
        let face = &mut self.faces[face_index];

        // Update the face data with resolved names
        if let Data::Ready { face: face_data, .. } = &mut face.data {
            face_data.given  = given.to_string();
            face_data.family = family.to_string();
        }

        // Attempt to rename the file
        face.rename_for_names(given, family)?;

        Ok(())
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
        let trombi_path = trombi_file_for_dir(&self.dirs.class, &self.dirs.class_name);
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

        ui.heading("Classe".to_owned() + &self.dirs.class_name);

        ui.horizontal(|ui| {
            ui.label("Maîtres de classe :");
            ui.text_edit_singleline(&mut self.maitres_text);
        });

        ui.horizontal(|ui| {
            ui.label("Qualité JPEG :");
            ui.add(egui::Slider::new(&mut self.jpeg_quality, 1..=100).suffix("%"));
            ui.separator();
            ui.label("Largeur :");
            ui.add(egui::Slider::new(&mut self.image_width, 50..=500).suffix("px"));
        });

        self.show_pdf_status(ui);

        ui.separator();

        let mut sort = false;
        let mut name_changes = Vec::new();
        let mut set_asides = Vec::new();
        let n_rows = self.faces.len() / 6 + 1;

        egui::Grid::new("face grid").show(ui, |ui| {
            for (n, face) in self.faces.iter_mut().enumerate() {
                let (sort_requested, name_change, set_aside) = face.show(ui, ctx, n_rows);
                if sort_requested {
                    sort = true;
                }
                if let Some((given, family)) = name_change {
                    name_changes.push((n, given, family));
                }
                if let Some(destination) = set_aside {
                    set_asides.push((n, destination));
                }
                if n % 6 == 5 { ui.end_row() }
            }
        });

        // Process name changes with duplicate resolution
        for (face_index, given, family) in name_changes {
            let (final_given, final_family) = self.resolve_duplicate_name(&given, &family, face_index);
            match self.apply_name_change(face_index, &final_given, &final_family) {
                Ok(()) => {
                    sort = true;
                }
                Err(e) => {
                    eprintln!("Failed to rename file: {}", e);
                    // Revert names on file system error - clone values first to avoid borrow conflicts
                    let saved_given  = self.faces[face_index].last_saved_given.clone();
                    let saved_family = self.faces[face_index].last_saved_family.clone();
                    if let Data::Ready { face, .. } = &mut self.faces[face_index].data {
                        face.given  = saved_given;
                        face.family = saved_family;
                    }
                }
            }
        }

        // Process set-aside requests. Removing from `self.faces` invalidates every
        // index after the one removed, so this must happen in descending order.
        set_asides.sort_by_key(|(face_index, _)| std::cmp::Reverse(*face_index));
        for (face_index, destination) in set_asides {
            match set_aside(&self.dirs, &self.faces[face_index].path, destination) {
                Ok(_) => { self.faces.remove(face_index); }
                Err(e) => eprintln!("Failed to set aside photo: {}", e),
            }
        }

        if sort { self.sort(); }
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
            egui::ScrollArea::both().show(ui, |ui| {
                self.show(ui, ctx);
            });
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
    last_saved_given: String,
    last_saved_family: String,
    /// While the right-click "set aside" menu is open: `None` until a destination has
    /// been picked once (menu shows both options), `Some(d)` once `d` has been picked
    /// and is awaiting a second click to actually confirm it (menu shows only `d`, as
    /// a question). Reset to `None` whenever the menu (re)opens, so a stale arming
    /// can never survive from one open of the menu to the next.
    set_aside_armed: Option<SetAsideDestination>,
}

// Parse names from filename in "Given @ Family.jpg" format
// Also accepts formats without spaces around @
fn parse_names_from_filename(path: &Path) -> (String, String) {
    let filename = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if let Some((given, family)) = filename.split_once("@") {
        (given.trim().to_string(), family.trim().to_string())
    } else {
        (String::new(), String::new())
    }
}

// Sanitize names for use in filenames
fn sanitize_name_for_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

// Create filename from given and family names
fn create_filename_for_names(given: &str, family: &str) -> String {
    let given_clean  = sanitize_name_for_filename(given);
    let family_clean = sanitize_name_for_filename(family);

    if given_clean.is_empty() && family_clean.is_empty() {
        "Unknown".to_string()
    } else if given_clean.is_empty() {
        family_clean
    } else if family_clean.is_empty() {
        given_clean
    } else {
        format!("{} @ {}", given_clean, family_clean)
    }
}

// Rename file to match the given and family names
fn rename_face_file(old_path: &Path, given: &str, family: &str) -> Result<PathBuf> {
    let new_filename = create_filename_for_names(given, family);
    let extension = old_path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("jpg");
    let new_path = old_path.with_file_name(format!("{}.{}", new_filename, extension));

    if old_path != new_path {
        std::fs::rename(old_path, &new_path)
            .context(RenameFileSnafu { from: old_path, to: &new_path })?;
    }

    Ok(new_path)
}

fn load_face_data(path: PathBuf) -> face::Result<Data> {
    let image = Arc::new(image::open(&path)?);
    let (given, family) = parse_names_from_filename(&path);

    // Load face position data from metadata, but get names from filename
    let mut face = FaceInImage::from_path_or_default_for(&path, image.width() as f32, image.height() as f32)?;
    face.given  = given;
    face.family = family;

    Ok(Data::Ready { face, image })
}

impl Face {

    fn load(
        path: impl AsRef<Path>,
        egui_ctx: &Context,
        tx_req: mpsc::Sender<(PathBuf, mpsc::Sender<face::Result<Data>>)>,
    ) -> face::Result<Face> {
        let (tx, rx) = mpsc::channel();
        let _ = tx_req.send((path.as_ref().into(), tx));
        let texture_name = path.as_ref().to_string_lossy().to_string();
        let dummy_image = ColorImage::filled([400,500], Color32::GRAY);
        let texture = egui_ctx.load_texture(&texture_name, dummy_image, egui::TextureOptions::default());

        let (initial_given, initial_family) = parse_names_from_filename(path.as_ref());

        Ok(Face {
            path: path.as_ref().into(),
            data: Data::Loading(rx),
            texture,
            last_saved_given:  initial_given,
            last_saved_family: initial_family,
            set_aside_armed: None,
        })
    }

    fn install_data(&mut self, data: Data) {
        if let Data::Ready { ref face, ref image } = data {
            let cropped_image = crop(image, face);
            let image_data = adapt_for_texture(&cropped_image);
            self.texture.set(image_data, egui::TextureOptions::default());
            self.last_saved_given  = face.given.clone();
            self.last_saved_family = face.family.clone();
            self.data = data;
        }
    }

    fn rename_for_names(&mut self, given: &str, family: &str) -> Result<()> {
        let new_path = rename_face_file(&self.path, given, family)?;
        self.path = new_path;
        self.last_saved_given  = given.to_string();
        self.last_saved_family = family.to_string();
        Ok(())
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &Context,
        n_rows: usize,
    ) -> (bool, Option<(String, String)>, Option<SetAsideDestination>) {
        let w = ctx.available_rect().width();
        let h = ctx.available_rect().height();
        let top_margin = 10.0;
        let mut request_sort = false;
        let mut name_change = None;
        let mut set_aside_request = None;

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
                        ui.separator();
                        ui.label("• Clic droit : écarter cette photo (liste de classe, ou mise à l'écart)");
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
                // A fresh right-click always starts from the unarmed (two-destination)
                // menu, never resuming a confirm state left over from a previous open.
                if response.secondary_clicked() {
                    self.set_aside_armed = None;
                }
                // Not the `response.context_menu(...)` shortcut: that hard-codes egui's
                // default `CloseOnClick` behaviour, which would tear the popup down on
                // the very first ("arm") click below, before the second ("confirm")
                // click ever gets a chance to render. `CloseOnClickOutside` leaves the
                // popup open across both clicks; it's still closed explicitly via
                // `ui.close()` on "Fermer", "Annuler", and on confirm.
                Popup::context_menu(&response)
                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        set_aside_request = self.show_set_aside_menu(ui);
                    });
            });

            // Handle UI and track if we need to process name changes
            let (lost_focus, names_changed) = match &mut self.data {
                Data::Ready { face, .. } => {
                    let g = ui.text_edit_singleline(&mut face.given ).on_hover_text("Prénom");
                    let f = ui.text_edit_singleline(&mut face.family).on_hover_text("Nom de famille");

                    let lost_focus = g.lost_focus() || f.lost_focus();
                    let names_changed = face.given != self.last_saved_given || face.family != self.last_saved_family;

                    (lost_focus, names_changed)
                }
                Data::Loading(rx) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(10));
                    match rx.try_recv() {
                        Ok(Ok(d)) => { self.install_data(d); request_sort = true; }
                        Ok(Err(face::Error::FaceNotLoaded)) => (),
                        _ => (),
                    }
                    (false, false)
                }
            };

            // If names changed and focus lost, signal this to the App for processing
            if lost_focus && names_changed {
                if let Data::Ready { face, .. } = &self.data {
                    name_change = Some((face.given.clone(), face.family.clone()));
                }
            }
        });

        (request_sort, name_change, set_aside_request)
    }

    /// Content of the right-click "set aside" menu. Returns `Some(d)` only on the
    /// frame the operator's second click actually confirms destination `d` — every
    /// other interaction (arming a destination, cancelling, closing) returns `None`
    /// and is reflected purely in `self.set_aside_armed`.
    fn show_set_aside_menu(&mut self, ui: &mut Ui) -> Option<SetAsideDestination> {
        match self.set_aside_armed {
            None => {
                for (destination, label) in [
                    (SetAsideDestination::Liste,    "Photo de la liste de classe"),
                    (SetAsideDestination::Ecartees, "Écarter cette photo"),
                ] {
                    if ui.add_sized([180.0, 24.0], egui::Button::new(label)).clicked() {
                        self.set_aside_armed = Some(destination);
                    }
                }
                if ui.add_sized([180.0, 24.0], egui::Button::new("Fermer")).clicked() {
                    ui.close();
                }
                None
            }
            Some(destination) => {
                let (question, confirm_label) = match destination {
                    SetAsideDestination::Liste    => ("Photo de la liste de classe ?", "Oui, photo de la liste"),
                    SetAsideDestination::Ecartees => ("Écarter cette photo ?",         "Oui, écarter"),
                };
                // The question sits where the just-clicked "arm" button was, but as a
                // plain (unclickable) label, so a fast accidental double-click there
                // lands on nothing. `Annuler` and `Confirmer` are then pushed well
                // clear of that spot, spaced apart from each other, and sized well
                // above the default button size, so a rushed second click can't land
                // on the wrong one, or on either one by mistake.
                ui.label(RichText::new(question).strong());
                ui.add_space(10.0);
                let cancel = ui.add_sized([180.0, 28.0], egui::Button::new("Annuler")).clicked();
                ui.add_space(14.0);
                let confirm = egui::Button::new(RichText::new(confirm_label).strong())
                    .fill(Color32::from_rgb(160, 40, 40));
                let confirmed = ui.add_sized([180.0, 36.0], confirm).clicked();
                if cancel {
                    self.set_aside_armed = None;
                }
                if confirmed {
                    self.set_aside_armed = None;
                    ui.close();
                    Some(destination)
                } else {
                    None
                }
            }
        }
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

fn save_and_regenerate(save_data: SaveData, dirs: &Dirs) -> Result<()> {
    // Only save face position metadata, not names (names are in filenames now)
    save_face_position_metadata(&save_data.face_data)?;

    let maitres_file_path = dirs.class.join(MAITRES_DE_CLASSE_FILENAME);
    std::fs::write(&maitres_file_path, &save_data.maitres_text)
        .context(WriteFileSnafu { path: &maitres_file_path })?;

    // Save config settings
    save_config(&save_data, dirs)?;

    ensure_empty_dir(&dirs.work)?;
    ensure_empty_dir(&dirs.render)?;
    write_many_face_images(&save_data.face_data, &dirs.work, save_data.jpeg_quality, save_data.image_width)?;
    trombinoscope(dirs)?;
    Ok(())
}

fn save_config(save_data: &SaveData, dirs: &Dirs) -> Result<()> {
    let config_path = dirs.class.join(CONFIG_FILENAME);
    let config_content = format!("jpeg_quality={}\nimage_width={}\n", save_data.jpeg_quality, save_data.image_width);
    std::fs::write(&config_path, &config_content).context(WriteFileSnafu { path: &config_path })?;
    Ok(())
}

/// Store only the position and cropping info of each face in the JPEG metadata
/// Names are now stored in filenames, not metadata
fn save_face_position_metadata(face_data: &[FaceData]) -> Result<()> {
    for data in face_data {
        // Create a copy of the face with empty names for metadata storage
        let mut face_for_metadata = data.face.clone();
        face_for_metadata.given  = String::new();
        face_for_metadata.family = String::new();
        face_for_metadata.embed_in_jpeg(&data.path)?;
    }
    Ok(())
}

/// Save each cropped face in its own image file in `dir`. Assumes `dir` exists.
fn write_many_face_images(face_data: &[FaceData], dir: impl AsRef<Path>, quality: u8, width: u32) -> Result<()> {
    let dir_path = dir.as_ref().to_path_buf(); // Convert to owned PathBuf for sharing across threads

    let results: Vec<Result<()>> = face_data
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
fn write_one_face_image(data: &FaceData, dir: impl AsRef<Path>, quality: u8, target_width: u32) -> Result<()> {
    let filename = format!("{} @ {}.jpg", &data.face.given, &data.face.family);
    let path = dir.as_ref().join(&filename);
    let file = &mut File::create(&path).context(CreateFileSnafu { path: &path })?;

    let cropped = crop(&data.image, &data.face);

    let target_height = (target_width as f32 * ASPECT_RATIO) as u32;
    let resized = cropped.resize(target_width, target_height, image::imageops::FilterType::Lanczos3);

    let mut encoder = JpegEncoder::new_with_quality(file, quality);

    encoder.encode(
        resized.as_bytes(),
        target_width,
        target_height,
        image::ExtendedColorType::Rgb8
    ).context(EncodeJpegSnafu { path: &path })?;
    Ok(())
}
