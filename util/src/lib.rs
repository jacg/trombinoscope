use std::{
    cmp::Ordering,
    io::{self, Write},
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub fn filename_to_given_family(path: impl AsRef<Path>) -> Option<(String, String)> {
    let basename = path.as_ref().file_name()?;
    let stem: String = Path::new(basename).file_stem()?.to_str().map(Into::into)?;
    let mut split = stem.split('@');
    Some((
        split.next()?.trim().into(),
        if let Some(name) = split.next() { name.trim() } else { "Séparer prénom du nom par un `@`" }.into()
    ))
}

pub fn ensure_empty_dir(dir: impl AsRef<Path>) -> std::io::Result<()> {
    let dir = dbg!(dir.as_ref().as_os_str());
    std::process::Command::new("rm")   .arg("-rf").arg(dir).output()?;
    std::process::Command::new("mkdir").arg("-p" ).arg(dir).output()?;
    Ok(())
}

pub fn unix_mv(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    std::process::Command::new("mv")
        .arg(from.as_ref().as_os_str())
        .arg(  to.as_ref().as_os_str())
        .output()?;
    Ok(())
}

pub fn unix_rm_rf(path: impl AsRef<Path>) -> io::Result<()> {
    std::process::Command::new("rm")
        .arg(path.as_ref().as_os_str())
        .arg("-rf")
        .output()?;
    Ok(())
}

pub fn find_jpgs_in_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|res| res.map(|e| e.path()).unwrap())
        .filter(|x| is_jpg(x)) // WTF: eta conversion leads to filter not implementing Iterator!
        .collect()
}

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
    pub fn new(class_dir: impl AsRef<Path>) -> Self {
        let class: PathBuf = class_dir.as_ref().into();
        Self {
            photo: class.join("Complet"),
            render: class.join("Recadré"),
            class,
            work: "/tmp/trombinoscope-working-dir".into(),
        }
    }
    pub fn class_name(&self) -> String { class_from_dir(&self.class)  }
}

fn class_from_dir(dir: impl AsRef<Path>) -> String {
    let std::path::Component::Normal(class) = dir.as_ref().components().last().unwrap()
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

pub fn family_given(l: &Item, r: &Item) -> Ordering {
    use std::cmp::Ordering::*;
    let (Item { name: l, .. }, Item { name: r, .. }) = (l,r);
    match l.family.to_uppercase().cmp(&r.family.to_uppercase()) {
        Equal => l.given.to_uppercase().cmp(&r.given.to_uppercase()),
        different => different,
    }
}


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

pub fn strip_metadata() {
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
