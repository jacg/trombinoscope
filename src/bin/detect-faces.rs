use std::env::Args;
use std::time::{Duration, Instant};

use image::{DynamicImage, GenericImageView, GrayImage};
use rustface::{Detector, FaceInfo, ImageData};
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

    let mut detector = match rustface::create_detector(options.model_path()) {
        Ok(detector) => detector,
        Err(error) => {
            println!("Failed to create detector: {}", error);
            std::process::exit(1)
        }
    };

    detector.set_min_face_size(20);
    detector.set_score_thresh(2.0);
    detector.set_pyramid_scale_factor(0.8);
    detector.set_slide_window_step(4, 4);

    let full_images = std::fs::read_dir(options.image_path())?
        .take(1)
        .filter_map(|x| x.ok())
        .map(|p| p.path())
        .inspect(|p| {dbg!(p);})
        .filter_map(|p| image::open(p).ok())
        .map(|i| i.rotate270())
        .collect::<Vec<_>>();

    let mut faces = full_images.iter()
        .map(|i| (i, detect_faces(&mut *detector, &i.to_luma8()).into_iter().map(|f| *f.bbox())))
        .flat_map(|(image, faces)| {
            faces.into_iter()
                .map(|b| {
                    Cropped {image, x: b.x() as u32, y: b.y() as u32, w: b.width(), h: b.height() }})
        })
        .collect::<Vec<_>>();

    let window = create_window("image", Default::default())?;

    for (n, face) in faces.iter_mut().enumerate() {
        face.set_aspect_ratio_5_4();
        face.zoom_out(10);
        face.down    (80);
        window.set_image("label", face.get()).unwrap();
        crop_interactively(face, &window).unwrap();

        let path = format!("cropped/face{n}.jpg");
        match face.get().save(&path) {
            Ok(_) => { println!("Saved result to {}", path)},
            Err(message) => println!("Failed to save result to a file. Reason: {}", message),
        };
    }

    Ok(())
}

struct Cropped<'i> {
    image: &'i DynamicImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Cropped<'_> {
    fn get(&self) -> DynamicImage {
        self.image.crop_imm(self.x, self.y, self.w, self.h)
    }

    fn set_aspect_ratio_5_4(&mut self) { self.h = 5 * self.w / 4; }

    fn up      (&mut self, n: u32) {                  self.y += n;                 }
    fn left    (&mut self, n: u32) {                  self.x += n;                 }
    fn down    (&mut self, n: u32) { if self.y >   n {self.y -= n;               } }
    fn right   (&mut self, n: u32) { if self.x >   n {self.x -= n;               } }
    fn shrink_h(&mut self, n: u32) { if self.h > 2*n {self.y += n; self.h -= 2*n;} }
    fn shrink_w(&mut self, n: u32) { if self.w > 2*n {self.x += n; self.w -= 2*n;} }
    fn   grow_h(&mut self, n: u32) { if self.y >   n {self.y -= n; self.h += 2*n;} }
    fn   grow_w(&mut self, n: u32) { if self.x >   n {self.x -= n; self.w += 2*n;} }
    fn zoom_in (&mut self, n: u32) { if self.h >10*n && self.w > 8*n {self.shrink_h(5*n); self.shrink_w(4*n);} }
    fn zoom_out(&mut self, n: u32) { if self.y > 5*n && self.x > 4*n {self.  grow_h(5*n); self.  grow_w(4*n);} }
}

fn crop_interactively(face: &mut Cropped<'_>, window: &show_image::WindowProxy) -> Result<(), Box<dyn std::error::Error>> {
    for event in window.event_channel()? {
        println!("{:#?}", event);
        if let event::WindowEvent::KeyboardInput(event) = event {
            use event::VirtualKeyCode::*;
            use show_image::event::KeyboardInput as KI;
            use show_image::event::ModifiersState as MS;
            let KI { scan_code, key_code, state, modifiers  } = event.input;
            let mut step_size = 1;
            if modifiers.contains(MS::SHIFT) { step_size *= 3; }
            if modifiers.contains(MS::CTRL ) { step_size *= 5; }
            // match event.input {
            //     KI { key_code: Some(Escape), modifiers: MS::SHIFT.. } => {  },
            //     _ => {},
            // }
            macro_rules! xxx {
                ($method:ident) => {
                    face.$method(step_size);
                    window.set_image("label", face.get()).unwrap();
                };
            }
            if let Some(code) = event.input.key_code {
                match code {
                    Escape => if event.input.state.is_pressed() { break },
                    Up    => { xxx!(up      ); },
                    Down  => { xxx!(down    ); },
                    Left  => { xxx!(left    ); },
                    Right => { xxx!(right   ); },
                    G     => { xxx!(zoom_out); },
                    P     => { xxx!(zoom_in ); },
                    Back => {},
                    Return => {},
                    Space => {},
                    _ => {},
                }
            }
        }
    }
    Ok(())
}

fn show_briefly(label: impl Into<String>, window: &show_image::WindowProxy, image: impl Into<Image>, duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
    window.set_image(label, image)?;
    std::thread::sleep(duration);
    Ok(())
}

fn show_until_escape(label: impl Into<String>, window: &show_image::WindowProxy, image: impl Into<Image>) -> Result<(), Box<dyn std::error::Error>> {
    window.set_image(label, image)?;
    wait_until_escape(window)?;
    Ok(())
}

fn wait_until_escape(window: &show_image::WindowProxy) -> Result<(), Box<dyn std::error::Error>> {
    for event in window.event_channel()? {
        if let event::WindowEvent::KeyboardInput(event) = event {
            if event.input.key_code == Some(event::VirtualKeyCode::Escape) && event.input.state.is_pressed() {
                break;
            }
        }
    }
    Ok(())
}

fn detect_faces(detector: &mut dyn Detector, gray: &GrayImage) -> Vec<FaceInfo> {
    let (width, height) = gray.dimensions();
    let image = ImageData::new(gray, width, height);
    let now = Instant::now();
    let faces = detector.detect(&image);
    println!(
        "Found {} faces in {} ms",
        faces.len(),
        get_millis(now.elapsed())
    );
    faces
}

fn get_millis(duration: Duration) -> u64 {
    duration.as_secs() * 1000u64 + u64::from(duration.subsec_millis())
}

struct Options {
    image_path: String,
    model_path: String,
}

impl Options {
    fn parse(args: Args) -> Result<Self, String> {
        let args: Vec<String> = args.into_iter().collect();
        if args.len() != 3 {
            return Err(format!("Usage: {} <model-path> <image-path>", args[0]));
        }

        let model_path = args[1].clone();
        let image_path = args[2].clone();

        Ok(Options {
            image_path,
            model_path,
        })
    }

    fn image_path(&self) -> &str {
        &self.image_path[..]
    }

    fn model_path(&self) -> &str {
        &self.model_path[..]
    }
}
