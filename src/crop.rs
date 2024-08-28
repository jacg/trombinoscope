use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::Write;
use std::time::Instant;

use image::{DynamicImage, GenericImageView, codecs::jpeg::JpegEncoder};
use img_parts::jpeg::{self, JpegSegment, Jpeg};
use show_image::event;
use bitcode::{self, Encode, Decode};

use crate::util::filename_to_given_family;


#[derive(Encode, Decode, PartialEq, Debug)]
struct Metadata {
    given: String,
    family: String,
    x: i32,
    y: i32,
    w: i32,
}

#[derive(Debug)]
pub struct Cropped {
    pub path: PathBuf,
    image: DynamicImage,
    pub given: String,
    pub family: String,
    x: i32,
    y: i32,
    w: i32,
    /// height to width aspect ratio
    r: (i32, i32),
}

fn bytes_to_jpeg(bytes: &[u8]) -> Jpeg { Jpeg::from_bytes(bytes.to_owned().into()).unwrap() }
fn write_jpeg(jpeg: Jpeg, sink: &mut impl Write) { jpeg.encoder().write_to(sink).unwrap(); }
fn read_jpeg(path: impl AsRef<Path>) -> Jpeg { bytes_to_jpeg(&std::fs::read(&path).unwrap()) }

const OUR_MARKER: u8 = jpeg::markers::APP14;
const OUR_LABEL: &str = "trombinoscope";

impl Cropped {
    fn new(path: impl AsRef<Path>, image: DynamicImage) -> Self {
        let (w, h) = image.dimensions();
        let basename = path.as_ref().file_name().unwrap();
        let (given, family) = filename_to_given_family(basename).unwrap();
        Self {
            path: path.as_ref().into(),
            image,
            given,
            family,
            x: w as i32 / 2,
            y: h as i32 / 5,
            w: w as i32 / 5,
            r: (3, 2),
        }
    }

    fn set_metadata(&mut self, Metadata { given, family, x, y, w }: Metadata) {
        self.given  = given;
        self.family = family;
        self.x = x;
        self.y = y;
        self.w = w;
    }

    pub fn load(path: impl AsRef<Path>) -> Option<Cropped> {
        let start = Instant::now();
        let image = image::open(&path).ok()?;
        let elapsed = start.elapsed();
        let image = image.rotate270();
        println!("Loaded {path} in {elapsed:.0?}", path = path.as_ref().display());

        let mut new = Self::new(&path, image);
        let jpeg = read_jpeg(&path);

        // TODO, use OUR_LABEL to avoid collisions with other apps using OUR_MARKER
        let metadata = jpeg
            .segment_by_marker(OUR_MARKER)
            .map(|seg| {
                let c = seg.contents().to_vec();
                bitcode::decode(&c).unwrap()
            });
        if let Some(metadata) = metadata {
            new.set_metadata(metadata);
        };
        Some(new)
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
        let &Self { x, y, w, .. } = self;
        let metadata = Metadata {
            given : self.given .clone(),
            family: self.family.clone(),
            x, y, w
        };
        let metadata = bitcode::encode(&metadata);
        JpegSegment::new_with_contents(
            OUR_MARKER,
            img_parts::Bytes::copy_from_slice(&metadata)
        )
    }

    fn get(&self) -> DynamicImage {
        let &Self { x, y, w, .. } = self;
        let h = self.h();
        self.image.crop_imm((x-w/2) as u32, (y-h/2) as u32, w as u32, h as u32)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), image::ImageError> {
        self.get().save(&path)
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

pub fn crop_interactively(faces: &mut [Cropped], window: &show_image::WindowProxy) -> Result<(), Box<dyn std::error::Error>> {
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
            let mut step_size = 10;
            if modifiers.contains(MS::CTRL ) { step_size /= 10; }
            if modifiers.contains(MS::SHIFT) { step_size *=  5; }
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
                    Back  =>  { face_n = face_n.saturating_sub(1);             show!(); },
                    Space =>  { face_n = (face_n + 1).clamp(0, faces.len()-1); show!(); },
                    _ => {},
                }
            }
        }
    }
    for face in faces {
        println!("Embedding metadata in {}", face.path.display());
        face.save_metadata();
    }
    Ok(())
}

pub fn write_cropped_images(faces: &[Cropped], dir: impl AsRef<Path>) {
    std::fs::create_dir_all(&dir).unwrap();
    for face in faces {
        //let filename = format!("{} @ {}.jpg", dbg!(&face.given), dbg!(&face.family));
        let filename = face.path.file_name().unwrap().to_string_lossy();
        let path = dir.as_ref().join(&*filename);
        let file = &mut File::create(path).unwrap();
        let mut encoder = JpegEncoder::new(file);
        let image_bytes = face.get().as_bytes().to_owned();
        encoder.encode(&image_bytes, face.w as u32, face.h() as u32, image::ExtendedColorType::Rgb8).unwrap();
    }
}
