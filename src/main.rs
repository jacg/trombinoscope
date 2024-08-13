use std::fs;
use std::io::Write;

use typst::foundations::Smart;
use typst::eval::Tracer;
use classlist::TypstWrapperWorld;

#[derive(Debug, Clone)]
struct Item {
    filename: String,
    name: String,
    surname: String,
}

impl Item {
    fn new(filename: &str, name: &str, surname: &str) -> Self {
        Item { filename: filename.into(), name: name.into(), surname: surname.to_uppercase() }
    }
}

fn render_items(items: &[Option<Item>], dir: &str, classe: &str) {

    let pic  = |i: &Option<Item>| if let Some(j) = i { format!(r#"image("{n}.jpg", width: 100%), "#, n=j.filename) }                                                                                  else { "[],".into() };
    let name = |i: &Option<Item>| if let Some(i) = i { format!("[#text([{name}], stroke: none, fill: colA) #h(1mm) #text([{surname}], stroke: none, fill: colB)],", name=i.name, surname=i.surname) } else { "[],".into() };

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

    let content = format!("{header}{table})", header = header(classe));

    // Create world with content.
    let world = TypstWrapperWorld::new(dir.to_owned(), content.clone());

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer).expect("Trombinoscope Error compiling typst.");

    // Output to pdf and svg
    let pdf = typst_pdf::pdf(&document, Smart::Auto, None);
    fs::write("./output.pdf", pdf).expect("Error writing PDF.");
    println!("Created pdf: `./output.pdf`");

    let mut out = fs::File::create("hmm.typ").unwrap();
    out.write_all(content.as_bytes()).unwrap();

}

fn main() {
    let mut args = std::env::args();

    let _executable = args.next();

    let top_data_dir = if let Some(dir) = args.next() { dir }
    else { panic!("Pass the directory containing the class directories as first argument"); };

    if let Some(class) = args.next() { doit(&top_data_dir, &class) }
    else { panic!("Pass the class name as second argument"); };


}

fn doit(top_data_dir: &str, classe: &str) {

    let dir = format!("{top_data_dir}/{classe}");
    let cache_file = format!("{dir}/.cache");

    let cache_contents = std::fs::read_to_string(cache_file)
        .unwrap();

    let lines = cache_contents.lines();
    let items: Vec<Option<Item>> = lines
        .enumerate()
        .map(|(n, line)| (n, line, line.split(',').map(str::trim)) )
        .map(|(n, line, line_components)| {
            let [filename, name, surname] = line_components.collect::<Vec<_>>()[..] else {
                panic!("Wrong number of commas on line {n} of cache file: '{line}'", n=n+1)
            };
            Some(Item::new(filename, name, surname))
        })
        .chain(vec![None; 24])
        .collect();

    render_items(&items, &dir, classe);
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
