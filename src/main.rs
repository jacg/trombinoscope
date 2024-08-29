use std::{
    cmp::Ordering, ffi::OsStr, fs::{self, File}, io::{self, Write}, path::{Path, PathBuf}, time::Instant
};

use image::{DynamicImage, GenericImageView, codecs::jpeg::JpegEncoder};
use img_parts::jpeg::{self, JpegSegment, Jpeg};
use show_image::event;
use bitcode::{self, Encode, Decode};

use clap::Parser;
use show_image::create_window;

use typst::{
    foundations::Smart,
    eval::Tracer,
};

use trombinoscope::{
    self as tromb,
    typst::TypstWrapperWorld,
};

#[derive(Parser)]
struct Cli {
    /// Directory containing the class assets
    class_dir: PathBuf,

    #[arg(long)]
    strip_old_metadata: bool,
}

#[derive(Debug)]
struct Dirs {
    class: PathBuf,
    photo: PathBuf,
    render: PathBuf,
    work: PathBuf,
}

impl Dirs {
    fn new(class_dir: impl AsRef<Path>) -> Self {
        let class: PathBuf = class_dir.as_ref().into();
        Self {
            photo: class.join("Complet"),
            render: class.join("Recadré"),
            class,
            work: "/tmp/trombinoscope-working-dir".into(),
        }
    }
    fn class_name(&self) -> String { class_from_dir(&self.class)  }
}

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let dirs = Dirs::new(cli.class_dir);

    let start = Instant::now();
    let mut faces = std::fs::read_dir(&dirs.photo)?
        .take(100)
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .filter_map(|path| Cropped::load(path, cli.strip_old_metadata))
        .collect::<Vec<_>>();
    println!("Loading all images took {:.1?}", start.elapsed());

    let window = create_window("image", Default::default())?;
    crop_interactively(&mut faces, &window, &dirs).unwrap();
    save_and_regenerate(&faces, &dirs);
    Ok(())
}

fn write_cropped(in_file: impl AsRef<Path>, out_dir: impl AsRef<Path>) {
    let cropped = Cropped::load(in_file, false).unwrap();
    let out_file = out_dir.as_ref().join(cropped.path.file_name().unwrap());
    println!("Writing {}", out_file.display());
    cropped.write(out_file).unwrap();
}

fn render(
    content: String,
    dir: &Dirs,
    ftype: FileType,
) {
    let class_name = dir.class_name();
    let typst_src_filename = format!("generated-{}.typ", match ftype {
        FileType::Trombi => format!("tombinoscope_{class_name}"),
        FileType::Labels => format!("étiquettes_{class_name}"),
    });

    let typst_src_path = dir.work.join(&typst_src_filename);
    let mut out = File::create(&typst_src_path).unwrap();
    out.write_all(content.as_bytes()).unwrap();

    // Create world with content.
    let world = TypstWrapperWorld::new(dir.work.display().to_string(), content.clone());

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer)
        .unwrap_or_else(|err| {
            panic!("\nError compiling typst source `{typst_src_filename}`:\n{err:?}\n")
        });

    // Output to pdf
    let pdf_bytes = typst_pdf::pdf(&document, Smart::Auto, None);

    let pdf_path = trombi_file_for_dir(&dir.work, &dir.class_name(), ftype);
    let pdf_path_display = pdf_path.display();

    fs::write(&pdf_path, pdf_bytes)
        .unwrap_or_else(|err| panic!("Error writing {pdf_path_display}:\n{err:?}"));

    let moved_pdf_path = trombi_file_for_dir(&dir.class, &dir.class_name(), ftype);
    let moved_pdf_path_display = moved_pdf_path.display();
    let msg = &format!("PDF généré: `{moved_pdf_path_display}`.");
    println!("{msg}");
}

