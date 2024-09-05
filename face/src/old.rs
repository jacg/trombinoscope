use std::{
    fs::File,
    path::{Path, PathBuf},
    time::Instant,
};

use image::{DynamicImage, GenericImageView, codecs::jpeg::JpegEncoder};

use util::read_jpeg;

use crate::{
    error::{Error, Result}, metadata::{self, OUR_MARKER}, FaceInImage
};


#[derive(Debug)]
pub struct Cropped {
    pub path: PathBuf,
    metadata: FaceInImage,
    image: DynamicImage,
    rotated_cache: DynamicImage,
}

impl Cropped {
    fn new(path: impl AsRef<Path>, image: DynamicImage) -> Self {
        let basename = path.as_ref().file_name().unwrap();
        let (given, family) = util::filename_to_given_family(basename).unwrap();
        let metadata = FaceInImage {
            given,
            family,
            ..FaceInImage::default_for_image(&image)
        };
        Self {
            path: path.as_ref().into(),
            image: image.clone(),
            metadata,
            rotated_cache: image,
        }
    }

    fn set_metadata(&mut self, metadata: FaceInImage) {
        self.metadata = metadata;
        self.set_rotation(self.metadata.rot);
    }

    fn set_rotation(&mut self, rotation: i8) {
        self.metadata.rot = rotation;
        let unrotated = &self.image;
        self.rotated_cache = match rotation {
            0 => unrotated.clone(),
            1 => unrotated.rotate90(),
            2 => unrotated.rotate180(),
            3 => unrotated.rotate270(),
            _ => unreachable!(),
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Option<Cropped> {
        let start = Instant::now();
        let image = image::open(&path).ok()?;
        let elapsed1 = start.elapsed();


        let start = Instant::now();
        let mut jpeg = read_jpeg(&path);
        let elapsed2 = start.elapsed();
        println!("Loaded {path} in {elapsed1:.0?} + {elapsed2:.0?}",
                 path = path.as_ref().display());

        let mut new = Self::new(&path, image);

        // TODO if strip_old_metadata
        match FaceInImage::from_jpeg(&jpeg) {
            Ok(metadata) => new.set_metadata(metadata),
            _ => {
                let (w,h) = new.image.dimensions();
                new.set_rotation(if h > w  {0} else {3});
            }
            // Err(Error::NoMetadataFound(_)) => FaceInImage::default_for_jpeg(&jpeg).unwrap(),
            // Err(Error::Bitcode(_)) => todo!(),
            // _ => todo!(),
        };

        Some(new)
    }

    fn save_metadata(&self) -> Result<()> { self.metadata.embed_in_jpeg(&self.path) }

    pub fn get(&self) -> DynamicImage {
        let FaceInImage { cx: x, cy: y, w, .. } = self.metadata;
        let h = self.h() as f32;
        self.rotated_cache.crop_imm(
            (x-w/2.0) as u32,
            (y-h/2.0) as u32,
            w as u32, h as u32
        )
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(self.get().save(&path)?)
    }

    fn h(&self) -> f32 { self.metadata.w * crate::ASPECT_RATIO }
    fn within_limits(&self, x: f32, y: f32, w: f32) -> bool {
        //let (x,y,w) = (x as i32, )
        // TODO sometimes crashes get through these checks
        let h = self.h();
        x - w / 2.0 > 0.0            &&
        y - h / 2.0 > 0.0            &&
        x + w / 2.0 < self.max_w()   &&
        y + h / 2.0 < self.max_h()   &&
        w           > 0.0
    }
    fn xxx(&mut self, x: f32, y: f32, w: f32) {
        if self.within_limits(x, y, w) {
            self.metadata.cx = x;
            self.metadata.cy = y;
            self.metadata. w = w;
        }
    }

    pub fn up      (&mut self, n: f32) { let FaceInImage {cx, cy, w, ..} = self.metadata; self.xxx(cx  , cy+n, w  ) }
    pub fn down    (&mut self, n: f32) { let FaceInImage {cx, cy, w, ..} = self.metadata; self.xxx(cx  , cy-n, w  ) }
    pub fn left    (&mut self, n: f32) { let FaceInImage {cx, cy, w, ..} = self.metadata; self.xxx(cx+n, cy  , w  ) }
    pub fn right   (&mut self, n: f32) { let FaceInImage {cx, cy, w, ..} = self.metadata; self.xxx(cx-n, cy  , w  ) }
    pub fn zoom_in (&mut self, n: f32) { let FaceInImage {cx, cy, w, ..} = self.metadata; self.xxx(cx  , cy  , w-n) }
    pub fn zoom_out(&mut self, n: f32) { let FaceInImage {cx, cy, w, ..} = self.metadata; self.xxx(cx  , cy  , w+n) }
    pub fn max_h(&self) -> f32 { self.image.height() as f32 }
    pub fn max_w(&self) -> f32 { self.image.width () as f32 }
    pub fn rot_r(&mut self) { self.set_rotation((self.metadata.rot + 1).rem_euclid(4)); }
    pub fn rot_l(&mut self) { self.set_rotation((self.metadata.rot - 1).rem_euclid(4)); }
    pub fn flip (&mut self) { self.set_rotation((self.metadata.rot + 2).rem_euclid(4)); }
}


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
        encoder.encode(
            &image_bytes,
            face.metadata.w as u32,
            face.h() as u32,
            image::ExtendedColorType::Rgb8
        ).unwrap();
    }
}
