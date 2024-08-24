use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;

use trombinoscope::crop::Cropped;
use typst::foundations::Smart;
use typst::eval::Tracer;
use tempfile::tempdir;
use native_dialog::{FileDialog, MessageDialog, MessageType};

use trombinoscope::typst::TypstWrapperWorld;

#[derive(Debug, Clone)      ] struct Name { given: String, family: String }
#[derive(Debug, Clone)      ] struct Item { image: PathBuf, name: Name }
#[derive(Debug, Clone, Copy)] enum FileType { Trombi, Labels }

#[derive(Parser)]
struct Cli {
    /// Directory containing the class assets
    class_dir: Option<PathBuf>,

    /// Use the photos in this subdirectory of `CLASS_DIR`
    #[arg(long)]
    photo_subdir: Option<PathBuf>,

    /// Class name, if different from `CLASS_DIR` base name
    #[arg(long)]
    class_name: Option<String>,
}

fn main() {

    let cli = Cli::parse();

    let use_gui = cli.class_dir.is_none();

    let class_dir = if let Some(class_dir) = cli.class_dir {
        class_dir
    } else {
        MessageDialog::new()
            .set_type(MessageType::Info)
            .set_title("Choix de dossier")
            .set_text("Chosissez le dossier contenant les photos de la classe dans le dialogue qui suit.")
            .show_alert()
            .unwrap();

        FileDialog::new()
            .set_location("~/Echanges/CO-Montbrillant/Prof/ZZZ Photos Eleves COMO/")
            .show_open_single_dir()
            .unwrap()
            .unwrap()
    };

    let photo_dir = if let Some(ref subdir) = cli.photo_subdir {
        class_dir.join(subdir)
    } else {
        class_dir.clone()
    };

    let render_dir = tempdir().unwrap();

    for file in find_jpgs_in_dir(photo_dir) {
        write_cropped(file, &render_dir);
    }

    let items = find_jpgs_in_dir(&render_dir)
        .iter()
        .filter_map(path_to_item)
        .collect::<Vec<_>>();

    let mut items = items.to_vec();
    items.sort_by(family_given);


    let class_name = class_from_dir(&class_dir);
    render(trombi_typst_src(&items, &class_name) , &render_dir, &class_dir, FileType::Trombi, use_gui);
    render(labels_typst_src(&items, &class_name) , &render_dir, &class_dir, FileType::Labels, use_gui);
}

fn write_cropped(in_file: impl AsRef<Path>, out_dir: impl AsRef<Path>) {
    let xxx = Cropped::load(in_file).unwrap();
    let out_file = out_dir.as_ref().join(format!("{} @ {}.jpg", xxx.given, xxx.family));
    println!("Writing {}", out_file.display());
    xxx.write(out_file).unwrap();
}

fn render(
    content: String,
    render_dir: impl AsRef<Path>,
    class_dir: impl AsRef<Path>,
    ftype: FileType,
    use_gui: bool,
) {
    let typst_src = format!("generated-{}.typ", match ftype {
        FileType::Trombi => "tombinoscope",
        FileType::Labels => "étiquettes",
    });

    let mut out = fs::File::create(&typst_src).unwrap();
    use std::io::Write;
    out.write_all(content.as_bytes()).unwrap();

    // Create world with content.
    let world = TypstWrapperWorld::new(render_dir.as_ref().display().to_string(), content.clone());

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer)
        .unwrap_or_else(|err| {
            panic!("\nError compiling typst source `{typst_src}`:\n{err:?}\n")
        });

    // Output to pdf
    let pdf_bytes = typst_pdf::pdf(&document, Smart::Auto, None);

    let pdf_path = trombi_file_for_dir(&render_dir, ftype);
    let pdf_path_display = pdf_path.display();

    fs::write(&pdf_path, pdf_bytes)
        .unwrap_or_else(|err| panic!("Error writing {pdf_path_display}:\n{err:?}"));

    fs::rename(
        trombi_file_for_dir(&render_dir, ftype),
        trombi_file_for_dir(& class_dir, ftype),
    ).unwrap();

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

fn labels_typst_src(items: &[Item], class_name: &str) -> String {
    let institution = "CO Montbrillant";

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

fn trombi_typst_src(items: &[Item], class_name: &str) -> String {
    let table_items = items
        .iter()
        .map(|Item { image, name: Name { given, family } }| {
            format!("    item([{given}], [{family}], \"{image}\")", image=image.display())
        })
        .collect::<Vec<_>>()
        .join(",\n");


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

#let item(given, family, path) = {{
    set rect(
        width: 200mm / n_columns,
        inset: 5pt,
        stroke: 0.5pt + gray,
        height: 10mm,
    )

    let given  = text(stroke: none, fill: colG,        given  )
    let family = text(stroke: none, fill: colF, upper[#family])

    stack(
        dir: ttb,
        rect(pic(path), height: 45mm, stroke: (           bottom: none)),
        rect(align(bottom, given )  , stroke: (top: none, bottom: none)),
        rect(align(top   , family)  , stroke: (top: none              )),
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
    let (Item { name: l, .. }, Item { name: r, .. }) = (l,r);
    match l.family.to_uppercase().cmp(&r.family.to_uppercase()) {
        Equal => l.given.to_uppercase().cmp(&r.given.to_uppercase()),
        different => different,
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

fn path_to_item(image_path: impl AsRef<Path>) -> Option<Item> {
    let basename = image_path.as_ref().file_name()?;
    let stem: String = Path::new(basename).file_stem()?.to_str().map(Into::into)?;
    let mut split = stem.split('@');
    Some( Item {
        image: basename.into(),
        name: Name {
            given: split.next()?.trim().into(),
            family: if let Some(name) = split.next() { name.trim() } else { "Séparer prénom du nom par un `@`" }.into(),
        }
    })
}
