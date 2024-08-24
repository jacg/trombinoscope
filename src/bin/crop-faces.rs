use std::env::Args;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use image::{DynamicImage, GenericImageView};
use img_parts::jpeg::{self, JpegSegment, Jpeg};
use show_image::{Image, create_window, event};


#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = match Options::parse(std::env::args()) {
        Ok(options) => options,
        Err(message) => {
            println!("Failed to parse program arguments: {}", message);
            std::process::exit(1)
        }
    };

    let mut faces = std::fs::read_dir(options.image_path())?
        .take(50)
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .filter_map(Cropped::load)
        .collect::<Vec<_>>();

    let window = create_window("image", Default::default())?;
    crop_interactively(&mut faces, &window).unwrap();

    for (n, face) in faces.iter_mut().enumerate() {
        let path = format!("cropped/face{n}.jpg");
        match face.get().save(&path) {
            Ok(_) => { println!("Saved result to {}", path)},
            Err(message) => println!("Failed to save result to a file. Reason: {}", message),
        };
    }

    Ok(())
}

struct Cropped {
    path: PathBuf,
    image: DynamicImage,
    x: i32,
    y: i32,
    w: i32,
    /// height to width aspect ratio
    r: (i32, i32),
}

fn report_segments(jpeg: &jpeg::Jpeg, our_marker: u8, msg: &str) {
    println!("---------- {msg} ----------");
    for (n, segment) in jpeg.segments().iter().enumerate() {
        let marker = segment.marker();
        println!("Marker {n:2} in input: {marker}");
    }
    println!("---- Contents of our segments ----");
    for segment in jpeg.segments_by_marker(our_marker) {
        let contents = std::str::from_utf8(segment.contents()).unwrap();
        println!("   {contents}");
    }
    println!("---- End of segment report ----");
}

fn bytes_to_jpeg(bytes: &[u8]) -> Jpeg { Jpeg::from_bytes(bytes.to_owned().into()).unwrap() }
fn write_jpeg(jpeg: Jpeg, sink: &mut impl Write) { jpeg.encoder().write_to(sink).unwrap(); }
fn read_jpeg(path: impl AsRef<Path>) -> Jpeg { bytes_to_jpeg(&std::fs::read(&path).unwrap()) }

fn jpeg_to_bytes(jpeg: Jpeg) -> Vec<u8> {
    let mut bytes = vec![];
    write_jpeg(jpeg, &mut bytes);
    bytes
}

const OUR_MARKER: u8 = jpeg::markers::APP14;
const OUR_LABEL: &str = "trombinoscope";

impl Cropped {
    fn load(path: impl AsRef<Path>) -> Option<Cropped> {
        let start = Instant::now();
        let image = image::open(&path).ok()?;
        let elapsed = start.elapsed();
        let image = image.rotate270();
        println!("\n\n");
        println!("Loaded {path} in {elapsed:.0?}", path = path.as_ref().display());

        let jpeg = read_jpeg(&path);

        // TODO, use OUR_LABEL to avoid collisions with other apps using OUR_MARKER
        let (x,y,w) = jpeg.segment_by_marker(OUR_MARKER)
            .map(|seg| {
                let c = seg.contents().to_vec();
                println!("Found metadata in JPEG: {c:?}");
                (
                    i32::from_le_bytes(c[0.. 4].try_into().unwrap()),
                    i32::from_le_bytes(c[4.. 8].try_into().unwrap()),
                    i32::from_le_bytes(c[8..12].try_into().unwrap()),
                )
            })
            .or_else(|| {
                println!("No metadata found");
                let (w, h) = image.dimensions();
                Some((
                    w as i32 / 2,
                    h as i32 / 5,
                    w as i32 / 5,
                ))
            })
            .unwrap();

        Some(Self {
            path: path.as_ref().to_owned(),
            image,
            x, y, w,
            r: (3,2)
        })
    }

    fn save_metadata(&self) {
        let mut jpeg = read_jpeg(&self.path);
        let all_segments = jpeg.segments_mut();
        let new_segment = self.make_metadata_segment();
        if let Some(segment) = all_segments.iter_mut().find(|seg| seg.marker() == OUR_MARKER) {
            *segment = new_segment;
        } else {
            let new_pos = all_segments.len() - 1;
            all_segments.insert(new_pos, new_segment);
        };
        let file = &mut File::create(&self.path).unwrap();
        write_jpeg(jpeg, file);
    }

    fn make_metadata_segment(&self) -> JpegSegment {
        let Self { x, y, w, .. } = self;
        let x = x.to_le_bytes();
        let y = y.to_le_bytes();
        let w = w.to_le_bytes();

        JpegSegment::new_with_contents(
            OUR_MARKER,
            img_parts::Bytes::from_iter(x.into_iter().chain(y).chain(w))
        )
    }

