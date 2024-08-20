use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use typst::foundations::Smart;
use typst::eval::Tracer;

use native_dialog::{FileDialog, MessageDialog, MessageType};

use trombinoscope::TypstWrapperWorld;

#[derive(Debug, Clone)      ] struct Name { given: String, family: String }
#[derive(Debug, Clone)      ] struct Item { image: PathBuf, name: Name }
#[derive(Debug, Clone, Copy)] enum FileType { Trombi, Labels }

fn main() {

    let mut args = std::env::args();
    let _executable = args.next();

    let (class_dir, use_gui): (PathBuf, bool) = if let Some(dir) = args.next() { (dir.into(), false) }
    else {
        MessageDialog::new()
            .set_type(MessageType::Info)
            .set_title("Choix de dossier")
            .set_text("Chosissez le dossier contenant les photos de la classe dans le dialogue qui suit.")
            .show_alert()
            .unwrap();

        (FileDialog::new()
            .set_location("~/src/trombinoscope/data")
            .show_open_single_dir()
            .unwrap()
            .unwrap(),
        true)
    };

    let items = find_image_prefixes_in_dir(&class_dir)
        .iter()
        .map(|x| file_stem_to_item(x)) // Why does eta-conversion cause type error?
        .collect::<Vec<_>>();

    let mut items = items.to_vec();
    items.sort_by(family_given);

    render(trombi_typst_src(&items, &class_dir) , &class_dir, FileType::Trombi, use_gui);
    render(labels_typst_src(&items, &class_dir) , &class_dir, FileType::Labels, use_gui);
}

fn render(content: String, class_dir: impl AsRef<Path>, ftype: FileType, use_gui: bool) {
    let typst_src = format!("generated-{}.typ", match ftype {
        FileType::Trombi => "tombinoscope",
        FileType::Labels => "étiquettes",
    });

    let mut out = fs::File::create(&typst_src).unwrap();
    use std::io::Write;
    out.write_all(content.as_bytes()).unwrap();

    // Create world with content.
    let world = TypstWrapperWorld::new(class_dir.as_ref().display().to_string(), content.clone());

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer)
        .unwrap_or_else(|err| {
            panic!("\nError compiling typst source `{typst_src}`:\n{err:?}\n")
        });

    // Output to pdf
    let pdf_bytes = typst_pdf::pdf(&document, Smart::Auto, None);

    let pdf_path = trombi_file_for_dir(&class_dir, ftype);
    let pdf_path_display = pdf_path.display();

    fs::write(&pdf_path, pdf_bytes)
        .unwrap_or_else(|err| panic!("Error writing {pdf_path_display}:\n{err:?}"));

    let msg = &format!("PDF généré: `{pdf_path_display}`.");
    println!("{msg}");
    if use_gui {
        MessageDialog::new()
            .set_type(MessageType::Info)
            .set_title("PDF généré avec succès")
            .set_text(msg)
            .show_alert()
            .unwrap();
    }
}

fn labels_typst_src(items: &[Item], class_dir: impl AsRef<Path>) -> String {
    let institution = "CO Montbrillant";
    let class = class_from_dir(&class_dir);
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

#let label = curry_label([{institution}], [Classe {class}])

#table(
    columns: 2,
    align: center + horizon,

    {labels}
)"#}
}

fn trombi_typst_src(items: &[Item], class_dir: impl AsRef<Path>) -> String {
    let pic  = |i: &Item| { format!("\n    image(\"{img}.jpg\", width: 100%),", img=i.image.display()) };
    let name = |i: &Item| {
        let Name { given, family } = &i.name;
        let family = family.to_uppercase();
        format!("\n    [#text([{given}], stroke: none, fill: colG) #h(1mm) #text([{family}], stroke: none, fill: colF)],")
    };

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

    let class = class_from_dir(&class_dir);
    format!("{header}{table})", header = header(&class))
}

fn class_from_dir(dir: impl AsRef<Path>) -> String {
    let std::path::Component::Normal(class) = dir.as_ref().components().last().unwrap()
        else { panic!("Last component of `{dir}` cannot be interpreted as a class name", dir = dir.as_ref().display()) };

    class.to_str().unwrap().into()
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
    format!(r#"#set page(
  paper: "a4",
  margin: (top: 10mm, bottom: 4mm, left: 5mm, right: 5mm),
)

#let colG = rgb(150,0,0)
#let colF = rgb(0,0,150)

#align(center, text([CLASSE {classe}], size: 50pt))

#v(-8mm) // TODO find sensible way of reducing space before table

#table(
    columns: 6,
    align: center + horizon,"#)
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
