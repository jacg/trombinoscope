use std::path::PathBuf;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {

    #[snafu(display("No face metadata was found in {location}"))]
    NoMetadataFound { location: String },

    #[snafu(display("Something went wrong in our use of `bitcode`. Old metadata version? If so, strip"), context(false))]
    Bitcode { source: bitcode::Error },

    #[snafu(display("Could not find basename in `{}`", path.display()))]
    NoBaseName { path: PathBuf },

    #[snafu(display("Face not loaded yet"))]
    FaceNotLoaded,

    #[snafu(display("Could not create `{}`: {source}", path.display()))]
    CreateFile { source: std::io::Error, path: PathBuf },

    #[snafu(display("Could not write to stdout: {source}"))]
    WriteStdout { source: std::io::Error },

    #[snafu(display("Could not read a key from stdin: {source}"))]
    ReadKey { source: std::io::Error },

    // TODO: this is probably too backend-specific to appear here
    #[snafu(display("Something went wrong in our use of `image`"), context(false))]
    Image { source: image::ImageError },

    #[snafu(context(false))]
    Util { source: util::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