    fn get(&self) -> DynamicImage {
        let &Self { x, y, w, .. } = self;
        let h = self.h();
        self.image.crop_imm((x-w/2) as u32, (y-h/2) as u32, w as u32, h as u32)
    }

    fn h(&self) -> i32 { let (hh, ww) = self.r; self.w * hh / ww }
    fn within_simits(&self, x: i32, y: i32, w: i32) -> bool {
        let h = self.h();
        x - w / 2 >= 0              &&
        y - h / 2 >= 0              &&
        x + w / 2 <  self.max_w()   &&
        y + h / 2 <  self.max_h()   &&
        w > 0
    }
    fn xxx(&mut self, x: i32, y: i32, w: i32) { if self.within_simits(x, y, w) { self.x = x; self.y = y; self.w = w } }

    fn up      (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y+n, w  ) }
    fn down    (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y-n, w  ) }
    fn left    (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x+n, y  , w  ) }
    fn right   (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x-n, y  , w  ) }
    fn zoom_in (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y  , w-n) }
    fn zoom_out(&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y  , w+n) }
    fn max_h(&self) -> i32 { self.image.height() as i32 }
    fn max_w(&self) -> i32 { self.image.width () as i32 }
}

fn crop_interactively(faces: &mut [Cropped], window: &show_image::WindowProxy) -> Result<(), Box<dyn std::error::Error>> {
    let mut face_n = 0;
    macro_rules! show { () => { window.set_image("label", faces[face_n].get()).unwrap(); }; }
    show!();
    for event in window.event_channel()? {
        //println!("{:#?}", event);
        if let event::WindowEvent::KeyboardInput(event) = event {
            use event::VirtualKeyCode::*;
            use show_image::event::KeyboardInput as KI;
            use show_image::event::ModifiersState as MS;
            use show_image::event::ElementState as ES;
            let KI { scan_code: _, key_code: _, state, modifiers  } = event.input;
            if state != ES::Pressed { continue; }
            let mut step_size = 1;
            if modifiers.contains(MS::SHIFT) { step_size *= 3; }
            if modifiers.contains(MS::CTRL ) { step_size *= 5; }
            if modifiers.contains(MS::ALT  ) { step_size *= 7; }
            // match event.input {
            //     KI { key_code: Some(Escape), modifiers: MS::SHIFT.. } => {  },
            //     _ => {},
            // }
            macro_rules! xxx {
                ($method:ident) => {
                    let face = &mut faces[face_n];
                    face.$method(step_size);
                    window.set_image("label", face.get()).unwrap();
                };
            }
            if let Some(code) = event.input.key_code {
                match code {
                    Escape => if event.input.state.is_pressed() { break },
                    Up    =>  { xxx!(up      ); },
                    Down  =>  { xxx!(down    ); },
                    Left  =>  { xxx!(left    ); },
                    Right =>  { xxx!(right   ); },
                    P     =>  { xxx!(zoom_out); },
                    G     =>  { xxx!(zoom_in ); },
                    Back  => { face_n = face_n.saturating_sub(1);             show!(); },
                    Space => { face_n = (face_n + 1).clamp(0, faces.len()-1); show!(); },
                    Return => {},
                    _ => {},
                }
            }
        }
    }
    for face in faces {
        face.save_metadata();
    }
    Ok(())
}

fn _show_briefly(label: impl Into<String>, window: &show_image::WindowProxy, image: impl Into<Image>, duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
    window.set_image(label, image)?;
    std::thread::sleep(duration);
    Ok(())
}

fn _show_until_escape(label: impl Into<String>, window: &show_image::WindowProxy, image: impl Into<Image>) -> Result<(), Box<dyn std::error::Error>> {
    window.set_image(label, image)?;
    _wait_until_escape(window)?;
    Ok(())
}

fn _wait_until_escape(window: &show_image::WindowProxy) -> Result<(), Box<dyn std::error::Error>> {
    for event in window.event_channel()? {
        if let event::WindowEvent::KeyboardInput(event) = event {
            if event.input.key_code == Some(event::VirtualKeyCode::Escape) && event.input.state.is_pressed() {
                break;
            }
        }
    }
    Ok(())
}

struct Options {
    image_path: String,
}

impl Options {
    fn parse(args: Args) -> Result<Self, String> {
        let args: Vec<String> = args.into_iter().collect();
        if args.len() != 2 {
            return Err(format!("Usage: {} <model-path> <image-path>", args[0]));
        }

        let image_path = args[1].clone();

        Ok(Options {
            image_path,
        })
    }

    fn image_path(&self) -> &str {
        &self.image_path[..]
    }
}
