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
        .take(100)
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
                    Cropped {image, x: b.x() as u32, y: b.y() as u32, width: b.width(), height: b.height() }})
        })
        .collect::<Vec<_>>();

    let window = create_window("image", Default::default())?;

    for (n, face) in faces.iter_mut().enumerate() {

        for _ in 0..30 {
            face.grow_h(10);
            show_briefly(format!("expand-{n:3}"), &window, face.get(), Duration::from_millis(10))?;
        }
        for _ in 0..10 {
            face.grow_w(10);
            show_briefly(format!("expand-{n:3}"), &window, face.get(), Duration::from_millis(10))?;
        }
        for _ in 0..4 {
            face.down(10);
            show_briefly(format!("expand-{n:3}"), &window, face.get(), Duration::from_millis(10))?;
        }

        // face.grow_h(300);
        // face.grow_w(300);
        // face.down  ( 40);

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
    width: u32,
    height: u32,
}

impl Cropped<'_> {
    fn get(&self) -> DynamicImage {
        self.image.crop_imm(self.x, self.y, self.width, self.height)
    }

    fn grow_h(&mut self, n: u32) { self.y -= n; self.height += 2*n; }
    fn grow_w(&mut self, n: u32) { self.x -= n; self.width  += 2*n; }
    fn up    (&mut self, n: u32) { self.y += n }
    fn down  (&mut self, n: u32) { if self.y > n {self.y -= n} }
}

fn show(label: impl Into<String>, window: &show_image::WindowProxy, image: impl Into<Image>) -> Result<(), Box<dyn std::error::Error>> {
    // let imageview = ImageView::new(ImageInfo::rgb8(image.width(), image.height()), image.as_bytes());
    // Create a window with default options and display the image.
    //let window = create_window("image", Default::default())?;
    window.set_image(label, image)?;
    wait_until_escape(window)?;
    Ok(())
}

fn show_briefly(label: impl Into<String>, window: &show_image::WindowProxy, image: impl Into<Image>, duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
    // let imageview = ImageView::new(ImageInfo::rgb8(image.width(), image.height()), image.as_bytes());
    // Create a window with default options and display the image.
    //let window = create_window("image", Default::default())?;
    window.set_image(label, image)?;
    std::thread::sleep(duration);
    Ok(())
}

fn wait_until_escape(window: &show_image::WindowProxy) -> Result<(), Box<dyn std::error::Error>> {
    // Print keyboard events until Escape is pressed, then exit.
    // If the user closes the window, the channel is closed and the loop also exits.
    for event in window.event_channel()? {
        if let event::WindowEvent::KeyboardInput(event) = event {
            //println!("{:#?}", event);
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
