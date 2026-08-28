use std::path::PathBuf;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {

    #[snafu(display("{source}"))]
    #[snafu(context(false))]
    Face { source: face::Error },

    #[snafu(display("{source}"))]
    #[snafu(context(false))]
    Render { source: render::Error },

    #[snafu(display("{source}"))]
    #[snafu(context(false))]
    Util { source: util::Error },

    #[snafu(display("Could not create `{}`: {source}", path.display()))]
    CreateFile { source: std::io::Error, path: PathBuf },

    #[snafu(display("Could not write `{}`: {source}", path.display()))]
    WriteFile { source: std::io::Error, path: PathBuf },

    #[snafu(display("Could not rename `{}` to `{}`: {source}", from.display(), to.display()))]
    RenameFile { source: std::io::Error, from: PathBuf, to: PathBuf },

    #[snafu(display("Could not encode JPEG for `{}`: {source}", path.display()))]
    EncodeJpeg { source: image::ImageError, path: PathBuf },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
