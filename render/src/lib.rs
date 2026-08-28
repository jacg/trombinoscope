use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use comemo::Prehashed;
use snafu::{OptionExt, ResultExt};

use typst::{
    diag::{FileError, FileResult, PackageError, PackageResult},
    eval::Tracer,
    foundations::{eco_format, Bytes, Datetime, Smart},
    syntax::{FileId, Source, package::PackageSpec},
    text::{Font, FontBook},
    Library,
};

use util::{
    Dirs, Item, Name,
    MAITRES_DE_CLASSE_FILENAME, MAITRES_DE_CLASSE_DEFAULT_CONTENT,
    find_jpgs_in_dir, path_to_item, sort_key, unix_rm_rf, unix_mv,
};

mod error;
use error::{
    LoadFontSnafu, ReadMaitresDeClasseSnafu, WriteMaitresDeClasseSnafu, WriteTypstSrcSnafu,
    CompileTypstSnafu, WritePdfSnafu, CopyPdfSnafu,
};
pub use error::{Error, Result};

/// Main interface that determines the environment for Typst.
pub struct TypstWrapperWorld {
    /// Root path to which files will be resolved.
    root: PathBuf,

    /// The content of a source.
    source: Source,

    /// The standard library.
    library: Prehashed<Library>,

    /// Metadata about all known fonts.
    book: Prehashed<FontBook>,

    /// Metadata about all known fonts.
    fonts: Vec<Font>,

    /// Map of all known files.
    files: RefCell<HashMap<FileId, FileEntry>>,

    /// Cache directory (e.g. where packages are downloaded to).
    cache_directory: PathBuf,

    /// http agent to download packages.
    http: ureq::Agent,

    /// Datetime.
    time: time::OffsetDateTime,
}

impl TypstWrapperWorld {
    pub fn new(root: String, source: String) -> Result<Self> {
        let fonts = fonts()?;

        Ok(Self {
            library: Prehashed::new(Library::default()),
            book: Prehashed::new(FontBook::from_fonts(&fonts)),
            root: PathBuf::from(root),
            fonts,
            source: Source::detached(source),
            time: time::OffsetDateTime::now_utc(),
            cache_directory: std::env::var_os("CACHE_DIRECTORY")
                .map(|os_path| os_path.into())
                .unwrap_or(std::env::temp_dir()),
            http: ureq::Agent::new(),
            files: RefCell::new(HashMap::new()),
        })
    }
}
impl TypstWrapperWorld {
    /// Helper to handle file requests.
    ///
    /// Requests will be either in packages or a local file.
    fn file(&self, id: FileId) -> FileResult<RefMut<'_, FileEntry>> {
        if let Ok(entry) = RefMut::filter_map(self.files.borrow_mut(), |files| files.get_mut(&id)) {
            return Ok(entry);
        }
        let path = if let Some(package) = id.package() {
            // Fetching file from package
            let package_dir = self.download_package(package)?;
            id.vpath().resolve(&package_dir)
        } else {
            // Fetching file from disk
            id.vpath().resolve(&self.root)
        }
        .ok_or(FileError::AccessDenied)?;
        // Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        let content = std::fs::read(&path).map_err(|error| FileError::from_io(error, &path))?;
        Ok(RefMut::map(self.files.borrow_mut(), |files| {
            files.entry(id).or_insert(FileEntry::new(content, None))
        }))
    }

    /// Downloads the package and returns the system path of the unpacked package.
    fn download_package(&self, package: &PackageSpec) -> PackageResult<PathBuf> {
        let package_subdir = format!("{}/{}/{}", package.namespace, package.name, package.version);
        let path = self.cache_directory.join(package_subdir);

        if path.exists() {
            return Ok(path);
        }

        eprintln!("downloading {package}");
        let url = format!(
            "https://packages.typst.org/{}/{}-{}.tar.gz",
            package.namespace, package.name, package.version,
        );

        let response = retry(|| {
            let response = self
                .http
                .get(&url)
                .call()
                .map_err(|error| eco_format!("{error}"))?;

            let status = response.status();
            if !http_successful(status) {
                return Err(eco_format!(
                    "response returned unsuccessful status code {status}",
                ));
            }

            Ok(response)
        })
        .map_err(|error| PackageError::NetworkFailed(Some(error)))?;

        let mut compressed_archive = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut compressed_archive)
            .map_err(|error| PackageError::NetworkFailed(Some(eco_format!("{error}"))))?;
        let raw_archive = zune_inflate::DeflateDecoder::new(&compressed_archive)
            .decode_gzip()
            .map_err(|error| PackageError::MalformedArchive(Some(eco_format!("{error}"))))?;
        let mut archive = tar::Archive::new(raw_archive.as_slice());
        archive.unpack(&path).map_err(|error| {
            _ = std::fs::remove_dir_all(&path);
            PackageError::MalformedArchive(Some(eco_format!("{error}")))
        })?;

        Ok(path)
    }
}

