use std::{
    fs::File,
    io::Write,
    path::Path,
};

use bitcode::{Decode, Encode};
use img_parts::jpeg::{self, Jpeg, JpegSegment};
use image::codecs::jpeg::JpegEncoder;

use render::trombinoscope;
use util::{Dirs, ensure_empty_dir, read_jpeg, write_jpeg};

use crate::{
    error::{Error, Result},
    ui,
    FaceType, ASPECT_RATIO,
};

/// The information needed to label and locate a face inside a photograph
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct FaceInImage {
    pub given: String,
    pub family: String,
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub rot: i8,
}

impl FaceInImage {

    /// Construct default guess of face description for image with given width
    /// and height
    pub fn default_for(width: f32, height: f32) -> Self {
        let (h, w, rot) = {
            let (w, h) = (width, height);
            if w < h {(w, h, 0)} else {(h, w, 3)}
        };
        Self {
            given:  String::default(),
            family: String::default(),
            cx: w / 3.0,
            cy: h / 4.0,
            w:  w / 8.0,
            rot,
        }
    }

    /// Load image at `path`. If image contains face metadata, use it; otherwise
    /// use `with` and `height` to guess where the face is.
    pub fn from_path_or_default_for(path: impl AsRef<Path>, width: f32, height: f32) -> Result<Self> {
        Ok(match FaceInImage::from_jpeg_in_file(&path) {
            Ok(face) => face,
            Err(Error::NoMetadataFound(_)) => FaceInImage::default_for(width, height),
            err => err?,
        })
    }

    pub fn from_jpeg_with_message(jpeg: &Jpeg, msg: &str) -> Result<Self> {
        jpeg
            .segment_by_marker(OUR_MARKER) // TODO, use OUR_LABEL to avoid collisions with other apps using OUR_MARKER
            .map_or_else(
                || Err(Error::NoMetadataFound(msg.into())),
                Self::from_jpeg_segment)
    }

    /// Construct from information encoded in in-memory JPEG parts
    pub fn from_jpeg(jpeg: &Jpeg) -> Result<Self> {
        Self::from_jpeg_with_message(jpeg, "In-memory JPEG")
    }

    /// Construct from information encoded in JPEG segment in the given file
    pub fn from_jpeg_in_file(path: impl AsRef<Path>) -> Result<Self> {
        // TODO make read_jpeg return Result
        Self::from_jpeg_with_message(&read_jpeg(&path), &path.as_ref().to_string_lossy())
    }

    /// Encode `self` as a JPEG metadata segment
    pub fn as_jpeg_segment(&self) -> JpegSegment {
        let encoded = bitcode::encode(self);
        JpegSegment::new_with_contents(
            OUR_MARKER,
            img_parts::Bytes::copy_from_slice(&encoded)
        )
    }

    /// Construct from information encoded in JPEG segment
    pub fn from_jpeg_segment(segment: &JpegSegment) -> Result<Self> {
        Ok(bitcode::decode(segment.contents())?)
    }

    /// Inject `self` as a metadata segment into existing JPEG file, overwriting
    /// old segment if present.
    pub fn embed_in_jpeg(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut jpeg = read_jpeg(&path);
        let all_segments = jpeg.segments_mut();
        let new_segment = self.as_jpeg_segment();
        if let Some(segment) = all_segments.iter_mut().find(|seg| seg.marker() == OUR_MARKER) {
            *segment = new_segment;
        } else {
            let new_pos = all_segments.len() - 1; // Hack around https://github.com/paolobarbolini/img-parts/issues/12
            all_segments.insert(new_pos, new_segment);
        };
        let file = &mut File::create(path).unwrap();
        write_jpeg(jpeg, file);
        Ok(())
    }

    pub fn strip_from_jpeg(path: impl AsRef<Path>) -> Result<()> {
        let mut jpeg = read_jpeg(&path); // TODO make read_jpeg return Result
        jpeg.remove_segments_by_marker(OUR_MARKER);
        let file = &mut File::create(path).unwrap();
        write_jpeg(jpeg, file);
        Ok(())
    }

    pub fn h(&self) -> f32 { self.w * ASPECT_RATIO }
}


/// INCOMPLETE
pub fn strip_from_jpgs_in_dir(dir: impl AsRef<Path>) -> Result<()> {
    let mut stdout = console::Term::stdout();
    stdout.write_all(b"\n\nARE YOU SURE THAT YOU WANT TO STRIP METADATA ?  This cannot be undone!
To continue with stripped metadata, press '@'.
Otherwise press any other key and rerun the program without the `--strip-metadata option`
").unwrap();
    if stdout.read_key().unwrap() != console::Key::Char('@') {
        println!("\nNot stripping metadata. Stopping. Rerun without `--strip-metadata`.");
        std::process::exit(0);
    } else {
        println!("STRIPPING METADATA");
        for jpg in util::find_jpgs_in_dir(dir) {
            FaceInImage::strip_from_jpeg(jpg)?;
        }
    }
    Ok(())
}

pub const OUR_MARKER: u8 = jpeg::markers::APP14;
pub const OUR_LABEL: &str = "trombinoscope";
