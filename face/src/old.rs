use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

use image::{DynamicImage, GenericImageView, codecs::jpeg::JpegEncoder};
use img_parts::jpeg::{self, JpegSegment, Jpeg};

use crate::FaceInImage;

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
    pub rot: i8,
    rotated_cache: DynamicImage,
}

const OUR_MARKER: u8 = jpeg::markers::APP14;
const OUR_LABEL: &str = "trombinoscope";

impl Cropped {
    fn new(path: impl AsRef<Path>, image: DynamicImage) -> Self {
        let (h, w) = {
            let (w, h) = image.dimensions();
            if w < h {(w,h)} else {(h,w)}
        };
        let basename = path.as_ref().file_name().unwrap();
        let (given, family) = util::filename_to_given_family(basename).unwrap();
        Self {
            path: path.as_ref().into(),
            image: image.clone(),
            given,
            family,
            x: w as i32 / 3,
            y: h as i32 / 4,
            w: w as i32 / 8,
            r: (5, 4),
            rot: 0,
            rotated_cache: image,
        }
    }

    fn set_metadata(&mut self, FaceInImage { given, family, cx: x, cy: y, w, rot }: FaceInImage) {
        self.given  = given;
        self.family = family;
        self.x = x as i32;
        self.y = y as i32;
        self.w = w as i32;
        self.set_rotation(rot);
    }

    fn set_rotation(&mut self, rotation: i8) {
        self.rot = rotation;
        let unrotated = &self.image;
        self.rotated_cache = match rotation {
            0 => unrotated.clone(),
            1 => unrotated.rotate90(),
            2 => unrotated.rotate180(),
            3 => unrotated.rotate270(),
            _ => unreachable!(),
        }
    }

    pub fn load(path: impl AsRef<Path>, strip_old_metadata: bool) -> Option<Cropped> {
        let start = Instant::now();
        let image = image::open(&path).ok()?;
        let elapsed = start.elapsed();
        println!("Loaded {path} in {elapsed:.0?}", path = path.as_ref().display());

        let mut new = Self::new(&path, image);
        let mut jpeg = read_jpeg(&path);
        if strip_old_metadata { jpeg.remove_segments_by_marker(OUR_MARKER) }

        // TODO, use OUR_LABEL to avoid collisions with other apps using OUR_MARKER
        let metadata = jpeg
            .segment_by_marker(OUR_MARKER)
            .map(|seg| {
                let c = seg.contents().to_vec();
                bitcode::decode(&c)
                    .expect("
Error in reading metadata.
JPEG file probably contains old metadata version.
Try stripping out metadata by rerunning trombinoscope with the --strip-metadata option.
\n")
            });
        if let Some(metadata) = metadata {
            new.set_metadata(metadata);
        } else {
            let (w,h) = new.image.dimensions();
            new.set_rotation( if h > w  {0} else {3});
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
        let &Self { x, y, w, rot: rotate, .. } = self;
        let metadata = FaceInImage {
            given : self.given .clone(),
            family: self.family.clone(),
            cx: x as f32, cy: y as f32, w: w as f32,
            rot: rotate,
        };
        let metadata = bitcode::encode(&metadata);
        JpegSegment::new_with_contents(
            OUR_MARKER,
            img_parts::Bytes::copy_from_slice(&metadata)
        )
    }

    pub fn get(&self) -> DynamicImage {
        let &Self { x, y, w, .. } = self;
        let h = self.h();
        self.rotated_cache.crop_imm((x-w/2) as u32, (y-h/2) as u32, w as u32, h as u32)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), image::ImageError> {
        self.get().save(&path)
    }

    fn h(&self) -> i32 { let (hh, ww) = self.r; self.w * hh / ww }
    fn within_limits(&self, x: i32, y: i32, w: i32) -> bool {
        // TODO sometimes crashes get through these checks
        let h = self.h();
        x - w / 2 > 0              &&
        y - h / 2 > 0              &&
        x + w / 2 < self.max_w()   &&
        y + h / 2 < self.max_h()   &&
        w > 0
    }
    fn xxx(&mut self, x: i32, y: i32, w: i32) { if self.within_limits(x, y, w) { self.x = x; self.y = y; self.w = w } }

    pub fn up      (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y+n, w  ) }
    pub fn down    (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y-n, w  ) }
    pub fn left    (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x+n, y  , w  ) }
    pub fn right   (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x-n, y  , w  ) }
    pub fn zoom_in (&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y  , w-n) }
    pub fn zoom_out(&mut self, n: i32) { let &mut Self {x, y, w, ..} = self; self.xxx(x  , y  , w+n) }
    pub fn max_h(&self) -> i32 { self.image.height() as i32 }
    pub fn max_w(&self) -> i32 { self.image.width () as i32 }
    pub fn rot_r(&mut self) { self.set_rotation((self.rot + 1).rem_euclid(4)); }
    pub fn rot_l(&mut self) { self.set_rotation((self.rot - 1).rem_euclid(4)); }
    pub fn flip (&mut self) { self.set_rotation((self.rot + 2).rem_euclid(4)); }
}

fn read_jpeg(path: impl AsRef<Path>) -> Jpeg { bytes_to_jpeg(&std::fs::read(&path).unwrap()) }
fn write_jpeg(jpeg: Jpeg, sink: &mut impl Write) { jpeg.encoder().write_to(sink).unwrap(); }
fn bytes_to_jpeg(bytes: &[u8]) -> Jpeg { Jpeg::from_bytes(bytes.to_owned().into()).unwrap() }


pub fn save_crop_metadata(faces: &[Cropped]) {
    let start_all = Instant::now();
    for face in faces {
        let start = Instant::now();
        face.save_metadata();
        println!("Embedded metadata in {} in {:.0?}",
                 face.path.display(),
                 start.elapsed(),
        );
    }
    println!("Saving metadata took {:.0?}", start_all.elapsed());
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
