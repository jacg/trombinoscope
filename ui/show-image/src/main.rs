use std::{
    io::Write,
    time::Instant
};

use show_image::{create_window, event};

use util::Dirs;

mod face;

use face::{CropSi, FaceSi, ViewSi};

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::parse();

    if cli.strip_metadata {
        let mut stdout = console::Term::stdout();
        stdout.write_all(b"\n\nARE YOU SURE THAT YOU WANT TO STRIP METADATA ?  This cannot be undone!
To continue with stripped metadata, press '@'.
Otherwise press any other key and rerun the program without the `--strip-metadata option`
").unwrap();
        if stdout.read_key().unwrap() != console::Key::Char('@') {
            println!("\nNot stripping metadata. Stopping. Rerun without `--strip-metadata`.");
            std::process::exit(0);
        }
    }
    let window = create_window("image", Default::default())?;

    let dirs = Dirs::new(cli.class_dir);

    let start = Instant::now();
    let mut faces = std::fs::read_dir(&dirs.photo)?
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .filter_map(|path| {
            image::open(&path)
                .ok()
                .and_then(|image| FaceSi::new(path, CropSi {
                    rotated_image: image,
                    window: &window,
                    rot: 0
                }).ok())
            // TODO cli.strip_metadata
        })
        .collect::<Vec<_>>();
    println!("Loading all images took {:.1?}", start.elapsed());

    crop_interactively(&mut faces, &window, &dirs).unwrap();
    save_and_regenerate(&faces, &dirs);
    Ok(())
}

fn crop_interactively(
    faces: &mut [FaceSi],
    window: &show_image::WindowProxy,
    dirs: &Dirs,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut face_n = 0;
    let view = ViewSi {  };

    macro_rules! show { () => { faces[face_n].view(view).unwrap() }; }
    show!();

    for event in window.event_channel()? {
        let face = &mut faces[face_n];
        face.view(view).unwrap();
        //println!("{:#?}", event);
        if let event::WindowEvent::KeyboardInput(event) = event {
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
                    Down  => { face.move_down ( step_size); }
                    Up    => { face.move_down (-step_size); }
                    Right => { face.move_right( step_size); }
                    Left  => { face.move_right(-step_size); }
                    G     => { face.zoom_in   ( step_size); }
                    P     => { face.zoom_in   (-step_size); }
                    R     => { face.rotate    ( 1        ); }
                    L     => { face.rotate    (-1        ); }
                    // S     =>  { save_and_regenerate(faces, dirs) }
                    Back  =>  { face_n = face_n.saturating_sub(1);             show!(); }
                    Space =>  { face_n = (face_n + 1).clamp(0, faces.len()-1); show!(); }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn save_and_regenerate(faces: &[FaceSi], dirs: &Dirs) {
    todo!()
    // save_crop_metadata(faces);
    // ensure_empty_dir(&dirs.work).unwrap();
    // ensure_empty_dir(&dirs.render).unwrap();
    // write_cropped_images(faces, &dirs.work);
    // trombinoscope(dirs);
}
