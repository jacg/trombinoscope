use std::{io, path::PathBuf};

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {

    #[snafu(display("Could not read directory `{}`: {source}", dir.display()))]
    ReadDir { source: io::Error, dir: PathBuf },

    #[snafu(display("Could not read an entry inside directory `{}`: {source}", dir.display()))]
    ReadDirEntry { source: io::Error, dir: PathBuf },

    #[snafu(display("Could not read image file `{}`: {source}", path.display()))]
    ReadImage { source: io::Error, path: PathBuf },

    #[snafu(display("Could not write JPEG data: {source}"))]
    EncodeJpeg { source: io::Error },

    #[snafu(display("Does not look like a valid JPEG: {source}"))]
    DecodeJpeg { source: img_parts::Error },

    #[snafu(display("Last component of `{}` cannot be interpreted as a class name", dir.display()))]
    NotAClassName { dir: PathBuf },

    #[snafu(display("Last component of `{}` is not valid UTF-8", dir.display()))]
    NonUtf8ClassName { dir: PathBuf },

    #[snafu(display("Could not run `{command}`: {source}"))]
    RunCommand { source: io::Error, command: String },

    #[snafu(display("`{command}` failed:\n{stderr}"))]
    CommandFailed { command: String, stderr: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
