use std::{
    io::{self, Write},
    ffi::OsStr,
    path::{Path, PathBuf},
};

use img_parts::jpeg::Jpeg;

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

/// Make sure given directory exists and is empty, deleting previous contents
pub fn ensure_empty_dir(dir: impl AsRef<Path>) -> std::io::Result<()> {
    let dir = dir.as_ref().as_os_str();
    std::process::Command::new("rm")   .arg("-rf").arg(dir).output()?;
    std::process::Command::new("mkdir").arg("-p" ).arg(dir).output()?;
    Ok(())
}

/// Use the underlying UNIX-like OS' `mv` command
pub fn unix_mv(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    std::process::Command::new("mv")
        .arg(from.as_ref().as_os_str())
        .arg(  to.as_ref().as_os_str())
        .output()?;
    Ok(())
}

/// Use the underlying UNIX-like OS' `rm -rf` command
pub fn unix_rm_rf(path: impl AsRef<Path>) -> io::Result<()> {
    std::process::Command::new("rm")
        .arg(path.as_ref().as_os_str())
        .arg("-rf")
        .output()?;
    Ok(())
}

/// Return a `Vec` of files in given directory, which have a filename extension
/// implying the contents are a JPEG image.
pub fn find_jpgs_in_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|res| res.map(|e| e.path()).unwrap())
        .filter(|x| is_jpg(x)) // WTF: eta conversion leads to filter not implementing Iterator!
        .collect()
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

#[derive(Debug)]
pub struct Dirs {
    pub class: PathBuf,
    pub photo: PathBuf,
    pub render: PathBuf,
    pub work: PathBuf,
}

impl Dirs {
    pub fn new(class_dir: impl AsRef<Path>, originals: impl AsRef<Path>) -> Self {
        let class: PathBuf = class_dir.as_ref().into();
        Self {
            photo: class.join(originals),
            render: class.join("Recadré"),
            class,
            work: "/tmp/trombinoscope-working-dir".into(),
        }
    }
    pub fn class_name(&self) -> String { class_from_dir(&self.class)  }
}

/// Deduce a class name from the given directory
fn class_from_dir(dir: impl AsRef<Path>) -> String {
    let std::path::Component::Normal(class) = dir.as_ref().components().next_back().unwrap()
        else { panic!("Last component of `{dir}` cannot be interpreted as a class name", dir = dir.as_ref().display()) };
    class.to_str().unwrap().into()
}

#[derive(Debug, Clone, Copy)] pub enum FileType { Trombi, Labels }
#[derive(Debug, Clone)      ] pub struct Name { pub given: String, pub family: String }
#[derive(Debug, Clone)      ] pub struct Item { pub image: PathBuf, pub name: Name }

pub fn path_to_item(image_path: impl AsRef<Path>) -> Option<Item> {
    let basename = image_path.as_ref().file_name()?;
    let (given, family) = filename_to_given_family(&image_path)?;
    Some( Item {
        image: basename.into(),
        name: Name { given, family }
    })
}

// TODO make read_jpeg return Result
pub fn read_jpeg(path: impl AsRef<Path>) -> Jpeg { bytes_to_jpeg(&std::fs::read(&path).unwrap()) }
pub fn write_jpeg(jpeg: Jpeg, sink: &mut impl Write) { jpeg.encoder().write_to(sink).unwrap(); }
pub fn bytes_to_jpeg(bytes: &[u8]) -> Jpeg { Jpeg::from_bytes(bytes.to_owned().into()).unwrap() }

pub fn sort_key(given: &str, family: &str) -> (String, String) {
    (family.to_ascii_uppercase(), given.to_ascii_uppercase())
}

pub fn move_index_by(index: usize, delta: isize, size: usize) -> usize {
    (index as isize + delta).rem_euclid(size as _) as _
}

pub const MAITRES_DE_CLASSE_FILENAME: &str = "maitres-de-classe.txt";
pub const MAITRES_DE_CLASSE_DEFAULT_CONTENT: &str = "Ajouter MdC, séparés par des virgules";

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
