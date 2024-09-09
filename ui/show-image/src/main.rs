use std::time::Instant;

use show_image::{create_window, event};

use util::{Dirs, find_jpgs_in_dir};
mod face;
use face::{SiFaceType, SiFace, ViewSi};

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::parse();
    // TODO cli.strip_metadata
    if cli.strip_metadata { util::strip_metadata(); }
    let dirs = Dirs::new(cli.class_dir);

    let start = Instant::now();
    let mut faces = find_jpgs_in_dir(&dirs.photo).into_iter()
        .map(<SiFace as ::face::ui::one::Face>::load)
        .collect::<Result<Vec<_>, _>>()?;
    println!("Loading all images took {:.1?}", start.elapsed());

    let mut face_n = 0;
    let view = ViewSi { window: create_window("image", Default::default())? };

    macro_rules! show { () => { faces[face_n].view(&view).unwrap() }; }
    show!();

    for event in view.window.event_channel()? {
        let start = std::time::Instant::now();
        let face = &mut faces[face_n];
        face.view(&view).unwrap();
        //println!("{:#?}", event);
        if let event::WindowEvent::KeyboardInput(ref event) = event {
            use event::VirtualKeyCode::*;
            use show_image::event::KeyboardInput  as KI;
            use show_image::event::ModifiersState as MS;
            use show_image::event::ElementState   as ES;
            let KI { scan_code: _, key_code: _, state, modifiers  } = event.input;
            if state != ES::Pressed { continue; }
            let mut step_size = 10;
            if modifiers.contains(MS::CTRL ) { step_size /= 10; }
            if modifiers.contains(MS::SHIFT) { step_size *=  5; }
            let step_size = step_size as f32;

            if let Some(code) = event.input.key_code {
                match code {
                    Escape => if event.input.state.is_pressed() { break },
                    Down  => { let _ = face.move_down ( step_size); }
                    Up    => { let _ = face.move_down (-step_size); }
                    Right => { let _ = face.move_right( step_size); }
                    Left  => { let _ = face.move_right(-step_size); }
                    G     => { let _ = face.zoom_in   ( step_size); }
                    P     => { let _ = face.zoom_in   (-step_size); }
                    R     => { let _ = face.rotate    ( 1        ); }
                    L     => { let _ = face.rotate    (-1        ); }
                    // S     =>  { save_and_regenerate(faces, dirs) }
                    Back  =>  { face_n = face_n.saturating_sub(1);             show!(); }
                    Space =>  { face_n = (face_n + 1).clamp(0, faces.len()-1); show!(); }
                    _ => {}
                }
            }
        }
        println!("{:.0?} {event:?}", start.elapsed());
    }

    save_and_regenerate(&faces, &dirs);
    Ok(())
}

fn save_and_regenerate(faces: &[SiFaceType], dirs: &Dirs) {
    todo!()
    // save_crop_metadata(faces);
    // ensure_empty_dir(&dirs.work).unwrap();
    // ensure_empty_dir(&dirs.render).unwrap();
    // write_cropped_images(faces, &dirs.work);
    // trombinoscope(dirs);
}
