use std::{io, path::PathBuf};

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {

    #[snafu(display("Could not load the embedded Inconsolata font"))]
    LoadFont,

    #[snafu(display("Could not read `{}`: {source}", path.display()))]
    ReadMaitresDeClasse { source: io::Error, path: PathBuf },

    #[snafu(display("Could not write default content to `{}`: {source}", path.display()))]
    WriteMaitresDeClasse { source: io::Error, path: PathBuf },

    #[snafu(display("Could not write generated Typst source to `{}`: {source}", path.display()))]
    WriteTypstSrc { source: io::Error, path: PathBuf },

    #[snafu(display("Typst failed to compile `{src_filename}`:\n{message}"))]
    CompileTypst { message: String, src_filename: String },

    #[snafu(display("Could not write `{}`: {source}", path.display()))]
    WritePdf { source: io::Error, path: PathBuf },

    #[snafu(display("Could not copy `{}` to `{}`: {source}", from.display(), to.display()))]
    CopyPdf { source: io::Error, from: PathBuf, to: PathBuf },

    #[snafu(context(false))]
    Util { source: util::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
