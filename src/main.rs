use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use typst::foundations::Smart;
use typst::eval::Tracer;
use trombinoscope::TypstWrapperWorld;

#[derive(Debug, Clone)] struct Name { given: String, family: String }
#[derive(Debug, Clone)] struct Item { image: String, name: Name }
#[derive(Debug       )] struct State(HashMap<String, Name>);
#[derive(Debug, Clone)] enum FileType { Cache, Trombi, Labels }

impl State {
    fn iter_items(&self) -> impl Iterator<Item = Item> + '_ {
        self
            .0
            .iter()
            .map(|(image, name)| Item { image: image.clone(), name: name.clone() } )

    }
}

fn render_items(items: &[Option<Item>], class_data_dir: impl AsRef<Path>) {

    let pic  = |i: &Option<Item>| if let Some(j) = i { format!(r#"image("{img}.jpg", width: 100%), "#, img=j.image) } else { "[],".into() };
    let name = |i: &Option<Item>| if let Some(i) = i {
        let Name { given, family } = &i.name;
        let family = family.to_uppercase();
        format!("[#text([{given}], stroke: none, fill: colA) #h(1mm) #text([{family}], stroke: none, fill: colB)],") }
    else { "[],".into() };

    macro_rules! make_row {
        ($bounds:expr) => {(
            items[$bounds].iter().map(pic ).collect::<Vec<_>>().join(""),
            items[$bounds].iter().map(name).collect::<Vec<_>>().join("")
        )};
    }
    let (row_1_pics, row_1_names)  = make_row![  .. 6];
    let (row_2_pics, row_2_names)  = make_row![ 6..12];
    let (row_3_pics, row_3_names)  = make_row![12..18];
    let (row_4_pics, row_4_names)  = make_row![18..24];

    let table = format!(r#"
  {row_1_pics} {row_1_names} table.hline(),
  {row_2_pics} {row_2_names} table.hline(),
  {row_3_pics} {row_3_names} table.hline(),
  {row_4_pics} {row_4_names}
"#);

    let std::path::Component::Normal(class) = class_data_dir.as_ref().components().last().unwrap()
        else { panic!("Last component of `{dir}` cannot be interpreted as a class name", dir = class_data_dir.as_ref().display()) };

    let class = class.to_str().unwrap();

    let content = format!("{header}{table})", header = header(class));

    // Create world with content.
    let world = TypstWrapperWorld::new(class_data_dir.as_ref().display().to_string(), content.clone());

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer).expect("Trombinoscope Error compiling typst.");

    // Output to pdf
    let pdf = typst_pdf::pdf(&document, Smart::Auto, None);
    let trombi_pdf = trombi_file_for_dir(&class_data_dir, FileType::Trombi);
    fs::write(&trombi_pdf, pdf).expect("Error writing PDF.");
    println!("Created pdf: `{}`", trombi_pdf.display());

    // let mut out = fs::File::create("generated.typ").unwrap();
    // out.write_all(content.as_bytes()).unwrap();

}

fn main() {
    let mut args = std::env::args();

    let _executable = args.next();

    let class_data_dir = if let Some(dir) = args.next() { dir }
    else { panic!("Pass the directory containing the class photographs as first CLI argument"); };

    let class_data_dir: PathBuf = class_data_dir.into();

    ensure_cache_file      (&class_data_dir);
    let state = read_cache_file(&class_data_dir);
    render_state(&state, &class_data_dir);
    write_cache_file(&state, &class_data_dir);
}

fn trombi_file_for_dir(dir: impl AsRef<Path>, ftype: FileType) -> PathBuf {
    use FileType::*;
    dir.as_ref().join(match ftype {
        Cache => ".trombi-cache",
        Trombi => "trombinoscope.pdf",
        Labels => "étiquettes.pdf",
    })
}

fn render_state(state: &State, dir: impl AsRef<Path>) {
    let mut items = state
        .iter_items()
        .map(Some)
        .chain(vec![None; 24])
    .collect::<Vec<_>>();

    items.sort_by(option_family_given);

    render_items(&items, &dir);
}

fn option_family_given(l: &Option<Item>, r: &Option<Item>) -> Ordering {
    use std::cmp::Ordering::*;
    match (l,r) {
        (None, None) => Equal,
        (None, Some(_)) => Greater,
        (Some(_), None) => Less,
        (Some(Item { name: l, .. }), Some(Item { name: r, .. })) => {
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

fn ensure_cache_file(dir: impl AsRef<Path>) {
    let cache_file = trombi_file_for_dir(&dir, FileType::Cache);
    if ! cache_file.exists() {
        let new_cache_file_state = find_image_prefixes_in_dir(&dir)
            .iter()
            .map(|image|  ( image.into(), Name { given: "Prénom".into(), family: "Nom".into() }))
            .collect::<HashMap<_, _>>();
        let new_cache_file_state = State(new_cache_file_state);
        write_cache_file(&new_cache_file_state, &dir);
    }
}

fn read_cache_file(dir: impl AsRef<Path>) -> State {
    let cache_contents = std::fs::read_to_string(trombi_file_for_dir(&dir, FileType::Cache))
        .unwrap();

    let lines = cache_contents.lines();
    let items: HashMap<String, Name> = lines
        .enumerate()
        .map(|(n, line)| (n, line, line.split(',').map(str::trim)) )
        .map(|(n, line, line_components)| {
            let [filename, name, surname] = line_components.collect::<Vec<_>>()[..] else {
                panic!("Wrong number of commas on line {n} of cache file: '{line}'", n=n+1)
            };
            (filename.into(), Name { given: name.into(), family: surname.into() })
        })
        .collect();
    State(items)
}

fn write_cache_file(state: &State, dir: impl AsRef<Path>) {
    let contents = state.0
        .iter()
        .map(|(image, Name { given, family })| format!("{image}, {given}, {family}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(trombi_file_for_dir(dir, FileType::Cache), contents + "\n").unwrap();
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
