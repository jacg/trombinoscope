use std::fs;

use typst::foundations::Smart;
use typst::eval::Tracer;
use classlist::TypstWrapperWorld;

fn main() {
    let content = r#"
#import "@preview/tablex:0.0.8": tablex, rowspanx, colspanx

#tablex(
  columns: 4,
  align: center + horizon,
  auto-vlines: false,

  // indicate the first two rows are the header
  // (in case we need to eventually
  // enable repeating the header across pages)
  header-rows: 2,

  // color the last column's cells
  // based on the written number
  map-cells: cell => {
    if cell.x == 3 and cell.y > 1 {
      cell.content = {
        let value = int(cell.content.text)
        let text-color = if value < 10 {
          red.lighten(30%)
        } else if value < 15 {
          yellow.darken(13%)
        } else {
          green
        }
        set text(text-color)
        strong(cell.content)
      }
    }
    cell
  },

  /* --- header --- */
  rowspanx(2)[*Username*], colspanx(2)[*Data*], (), rowspanx(2)[*Score*],
  (),                 [*Location*], [*Height*], (),
  /* -------------- */

  [John], [Second St.], [180 cm], [5],
  [Wally], [Third Av.], [160 cm], [10],
  [Jason], [Some St.], [150 cm], [15],
  [Robert], [123 Av.], [190 cm], [20],
  [Other], [Unknown St.], [170 cm], [25],
)
"#
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