/// This is the interface we have to implement such that `typst` can compile it.
///
/// I have tried to keep it as minimal as possible
impl typst::World for TypstWrapperWorld {
    /// Standard library.
    fn library(&self) -> &Prehashed<Library> {
        &self.library
    }

    /// Metadata about all known Books.
    fn book(&self) -> &Prehashed<FontBook> {
        &self.book
    }

    /// Accessing the main source file.
    fn main(&self) -> Source {
        self.source.clone()
    }

    /// Accessing a specified source file (based on `FileId`).
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            self.file(id)?.source(id)
        }
    }

    /// Accessing a specified file (non-file).
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.file(id).map(|file| file.bytes.clone())
    }

    /// Accessing a specified font per index of font book.
    fn font(&self, id: usize) -> Option<Font> {
        self.fonts.get(id).cloned()
    }

    /// Get the current date.
    ///
    /// Optionally, an offset in hours is given.
    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let offset = offset.unwrap_or(0);
        let offset = time::UtcOffset::from_hms(offset.try_into().ok()?, 0, 0).ok()?;
        let time = self.time.checked_to_offset(offset)?;
        Some(Datetime::Date(time.date()))
    }
}


/// A File that will be stored in the HashMap.
#[derive(Clone, Debug)]
struct FileEntry {
    bytes: Bytes,
    source: Option<Source>,
}

impl FileEntry {
    fn new(bytes: Vec<u8>, source: Option<Source>) -> Self {
        Self {
            bytes: bytes.into(),
            source,
        }
    }

    fn source(&mut self, id: FileId) -> FileResult<Source> {
        let source = if let Some(source) = &self.source {
            source
        } else {
            let contents = std::str::from_utf8(&self.bytes).map_err(|_| FileError::InvalidUtf8)?;
            let contents = contents.trim_start_matches('\u{feff}');
            let source = Source::new(id, contents.into());
            self.source.insert(source)
        };
        Ok(source.clone())
    }
}

pub fn fonts() -> Result<Vec<Font>> {
    let bytes = include_bytes!("../../fonts/Inconsolata-Black.ttf");
    let buffer = Bytes::from_static(bytes);
    Ok(vec![Font::new(buffer, 0).context(LoadFontSnafu)?])
}

pub fn retry<T, E>(mut f: impl FnMut() -> Result<T, E>) -> Result<T, E> {
    if let Ok(ok) = f() {
        Ok(ok)
    } else {
        f()
    }
}

pub fn http_successful(status: u16) -> bool {
    // 2XX
    status / 100 == 2
}

