use std::fs;

use typst::foundations::Smart;
use typst::eval::Tracer;
use classlist::TypstWrapperWorld;

fn main() {
    let content = r#"
#import "@preview/polylux:0.3.1": *
#import themes.simple: *

#set page(paper: "presentation-16-9")

#show: simple-theme.with()

#title-slide[
= Hello, World!
A document (+ `polylux` library) rendered with `Typst`!
]"#
    .to_owned();

    // Create world with content.
    let world = TypstWrapperWorld::new("./".to_owned(), content);

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer).expect("Error compiling typst.");

    // Output to pdf and svg
    let pdf = typst_pdf::pdf(&document, Smart::Auto, None);
    fs::write("./output.pdf", pdf).expect("Error writing PDF.");
    println!("Created pdf: `./output.pdf`");

}
