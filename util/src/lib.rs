use std::{
    fs,
    io::Write,
    ffi::OsStr,
    path::{Path, PathBuf},
    process,
};

use img_parts::jpeg::Jpeg;
use snafu::{OptionExt, ResultExt};

mod error;
use error::{
    ReadDirSnafu, ReadDirEntrySnafu, ReadImageSnafu, EncodeJpegSnafu, DecodeJpegSnafu,
    NotAClassNameSnafu, NonUtf8ClassNameSnafu, RunCommandSnafu, CommandFailedSnafu,
    CreateSubdirSnafu, NoFileNameSnafu,
};
pub use error::{Error, Result};

mod bootstrap;
pub use bootstrap::{classify_situation, scan_class_dir, bootstrap_originaux, Situation, TopLevelEntry};

/// Extract the non-extension part of the final component of `path`
pub fn basename_stem(path: impl AsRef<Path>) -> Option<String> {
    path
        .as_ref()
        .file_name()
        .map(Path::new)?
        .file_stem()?
        .to_str()
        .map(Into::into)
}

/// Extract name and surname from filename in format 'name @ surname.<extension>'
pub fn filename_to_given_family(path: impl AsRef<Path>) -> Option<(String, String)> {
    let stem = basename_stem(path)?;
    let mut split = stem.split('@');
    Some((
        split.next()?.trim().into(),
        split.next()?.trim().into(),
    ))
}

/// Run `cmd`, reporting an error if it could not be spawned, or exited unsuccessfully
fn run_checked(cmd: &mut process::Command) -> Result<()> {
    let command = format!("{cmd:?}");
    let output = cmd.output().context(RunCommandSnafu { command: command.clone() })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return CommandFailedSnafu { command, stderr }.fail();
    }
    Ok(())
}

/// Make sure given directory exists and is empty, deleting previous contents
pub fn ensure_empty_dir(dir: impl AsRef<Path>) -> Result<()> {
    let dir = dir.as_ref().as_os_str();
    run_checked(process::Command::new("rm")   .arg("-rf").arg(dir))?;
    run_checked(process::Command::new("mkdir").arg("-p" ).arg(dir))?;
    Ok(())
}

/// Use the underlying UNIX-like OS' `mv` command
pub fn unix_mv(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    run_checked(
        process::Command::new("mv")
            .arg(from.as_ref().as_os_str())
            .arg(  to.as_ref().as_os_str())
    )
}

/// Use the underlying UNIX-like OS' `rm -rf` command
pub fn unix_rm_rf(path: impl AsRef<Path>) -> Result<()> {
    run_checked(
        process::Command::new("rm")
            .arg(path.as_ref().as_os_str())
            .arg("-rf")
    )
}

/// Return a `Vec` of files in given directory, which have a filename extension
/// implying the contents are a JPEG image.
pub fn find_jpgs_in_dir(dir: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let mut jpgs = Vec::new();
    for entry in std::fs::read_dir(dir).context(ReadDirSnafu { dir })? {
        let path = entry.context(ReadDirEntrySnafu { dir })?.path();
        if is_jpg(&path) { jpgs.push(path); }
    }
    Ok(jpgs)
}

/// Check whether the filename extension is one of jpg, jpeg, JPG or JPEG
pub fn is_jpg(path: impl AsRef<Path>) -> bool {
    if let Some(ref extension) = path.as_ref().extension() {
        ["jpg", "jpeg", "JPG", "JPEG"]
            .iter()
            .map(OsStr::new)
            .collect::<Vec<_>>()
            .contains(extension)
    } else {
        false
    }
}

#[derive(Debug, Clone)]
pub struct Dirs {
    pub class: PathBuf,
    pub photo: PathBuf,
    pub render: PathBuf,
    pub work: PathBuf,
    pub class_name: String,
}

impl Dirs {
    pub fn new(class_dir: impl AsRef<Path>, originals: impl AsRef<Path>) -> Result<Self> {
        let class: PathBuf = class_dir.as_ref().into();
        let class_name = class_from_dir(&class)?;
        Ok(Self {
            photo: class.join(originals),
            render: class.join("Recadré"),
            class,
            work: "/tmp/trombinoscope-working-dir".into(),
            class_name,
        })
    }
}

