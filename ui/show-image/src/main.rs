use std::time::Instant;

use show_image::{create_window, event};

use util::{Dirs, ensure_empty_dir, find_jpgs_in_dir};
use render::trombinoscope;
use ::face::{save_face_metadata, write_many_face_images};

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
    faces[face_n].view(&view).unwrap();

    for event in view.window.event_channel()? {
        let face = &mut faces[face_n];
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
                let changed = match code {
                    Escape if event.input.state.is_pressed() => { break },
                    Down  => { face.move_down ( step_size).is_ok() }
                    Up    => { face.move_down (-step_size).is_ok() }
                    Right => { face.move_right( step_size).is_ok() }
                    Left  => { face.move_right(-step_size).is_ok() }
                    G     => { face.zoom_in   ( step_size).is_ok() }
                    P     => { face.zoom_in   (-step_size).is_ok() }
                    R     => { face.rotate    ( 1        ).is_ok() }
                    L     => { face.rotate    (-1        ).is_ok() }
                    S     => { save_and_regenerate(&faces, &dirs).unwrap(); false }
                    Back  => { face_n = face_n.saturating_sub(1);             true }
                    Space => { face_n = (face_n + 1).clamp(0, faces.len()-1); true }
                    _ => { false}
                };
                if changed { faces[face_n].view(&view).unwrap(); }
            };
        }
    }

    save_and_regenerate(&faces, &dirs)?;
    Ok(())
}

fn save_and_regenerate(faces: &[SiFaceType], dirs: &Dirs) -> ::face::Result<()> {
    save_face_metadata(faces)?;
    ensure_empty_dir(&dirs.work)?;
    ensure_empty_dir(&dirs.render)?;
    write_many_face_images(faces, &dirs.work)?;
    trombinoscope(dirs);
    Ok(())
}
