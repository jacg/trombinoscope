use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Instant
};

use show_image::event;

use clap::Parser;
use show_image::create_window;


use face::{Cropped, write_cropped_images, save_crop_metadata};
use render::trombinoscope;
use util::{Dirs, ensure_empty_dir};

#[derive(Parser)]
struct Cli {
    /// Directory containing the class assets
    class_dir: PathBuf,

    #[arg(long)]
    strip_metadata: bool,
}

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

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

    let dirs = Dirs::new(cli.class_dir);

    let start = Instant::now();
    let mut faces = std::fs::read_dir(&dirs.photo)?
        .take(100)
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .filter_map(|path| Cropped::load(path, cli.strip_metadata))
        .collect::<Vec<_>>();
    println!("Loading all images took {:.1?}", start.elapsed());

    let window = create_window("image", Default::default())?;
    crop_interactively(&mut faces, &window, &dirs).unwrap();
    save_and_regenerate(&faces, &dirs);
    Ok(())
}

fn write_cropped(in_file: impl AsRef<Path>, out_dir: impl AsRef<Path>) {
    let cropped = Cropped::load(in_file, false).unwrap();
    let out_file = out_dir.as_ref().join(cropped.path.file_name().unwrap());
    println!("Writing {}", out_file.display());
    cropped.write(out_file).unwrap();
}




fn crop_interactively(
    faces: &mut [Cropped],
    window: &show_image::WindowProxy,
    dirs: &Dirs,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut face_n = 0;
    macro_rules! show { () => { window.set_image("label", faces[face_n].get()).unwrap(); }; }
    show!();
    for event in window.event_channel()? {
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
            // match event.input {
            //     KI { key_code: Some(Escape), modifiers: MS::SHIFT.. } => {  },
            //     _ => {},
            // }
            macro_rules! limit {
                ($method:ident) => {
                    let face = &mut faces[face_n];
                    face.$method(step_size);
                    dbg!(face.rotate);
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
                    S     =>  { save_and_regenerate(faces, dirs) }
                    R     =>  { faces[face_n].rot_r(); window.set_image("label", faces[face_n].get()).unwrap()  }
                    L     =>  { faces[face_n].rot_l(); window.set_image("label", faces[face_n].get()).unwrap()  }
                    I     =>  { faces[face_n].flip (); window.set_image("label", faces[face_n].get()).unwrap()  }
                    Back  =>  { face_n = face_n.saturating_sub(1);             show!(); }
                    Space =>  { face_n = (face_n + 1).clamp(0, faces.len()-1); show!(); }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn save_and_regenerate(faces: &[Cropped], dirs: &Dirs) {
    save_crop_metadata(faces);
    ensure_empty_dir(&dirs.work).unwrap();
    ensure_empty_dir(&dirs.render).unwrap();
    write_cropped_images(faces, &dirs.work);
    trombinoscope(dirs);
}
