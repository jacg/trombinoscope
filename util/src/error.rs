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

    #[snafu(display("Could not create directory `{}`: {source}", dir.display()))]
    CreateSubdir { source: io::Error, dir: PathBuf },

    #[snafu(display(
        "Impossible de déplacer les photos vers `{}` : {move_error}\n\
         La tentative d'annulation (remise en place des photos déjà déplacées) a, elle aussi, échoué : {rollback_error}",
        dir.display(),
    ))]
    BootstrapRollbackFailed { dir: PathBuf, move_error: String, rollback_error: String },

    #[snafu(display(
        "Les photos ont été remises en place après un échec, mais `{}` n'a pas pu être supprimé : {source}",
        dir.display(),
    ))]
    RemoveSubdirAfterRollback { source: io::Error, dir: PathBuf },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
