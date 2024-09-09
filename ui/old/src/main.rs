use std::time::Instant;

use show_image::{create_window, event};

use face::old::{Cropped, write_cropped_images, save_crop_metadata};
use render::trombinoscope;
use util::{Dirs, ensure_empty_dir, find_jpgs_in_dir};

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::parse();
    if cli.strip_metadata { panic!("--strip-metadata temporarily unavailable"); }
    if cli.strip_metadata { util::strip_metadata(); }
    let dirs = Dirs::new(cli.class_dir);

    let start = Instant::now();
    let mut faces = find_jpgs_in_dir(&dirs.photo).into_iter()
        .filter_map(Cropped::load)
        .collect::<Vec<_>>();
    println!("Loading all images took {:.1?}", start.elapsed());

    let mut face_n = 0;
    let window = create_window("image", Default::default())?;

    macro_rules! show { () => { window.set_image("label", faces[face_n].get()).unwrap(); }; }
    show!();

    for event in window.event_channel()? {
        let start = std::time::Instant::now();
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
            macro_rules! limit {
                ($method:ident) => {
                    let face = &mut faces[face_n];
                    face.$method(step_size as f32);
                    window.set_image("label", face.get()).unwrap();
                };
            }
            if let Some(code) = event.input.key_code {
                match code {
                    Escape => if event.input.state.is_pressed() { break },
                    Up    =>  { limit!(up      ); }
                    Down  =>  { limit!(down    ); }
                    Left  =>  { limit!(left    ); }
                    Right =>  { limit!(right   ); }
                    P     =>  { limit!(zoom_out); }
                    G     =>  { limit!(zoom_in ); }
                    S     =>  { save_and_regenerate(&faces, &dirs) }
                    R     =>  { faces[face_n].rot_r(); window.set_image("label", faces[face_n].get()).unwrap()  }
                    L     =>  { faces[face_n].rot_l(); window.set_image("label", faces[face_n].get()).unwrap()  }
                    I     =>  { faces[face_n].flip (); window.set_image("label", faces[face_n].get()).unwrap()  }
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

fn save_and_regenerate(faces: &[Cropped], dirs: &Dirs) {
    save_crop_metadata(faces);
    ensure_empty_dir(&dirs.work).unwrap();
    ensure_empty_dir(&dirs.render).unwrap();
    write_cropped_images(faces, &dirs.work);
    trombinoscope(dirs);
}
