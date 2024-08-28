use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;
use show_image::create_window;

use trombinoscope::crop::{crop_interactively, write_cropped_images, Cropped};
use typst::foundations::Smart;
use typst::eval::Tracer;

use trombinoscope::typst::TypstWrapperWorld;

#[derive(Debug, Clone)      ] struct Name { given: String, family: String }
#[derive(Debug, Clone)      ] struct Item { image: PathBuf, name: Name }
#[derive(Debug, Clone, Copy)] enum FileType { Trombi, Labels }

#[derive(Parser)]
struct Cli {
    /// Directory containing the class assets
    class_dir: PathBuf,
}

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let full_photo_dir = cli.class_dir.join("Complet");
    let render_dir     = cli.class_dir.join("Recadré");

    let start = Instant::now();
    let mut faces = std::fs::read_dir(full_photo_dir)?
        .take(100)
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .filter_map(Cropped::load)
        .collect::<Vec<_>>();
    println!("Loading all images took {:.1?}", start.elapsed());

    let window = create_window("image", Default::default())?;
    crop_interactively(&mut faces, &window).unwrap();

    std::fs::create_dir_all(&render_dir).unwrap(); // Ensure it exists so next line works
    std::fs::remove_dir_all(&render_dir).unwrap(); // Remove it and its contents
    std::fs::create_dir_all(&render_dir).unwrap(); // Ensure it exists

    write_cropped_images(&faces, &render_dir);

    trombinoscope(render_dir, cli.class_dir);

    Ok(())
}

fn trombinoscope(render_dir: impl AsRef<Path>, class_dir: impl AsRef<Path>) {

    let items = find_jpgs_in_dir(&render_dir)
        .iter()
        .filter_map(path_to_item)
        .collect::<Vec<_>>();

    let mut items = items.to_vec();
    items.sort_by(family_given);

    let class_name = class_from_dir(&class_dir);
    render(trombi_typst_src(&items, &class_name) , &render_dir, &class_dir, FileType::Trombi);
    render(labels_typst_src(&items, &class_name) , &render_dir, &class_dir, FileType::Labels);
}

fn write_cropped(in_file: impl AsRef<Path>, out_dir: impl AsRef<Path>) {
    let cropped = Cropped::load(in_file).unwrap();
    let out_file = out_dir.as_ref().join(cropped.path.file_name().unwrap());
    println!("Writing {}", out_file.display());
    cropped.write(out_file).unwrap();
}

fn render(
    content: String,
    render_dir: impl AsRef<Path>,
    class_dir: impl AsRef<Path>,
    ftype: FileType,
) {
    let typst_src_filename = format!("generated-{}.typ", match ftype {
        FileType::Trombi => "tombinoscope",
        FileType::Labels => "étiquettes",
    });

    let typst_src_path = render_dir.as_ref().join(&typst_src_filename);
    let mut out = fs::File::create(&typst_src_path).unwrap();
    out.write_all(content.as_bytes()).unwrap();

    // Create world with content.
    let world = TypstWrapperWorld::new(render_dir.as_ref().display().to_string(), content.clone());

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer)
        .unwrap_or_else(|err| {
            panic!("\nError compiling typst source `{typst_src_filename}`:\n{err:?}\n")
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

    let moved_pdf_path = trombi_file_for_dir(& class_dir, ftype);
    let moved_pdf_path_display = moved_pdf_path.display();
    let msg = &format!("PDF généré: `{moved_pdf_path_display}`.");
    println!("{msg}");
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
    let (given, family) = trombinoscope::util::filename_to_given_family(&image_path)?;
    Some( Item {
        image: basename.into(),
        name: Name { given, family }
    })
}