fn trombi_typst_src(items: &[Item], dir: &Dirs) -> Result<String> {
    let table_items = items
        .iter()
        .map(|Item { image, name: Name { given, family } }| {
            format!("    item([{given}], [{family}], \"{image}\")", image=image.display())
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let n_columns = if items.len() <= 24 { 6 } else { 7 };
    let class_name = &dir.class_name;

    let maitres_de_classe_file_location = dir.class.join(MAITRES_DE_CLASSE_FILENAME);
    let maitres_de_classe = match fs::read_to_string(&maitres_de_classe_file_location) {
        Ok(content) => content,
        Err(_) => {
            fs::write(&maitres_de_classe_file_location, MAITRES_DE_CLASSE_DEFAULT_CONTENT)
                .context(WriteMaitresDeClasseSnafu { path: &maitres_de_classe_file_location })?;
            fs::read_to_string(&maitres_de_classe_file_location)
                .context(ReadMaitresDeClasseSnafu { path: &maitres_de_classe_file_location })?
        }
    }
    .split(',')
    .map(|item| item.trim())
    .filter(|item| !item.is_empty())
    .collect::<Vec<_>>()
    .join(" - ");

    Ok(format!(r#"#set page(
  paper: "a4",
  margin: (top: 10mm, bottom: 4mm, left: 5mm, right: 5mm),
)

#let colG = rgb(150,0,0)
#let colF = rgb(0,0,150)

#align(center, text([CLASSE {class_name}], size: 50pt))
#v(-10mm) // TODO find sensible way of reducing space before table

#align(center, text([{maitres_de_classe}], size: 15pt))

#let pic(path) = image(path, width: 100%)

#let label(given, family) = [
    #text(given , stroke: none, fill: colG) #h(1mm)
    #text(family, stroke: none, fill: colF)
]

#let n_columns = {n_columns}
#let pic_w = 200mm / n_columns
#let pic_h = pic_w * 5 / 4

// Shrinks `body` down from `max-size` (never below `min-size`) via binary
// search until it, laid out at `avail-width` (wrapping as needed), is no
// taller than `avail-height`. Prefers wrapping onto extra lines over
// shrinking the font.
#let fit-text(body, avail-width, avail-height, max-size: 10pt, min-size: 5pt, tries: 8) = context {{
    let lo   = min-size
    let hi   = max-size
    let best = min-size

    for _ in range(tries) {{
        let mid = (lo + hi) / 2
        let h   = measure(block(width: avail-width, text(size: mid, body))).height

        if h <= avail-height {{
            best = mid
            lo   = mid
        }} else {{
            hi = mid
        }}
    }}

    block(width: avail-width, text(size: best, body))
}}

#let item(given, family, path) = {{
    set rect(
        width: pic_w,
        inset: 5pt,
        stroke: 0.5pt + gray,
        height: 10mm,
    )

    let avail-w = pic_w - 10pt
    let avail-h = 10mm  - 10pt

    let given  = fit-text(text(stroke: none, fill: colG,        given  ), avail-w, avail-h)
    let family = fit-text(text(stroke: none, fill: colF, upper[#family]), avail-w, avail-h)

    stack(
        dir: ttb,
        rect(pic(path), height: pic_h, stroke: (           bottom: none)),
        rect(align(bottom, given )   , stroke: (top: none, bottom: none)),
        rect(align(top   , family)   , stroke: (top: none              )),
    )
}}

#table(
    columns: n_columns,
    align: center + horizon,
    stroke: none,
    inset: 0pt,

{table_items}
)
"#))

}

fn render(
    content: String,
    dir: &Dirs,
) -> Result<()> {
    let class_name = &dir.class_name;
    let typst_src_filename = format!("generated-tombinoscope_{class_name}.typ");

    let typst_src_path = dir.work.join(&typst_src_filename);
    let mut out = File::create(&typst_src_path).context(WriteTypstSrcSnafu { path: &typst_src_path })?;
    out.write_all(content.as_bytes()).context(WriteTypstSrcSnafu { path: &typst_src_path })?;

    // Create world with content.
    let world = TypstWrapperWorld::new(dir.work.display().to_string(), content.clone())?;

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer)
        .map_err(|err| CompileTypstSnafu {
            message: format!("{err:?}"),
            src_filename: typst_src_filename.clone(),
        }.build())?;

    // Output to pdf
    let pdf_bytes = typst_pdf::pdf(&document, Smart::Auto, None);

    let pdf_path = trombi_file_for_dir(&dir.work, &dir.class_name);

    fs::write(&pdf_path, pdf_bytes).context(WritePdfSnafu { path: &pdf_path })?;

    let moved_pdf_path = trombi_file_for_dir(&dir.class, &dir.class_name);
    let moved_pdf_path_display = moved_pdf_path.display();
    let msg = &format!("PDF généré: `{moved_pdf_path_display}`.");
    println!("{msg}");
    Ok(())
}


pub fn trombi_file_for_dir(dir: impl AsRef<Path>, class_name: &str) -> PathBuf {
    dir.as_ref().join(format!("trombinoscope_{class_name}.pdf"))
}


pub fn trombinoscope(dir: &Dirs) -> Result<()> {
    let items = find_jpgs_in_dir(&dir.work)?
        .iter()
        .filter_map(path_to_item)
        .collect::<Vec<_>>();

    let mut items = items.to_vec();
    items.sort_by_cached_key(|f| sort_key(&f.name.given, &f.name.family));

    render(trombi_typst_src(&items, dir)?, dir)?;

    unix_rm_rf(&dir.render)?;
    unix_mv(&dir.work, &dir.render)?;

    let from = trombi_file_for_dir(&dir.render, &dir.class_name);
    let to   = trombi_file_for_dir(&dir.class , &dir.class_name);
    fs::copy(&from, &to).context(CopyPdfSnafu { from, to })?;

    Ok(())
}