/// Where a photo goes when the operator removes it from further consideration in the
/// running app (see [`set_aside`]), rather than which crop/orientation/name it ends up
/// with. Both subdirectories of [`Dirs::class`], created lazily on first use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetAsideDestination {
    /// This is the class list photo, not a face at all.
    Liste,
    /// Neither a usable face photo nor the class list; set aside without further review.
    Ecartees,
}

impl SetAsideDestination {
    fn dirname(self) -> &'static str {
        match self {
            SetAsideDestination::Liste    => "Liste",
            SetAsideDestination::Ecartees => "Écartées",
        }
    }
}

/// Move `image_path` (expected to currently live in `dirs.photo`) into `dirs.class`'s
/// subdirectory for `destination`, creating that subdirectory first if this is its
/// first use. Returns the photo's new path.
pub fn set_aside(
    dirs: &Dirs,
    image_path: impl AsRef<Path>,
    destination: SetAsideDestination,
) -> Result<PathBuf> {
    let image_path = image_path.as_ref();
    let target_dir = dirs.class.join(destination.dirname());
    if !target_dir.is_dir() {
        fs::create_dir(&target_dir).context(CreateSubdirSnafu { dir: target_dir.clone() })?;
    }

    let filename = image_path.file_name().context(NoFileNameSnafu { path: image_path })?;
    let target = target_dir.join(filename);
    unix_mv(image_path, &target)?;
    Ok(target)
}

/// Deduce a class name from the given directory
fn class_from_dir(dir: impl AsRef<Path>) -> Result<String> {
    let dir = dir.as_ref();
    let component = dir.components().next_back().context(NotAClassNameSnafu { dir })?;
    let std::path::Component::Normal(class) = component
        else { return NotAClassNameSnafu { dir }.fail() };
    Ok(class.to_str().context(NonUtf8ClassNameSnafu { dir })?.into())
}

#[derive(Debug, Clone)] pub struct Name { pub given: String, pub family: String }
#[derive(Debug, Clone)] pub struct Item { pub image: PathBuf, pub name: Name }

pub fn path_to_item(image_path: impl AsRef<Path>) -> Option<Item> {
    let basename = image_path.as_ref().file_name()?;
    let (given, family) = filename_to_given_family(&image_path)?;
    Some( Item {
        image: basename.into(),
        name: Name { given, family }
    })
}

pub fn read_jpeg(path: impl AsRef<Path>) -> Result<Jpeg> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).context(ReadImageSnafu { path })?;
    bytes_to_jpeg(&bytes)
}
pub fn write_jpeg(jpeg: Jpeg, sink: &mut impl Write) -> Result<()> {
    jpeg.encoder().write_to(sink).context(EncodeJpegSnafu)?;
    Ok(())
}
pub fn bytes_to_jpeg(bytes: &[u8]) -> Result<Jpeg> {
    Jpeg::from_bytes(bytes.to_owned().into()).context(DecodeJpegSnafu)
}

pub fn sort_key(given: &str, family: &str) -> (String, String) {
    (family.to_ascii_uppercase(), given.to_ascii_uppercase())
}

pub fn move_index_by(index: usize, delta: isize, size: usize) -> usize {
    (index as isize + delta).rem_euclid(size as _) as _
}

pub const MAITRES_DE_CLASSE_FILENAME: &str = "maitres-de-classe.txt";
pub const MAITRES_DE_CLASSE_DEFAULT_CONTENT: &str = "Ajouter MdC, séparés par des virgules";
pub const CONFIG_FILENAME: &str = "config.txt";
pub const DEFAULT_JPEG_QUALITY: u8 =  60;
pub const DEFAULT_IMAGE_WIDTH: u32 = 200;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use pretty_assertions::assert_eq;

    #[rstest]
    #[case("123_IMG.JPEG", "123_IMG", "Séparer prénom du nom par un `@`")]
    #[case("John @ Smith.jpg", "John", "Smith")]
    fn test_name(
        #[case] filename: &str,
        #[case] xgiven: &str,
        #[case] xfamily: &str,
    ) {
        let (given, family) = filename_to_given_family(filename).unwrap();
        assert_eq!( given,  xgiven);
        assert_eq!(family, xfamily);
    }
}