fn labels_typst_src(items: &[Item], dir: &Dirs) -> String {
    let institution = "CO Montbrillant";
    let class_name = dir.class_name();
    let label = |given, family| format!("label([{given}], [{family}])");
    let labels = items
        .iter()
        .map(|i| label(i.name.given.clone(), i.name.family.clone()))
        .collect::<Vec<_>>()
        .join(",\n    ");

    format!{r#"#set page(
  paper: "a4",
  margin: (top: 10mm, bottom: 4mm, left: 5mm, right: 5mm),
)
#set text(size: 23pt, font: "Inconsolata", weight: "black")

#let colG = rgb(150,0,0)
#let colF = rgb(0,0,150)

#let curry_label(institution, class) = {{
    (given, family) => {{
        set rect(width: 10cm, height: 13.3mm, stroke: none)
        stack(
        dir: ttb,
        rect(),
        rect(align(bottom, text(stroke: none, fill: colF, upper[#family]))),
        rect(              text(stroke: none, fill: colG,      [#given])),
        rect(                                                  [#class]),
        rect(                                                  [#institution]),
       )
   }}
}}

#let label = curry_label([{institution}], [Classe {class_name}])

#table(
    columns: 2,
    align: center + horizon,
    stroke: 0.6pt + gray,
    {labels}
)"#}
}

fn trombi_typst_src(items: &[Item], dir: &Dirs) -> String {
    let table_items = items
        .iter()
        .map(|Item { image, name: Name { given, family } }| {
            format!("    item([{given}], [{family}], \"{image}\")", image=image.display())
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let class_name = dir.class_name();

    format!(r#"#set page(
  paper: "a4",
  margin: (top: 10mm, bottom: 4mm, left: 5mm, right: 5mm),
)

#let colG = rgb(150,0,0)
#let colF = rgb(0,0,150)

#align(center, text([CLASSE {class_name}], size: 50pt))

#v(-10mm) // TODO find sensible way of reducing space before table

#let pic(path) = image(path, width: 100%)

#let label(given, family) = [
    #text(given , stroke: none, fill: colG) #h(1mm)
    #text(family, stroke: none, fill: colF)
]

#let n_columns = 6
#let pic_w = 200mm / n_columns
#let pic_h = pic_w * 5 / 4

#let item(given, family, path) = {{
    set rect(
        width: pic_w,
        inset: 5pt,
        stroke: 0.5pt + gray,
        height: 10mm,
    )

    let given  = text(stroke: none, fill: colG,        given  )
    let family = text(stroke: none, fill: colF, upper[#family])

    stack(
        dir: ttb,
        rect(pic(path), height: pic_h, stroke: (           bottom: none)),
        rect(align(bottom, given )   , stroke: (top: none, bottom: none)),
        rect(align(top   , family)   , stroke: (top: none              )),
    )
}}

#table(
    columns: n_columns,
    align: center + horizon,
    stroke: none,
    inset: 0pt,

{table_items}
)
"#)

}



#[derive(Encode, Decode, PartialEq, Debug)]
struct Metadata {
    given: String,
    family: String,
    x: i32,
    y: i32,
    w: i32,
    rotate: i8,
}

#[derive(Debug)]
pub struct Cropped {
    pub path: PathBuf,
    image: DynamicImage,
    pub given: String,
    pub family: String,
    x: i32,
    y: i32,
    w: i32,
    /// height to width aspect ratio
    r: (i32, i32),
    rotate: i8,
    rotated_cache: DynamicImage,
}

#[derive(Debug, Clone)      ] struct Name { given: String, family: String }
#[derive(Debug, Clone)      ] struct Item { image: PathBuf, name: Name }
#[derive(Debug, Clone, Copy)] enum FileType { Trombi, Labels }

fn bytes_to_jpeg(bytes: &[u8]) -> Jpeg { Jpeg::from_bytes(bytes.to_owned().into()).unwrap() }
fn write_jpeg(jpeg: Jpeg, sink: &mut impl Write) { jpeg.encoder().write_to(sink).unwrap(); }
fn read_jpeg(path: impl AsRef<Path>) -> Jpeg { bytes_to_jpeg(&std::fs::read(&path).unwrap()) }

const OUR_MARKER: u8 = jpeg::markers::APP14;
const OUR_LABEL: &str = "trombinoscope";

impl Cropped {
    fn new(path: impl AsRef<Path>, image: DynamicImage) -> Self {
        let (w, h) = image.dimensions();
        let basename = path.as_ref().file_name().unwrap();
        let (given, family) = tromb::util::filename_to_given_family(basename).unwrap();
        Self {
            path: path.as_ref().into(),
            image: image.clone(),
            given,
            family,
            x: w as i32 / 2,
            y: h as i32 / 2,
            w: w as i32 / 5,
            r: (5, 4),
            rotate: 0,
            rotated_cache: image,
        }
    }

    fn set_metadata(&mut self, Metadata { given, family, x, y, w, rotate }: Metadata) {
        self.given  = given;
        self.family = family;
        self.x = x;
        self.y = y;
        self.w = w;
        self.set_rotation(rotate);
    }

    fn set_rotation(&mut self, rotation: i8) {
        self.rotate = rotation;
        let unrotated = &self.image;
        self.rotated_cache = match rotation {
            0 => unrotated.clone(),
            1 => unrotated.rotate90(),
            2 => unrotated.rotate180(),
            3 => unrotated.rotate270(),
            _ => unreachable!(),

        }
    }

    pub fn load(path: impl AsRef<Path>, strip_old_metadata: bool) -> Option<Cropped> {
        let start = Instant::now();
        let image = image::open(&path).ok()?;
        let elapsed = start.elapsed();
        println!("Loaded {path} in {elapsed:.0?}", path = path.as_ref().display());

        let mut new = Self::new(&path, image);
        let mut jpeg = read_jpeg(&path);
        if strip_old_metadata { jpeg.remove_segments_by_marker(OUR_MARKER) }

        // TODO, use OUR_LABEL to avoid collisions with other apps using OUR_MARKER
        let metadata = jpeg
            .segment_by_marker(OUR_MARKER)
            .map(|seg| {
                let c = seg.contents().to_vec();
                bitcode::decode(&c).unwrap()
            });
        if let Some(metadata) = metadata {
            new.set_metadata(metadata);
        };
        Some(new)
    }

    fn save_metadata(&self) {
        let mut jpeg = read_jpeg(&self.path);
        let all_segments = jpeg.segments_mut();
        let new_segment = self.make_metadata_segment();
        if let Some(segment) = all_segments.iter_mut().find(|seg| seg.marker() == OUR_MARKER) {
            *segment = new_segment;
        } else {
            let new_pos = all_segments.len() - 1;
            all_segments.insert(new_pos, new_segment);
        };
        let file = &mut File::create(&self.path).unwrap();
        write_jpeg(jpeg, file);
    }

    fn make_metadata_segment(&self) -> JpegSegment {
        let &Self { x, y, w, rotate, .. } = self;
        let metadata = Metadata {
            given : self.given .clone(),
            family: self.family.clone(),
            x, y, w,
            rotate,
        };
        let metadata = bitcode::encode(&metadata);
        JpegSegment::new_with_contents(
            OUR_MARKER,
            img_parts::Bytes::copy_from_slice(&metadata)
        )
    }

    fn get(&self) -> DynamicImage {
        let &Self { x, y, w, .. } = self;
        let h = self.h();
        self.rotated_cache.crop_imm((x-w/2) as u32, (y-h/2) as u32, w as u32, h as u32)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), image::ImageError> {
        self.get().save(&path)
    }

    fn h(&self) -> i32 { let (hh, ww) = self.r; self.w * hh / ww }
    fn within_limits(&self, x: i32, y: i32, w: i32) -> bool {
        // TODO sometimes crashes get through these checksi
        let h = self.h();
        x - w / 2 > 0              &&
        y - h / 2 > 0              &&
        x + w / 2 < self.max_w()   &&
        y + h / 2 < self.max_h()   &&
        w > 0
    }
    fn xxx(&mut self, x: i32, y: i32, w: i32) { if self.within_limits(x, y, w) { self.x = x; self.y = y; self.w = w } }

    fn up      (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y+n, w  ) }
    fn down    (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y-n, w  ) }
    fn left    (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x+n, y  , w  ) }
    fn right   (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x-n, y  , w  ) }
    fn zoom_in (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y  , w-n) }
    fn zoom_out(&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y  , w+n) }
    fn max_h(&self) -> i32 { self.image.height() as i32 }
    fn max_w(&self) -> i32 { self.image.width () as i32 }
    fn rot_r(&mut self) { self.set_rotation(dbg!((self.rotate + 1).rem_euclid(4))); }
    fn rot_l(&mut self) { self.set_rotation(dbg!((self.rotate - 1).rem_euclid(4))); }
}

fn crop_interactively(
    faces: &mut [Cropped],
    window: &show_image::WindowProxy,
    dirs: &Dirs,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut face_n = 0;
    macro_rules! show { () => { window.set_image("label", faces[face_n].get()).unwrap(); }; }
    show!();
    for event in window.event_channel()? {
        //println!("{:#?}", event);
        if let event::WindowEvent::KeyboardInput(event) = event {
            use event::VirtualKeyCode::*;
            use show_image::event::KeyboardInput  as KI;
            use show_image::event::ModifiersState as MS;
            use show_image::event::ElementState   as ES;
            let KI { scan_code: _, key_code: _, state, modifiers  } = event.input;
            if state != ES::Pressed { continue; }
            let mut step_size = 10;
            if modifiers.contains(MS::CTRL ) { step_size /= 10; }
            if modifiers.contains(MS::SHIFT) { step_size *=  5; }
            // match event.input {
            //     KI { key_code: Some(Escape), modifiers: MS::SHIFT.. } => {  },
            //     _ => {},
            // }
            macro_rules! limit {
                ($method:ident) => {
                    let face = &mut faces[face_n];
                    face.$method(step_size);
                    dbg!(face.rotate);
                    window.set_image("label", face.get()).unwrap();
                };
            }
            if let Some(code) = event.input.key_code {
                match code {
                    Escape => if event.input.state.is_pressed() { break },
                    Up    =>  { limit!(up      ); }
                    Down  =>  { limit!(down    ); }
                    Left  =>  { limit!(left    ); }
                    Right =>  { limit!(right   ); }
                    P     =>  { limit!(zoom_out); }
                    G     =>  { limit!(zoom_in ); }
                    S     =>  { save_and_regenerate(faces, dirs) }
                    R     =>  { faces[face_n].rot_r(); window.set_image("label", faces[face_n].get()).unwrap()  }
                    L     =>  { faces[face_n].rot_l(); window.set_image("label", faces[face_n].get()).unwrap()  }
                    Back  =>  { face_n = face_n.saturating_sub(1);             show!(); }
                    Space =>  { face_n = (face_n + 1).clamp(0, faces.len()-1); show!(); }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn save_and_regenerate(faces: &[Cropped], dirs: &Dirs) {
    save_crop_metadata(faces);
    ensure_empty_dir(&dirs.work).unwrap();
    ensure_empty_dir(&dirs.render).unwrap();
    write_cropped_images(&faces, &dirs.work);
    trombinoscope(&dirs);
}

fn trombinoscope(dir: &Dirs) {
    let items = find_jpgs_in_dir(&dir.work)
        .iter()
        .filter_map(path_to_item)
        .collect::<Vec<_>>();

    let mut items = items.to_vec();
    items.sort_by(family_given);

    use FileType::*;
    render(trombi_typst_src(&items, dir), &dir, Trombi);
    render(labels_typst_src(&items, dir), &dir, Labels);

    unix_rm_rf(&dir.render).unwrap();
    unix_mv(&dir.work, &dir.render).unwrap();

    fs::copy(
        dbg!(trombi_file_for_dir(&dir.render, &dir.class_name(), Trombi)),
        dbg!(trombi_file_for_dir(&dir.class , &dir.class_name(), Trombi)),
    ).unwrap();

    fs::copy(
        dbg!(trombi_file_for_dir(&dir.render, &dir.class_name(), Labels)),
        dbg!(trombi_file_for_dir(&dir.class , &dir.class_name(), Labels)),
    ).unwrap();

}

fn path_to_item(image_path: impl AsRef<Path>) -> Option<Item> {
    let basename = image_path.as_ref().file_name()?;
    let (given, family) = tromb::util::filename_to_given_family(&image_path)?;
    Some( Item {
        image: basename.into(),
        name: Name { given, family }
    })
}

fn family_given(l: &Item, r: &Item) -> Ordering {
    use std::cmp::Ordering::*;
    let (Item { name: l, .. }, Item { name: r, .. }) = (l,r);
    match l.family.to_uppercase().cmp(&r.family.to_uppercase()) {
        Equal => l.given.to_uppercase().cmp(&r.given.to_uppercase()),
        different => different,
    }
}

fn save_crop_metadata(faces: &[Cropped]) {
    let start_all = Instant::now();
    for face in faces {
        let start = Instant::now();
        face.save_metadata();
        println!("Embedded metadata in {} in {:.0?}",
                 face.path.display(),
                 start.elapsed(),
        );
    }
    println!("Saving metadata took {:.0?}", start_all.elapsed());
}

pub fn write_cropped_images(faces: &[Cropped], dir: impl AsRef<Path>) {
    std::fs::create_dir_all(&dir).unwrap();
    for face in faces {
        //let filename = format!("{} @ {}.jpg", dbg!(&face.given), dbg!(&face.family));
        let filename = face.path.file_name().unwrap().to_string_lossy();
        let path = dir.as_ref().join(&*filename);
        let file = &mut File::create(path).unwrap();
        let mut encoder = JpegEncoder::new(file);
        let image_bytes = face.get().as_bytes().to_owned();
        encoder.encode(&image_bytes, face.w as u32, face.h() as u32, image::ExtendedColorType::Rgb8).unwrap();
    }
}


fn find_jpgs_in_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|res| res.map(|e| e.path()).unwrap())
        .filter(|x| is_jpg(x)) // WTF: eta conversion leads to filter not implementing Iterator!
        .collect()
}

fn is_jpg(path: impl AsRef<Path>) -> bool {
    if let Some(ref extension) = path.as_ref().extension() {
        ["jpg", "jpeg", "JPG", "JPEG"]
            .iter()
            .map(OsStr::new)
            .collect::<Vec<_>>()
            .contains(extension)
    } else {
        false
    }
}

fn class_from_dir(dir: impl AsRef<Path>) -> String {
    let std::path::Component::Normal(class) = dir.as_ref().components().last().unwrap()
        else { panic!("Last component of `{dir}` cannot be interpreted as a class name", dir = dir.as_ref().display()) };

    class.to_str().unwrap().into()
}

fn trombi_file_for_dir(dir: impl AsRef<Path>, class_name: &str, ftype: FileType) -> PathBuf {
    use FileType::*;
    dir.as_ref().join(match ftype {
        Trombi => format!("trombinoscope_{class_name}.pdf"),
        Labels => format!("étiquettes_{class_name}.pdf"),
    })
}

fn unix_mv(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    std::process::Command::new("mv")
        .arg(from.as_ref().as_os_str())
        .arg(  to.as_ref().as_os_str())
        .output()?;
    Ok(())
}

fn unix_rm_rf(path: impl AsRef<Path>) -> io::Result<()> {
    std::process::Command::new("rm")
        .arg(path.as_ref().as_os_str())
        .arg("-rf")
        .output()?;
    Ok(())
}

fn ensure_empty_dir(dir: impl AsRef<Path>) -> std::io::Result<()> {
    let dir = dbg!(dir.as_ref().as_os_str());
    std::process::Command::new("rm")   .arg("-rf").arg(dir).output()?;
    std::process::Command::new("mkdir").arg("-p" ).arg(dir).output()?;
    Ok(())
}
