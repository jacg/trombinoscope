use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use typst::foundations::Smart;
use typst::eval::Tracer;

use native_dialog::{FileDialog, MessageDialog, MessageType};

use trombinoscope::TypstWrapperWorld;

#[derive(Debug, Clone)] struct Name { given: String, family: String }
#[derive(Debug, Clone)] struct Item { image: PathBuf, name: Name }
#[derive(Debug, Clone)] enum FileType { Trombi, Labels }

fn main() {

    let mut args = std::env::args();
    let _executable = args.next();

    let class_dir: PathBuf = if let Some(dir) = args.next() { dir.into() }
    else {

        MessageDialog::new()
            .set_type(MessageType::Info)
            .set_title("Instructions trombinoscope")
            .set_text("Chosissez le dossier contenant les photos de la classe dans le dialogue qui suit.")
            .show_alert()
            .unwrap();


        FileDialog::new()
            .set_location("~/src/trombinoscope/data")
            .show_open_single_dir()
            .unwrap()
            .unwrap()
    };

    let items = find_image_prefixes_in_dir(&class_dir)
        .iter()
        .map(|x| file_stem_to_item(x)) // Why does eta-conversion cause type error?
        .collect::<Vec<_>>();

    render_items(&items, &class_dir);
}

fn render_items(items: &[Item], class_dir: impl AsRef<Path>) {
    let mut items = items.to_vec();
    items.sort_by(family_given);
    let pic  = |i: &Item| { format!(r#"image("{img}.jpg", width: 100%),"#, img=i.image.display()) };
    let name = |i: &Item| {
        let Name { given, family } = &i.name;
        let family = family.to_uppercase();
        format!("[#text([{given}], stroke: none, fill: colA) #h(1mm) #text([{family}], stroke: none, fill: colB)],\n") };

    let make_row = |range: std::ops::Range<usize>| {
        let len = items.len();
        if len <= range.start { ("".into(), "".into()) } else {

            let npad = if len > range.end { 0 } else { range.end - len };
            let (lo, hi) = (range.start.min(len), range.end.min(len));

            (
                items[lo..hi].iter().map(pic ).chain(vec!["[],".into(); npad]).collect::<Vec<_>>().join(""),
                items[lo..hi].iter().map(name)                                .collect::<Vec<_>>().join("")
            )
        }

    };
    let w = 6;
    let (row_1_pics, row_1_names)  = make_row(  0..w*1);
    let (row_2_pics, row_2_names)  = make_row(w*1..w*2);
    let (row_3_pics, row_3_names)  = make_row(w*2..w*3);
    let (row_4_pics, row_4_names)  = make_row(w*3..w*4);

    let table = format!(r#"
  {row_1_pics} {row_1_names} 
  {row_2_pics} {row_2_names}
  {row_3_pics} {row_3_names}
  {row_4_pics} {row_4_names}
"#);

    let std::path::Component::Normal(class) = class_dir.as_ref().components().last().unwrap()
        else { panic!("Last component of `{dir}` cannot be interpreted as a class name", dir = class_dir.as_ref().display()) };

    let class = class.to_str().unwrap();

    let content = format!("{header}{table})", header = header(class));

    // Create world with content.
    let world = TypstWrapperWorld::new(class_dir.as_ref().display().to_string(), content.clone());

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer).expect("Trombinoscope Error compiling typst.");

    // Output to pdf
    let pdf = typst_pdf::pdf(&document, Smart::Auto, None);
    let trombi_pdf = trombi_file_for_dir(&class_dir, FileType::Trombi);
    let trombi_pdf_display = trombi_pdf.display();
    fs::write(&trombi_pdf, pdf).expect("Error writing PDF.");

    MessageDialog::new()
        .set_type(MessageType::Info)
        .set_title("Trombinoscope crée avec succès")
        .set_text(&format!("Le tormbinoscope a été crée dans `{trombi_pdf_display}`."))
        .show_alert()
        .unwrap();

    println!("Created pdf: `{trombi_pdf_display}`");

    let mut out = fs::File::create("generated.typ").unwrap();
    use std::io::Write;
    out.write_all(content.as_bytes()).unwrap();

}

fn trombi_file_for_dir(dir: impl AsRef<Path>, ftype: FileType) -> PathBuf {
    use FileType::*;
    dir.as_ref().join(match ftype {
        Trombi => "trombinoscope.pdf",
        Labels => "étiquettes.pdf",
    })
}

fn family_given(l: &Item, r: &Item) -> Ordering {
    use std::cmp::Ordering::*;
    match (l,r) {
        (Item { name: l, .. }, Item { name: r, .. }) => {
            match l.family.to_uppercase().cmp(&r.family.to_uppercase()) {
                Equal => l.given.to_uppercase().cmp(&r.given.to_uppercase()),
                different => different,
            }
        }
    }
}

fn header(classe: &str) -> String {
    format!(r#"
#set page(
  paper: "a4",
  margin: (top: 10mm, bottom: 4mm, left: 5mm, right: 5mm),
)

#let colA = rgb(150,0,0)
#let colB = rgb(0,0,150)

#align(center, text([CLASSE {classe}], size: 50pt))

#v(-8mm) // TODO find sensible way of reducing space before table

#table(
  columns: 6,
  align: center + horizon,
  //stroke: none,
"#)
}

fn find_image_prefixes_in_dir(dir: impl AsRef<Path>) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|res| res.map(|e| e.path()).unwrap())
        .filter(|path| path.extension() == Some(OsStr::new("jpg")))
        .map(|path| path.file_stem().unwrap().to_owned())
        .map(|file| file.to_str().unwrap().to_owned())
        .collect()
}

fn file_stem_to_item(image: &str) -> Item {
    let mut split = image.split('@');
    Item {
        image: image.into(),
        name: Name {
            given: split.next().unwrap().trim().into(),
            family: if let Some(name) = split.next() { name.trim() } else { "Séparer prénom du nom par un `@`" }.into(),
        }
    }
}
