use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use comemo::Prehashed;

use typst::{
    diag::{FileError, FileResult, PackageError, PackageResult},
    eval::Tracer,
    foundations::{eco_format, Bytes, Datetime, Smart},
    syntax::{FileId, Source, package::PackageSpec},
    text::{Font, FontBook},
    Library,
};

use util::{Dirs, FileType, Item, Name, find_jpgs_in_dir, path_to_item, family_given, unix_rm_rf, unix_mv};

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
    pub fn new(root: String, source: String) -> Self {
        let fonts = fonts();

        Self {
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
        }
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

pub fn fonts() -> Vec<Font> {
    let bytes = include_bytes!("../../fonts/Inconsolata-Black.ttf");
    let buffer = Bytes::from_static(bytes);
    vec![Font::new(buffer, 0).unwrap()]
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

fn labels_typst_src(items: &[Item], dir: &Dirs) -> String {
    let institution = "CO Montbrillant";
    let class_name = dir.class_name();
    let label = |given, family| format!("label([{given}], [{family}])");
    let labels = items
        .iter()
        .map(|i| label(i.name.given.clone(), i.name.family.clone()))
        .collect::<Vec<_>>()
        .join(",\n    ");

    format!{r#"#set page(
  paper: "a4",
  margin: (top: 10mm, bottom: 4mm, left: 5mm, right: 5mm),
)
#set text(size: 23pt, font: "Inconsolata", weight: "black")

#let colG = rgb(150,0,0)
#let colF = rgb(0,0,150)

#let curry_label(institution, class) = {{
    (given, family) => {{
        set rect(width: 10cm, height: 13.3mm, stroke: none)
        stack(
        dir: ttb,
        rect(),
        rect(align(bottom, text(stroke: none, fill: colF, upper[#family]))),
        rect(              text(stroke: none, fill: colG,      [#given])),
        rect(                                                  [#class]),
        rect(                                                  [#institution]),
       )
   }}
}}

#let label = curry_label([{institution}], [Classe {class_name}])

#table(
    columns: 2,
    align: center + horizon,
    stroke: 0.6pt + gray,
    {labels}
)"#}
}

fn trombi_typst_src(items: &[Item], dir: &Dirs) -> String {
    let table_items = items
        .iter()
        .map(|Item { image, name: Name { given, family } }| {
            format!("    item([{given}], [{family}], \"{image}\")", image=image.display())
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let class_name = dir.class_name();

    format!(r#"#set page(
  paper: "a4",
  margin: (top: 10mm, bottom: 4mm, left: 5mm, right: 5mm),
)

#let colG = rgb(150,0,0)
#let colF = rgb(0,0,150)

#align(center, text([CLASSE {class_name}], size: 50pt))

#v(-10mm) // TODO find sensible way of reducing space before table

#let pic(path) = image(path, width: 100%)

#let label(given, family) = [
    #text(given , stroke: none, fill: colG) #h(1mm)
    #text(family, stroke: none, fill: colF)
]

#let n_columns = 6
#let pic_w = 200mm / n_columns
#let pic_h = pic_w * 5 / 4

#let item(given, family, path) = {{
    set rect(
        width: pic_w,
        inset: 5pt,
        stroke: 0.5pt + gray,
        height: 10mm,
    )

    let given  = text(stroke: none, fill: colG,        given  )
    let family = text(stroke: none, fill: colF, upper[#family])

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
"#)

}

fn render(
    content: String,
    dir: &Dirs,
    ftype: FileType,
) {
    let class_name = dir.class_name();
    let typst_src_filename = format!("generated-{}.typ", match ftype {
        FileType::Trombi => format!("tombinoscope_{class_name}"),
        FileType::Labels => format!("étiquettes_{class_name}"),
    });

    let typst_src_path = dir.work.join(&typst_src_filename);
    let mut out = File::create(typst_src_path).unwrap();
    out.write_all(content.as_bytes()).unwrap();

    // Create world with content.
    let world = TypstWrapperWorld::new(dir.work.display().to_string(), content.clone());

    // Render document
    let mut tracer = Tracer::default();
    let document = typst::compile(&world, &mut tracer)
        .unwrap_or_else(|err| {
            panic!("\nError compiling typst source `{typst_src_filename}`:\n{err:?}\n")
        });

    // Output to pdf
    let pdf_bytes = typst_pdf::pdf(&document, Smart::Auto, None);

    let pdf_path = trombi_file_for_dir(&dir.work, &dir.class_name(), ftype);
    let pdf_path_display = pdf_path.display();

    fs::write(&pdf_path, pdf_bytes)
        .unwrap_or_else(|err| panic!("Error writing {pdf_path_display}:\n{err:?}"));

    let moved_pdf_path = trombi_file_for_dir(&dir.class, &dir.class_name(), ftype);
    let moved_pdf_path_display = moved_pdf_path.display();
    let msg = &format!("PDF généré: `{moved_pdf_path_display}`.");
    println!("{msg}");
}


fn trombi_file_for_dir(dir: impl AsRef<Path>, class_name: &str, ftype: FileType) -> PathBuf {
    use FileType::*;
    dir.as_ref().join(match ftype {
        Trombi => format!("trombinoscope_{class_name}.pdf"),
        Labels => format!("étiquettes_{class_name}.pdf"),
    })
}


pub fn trombinoscope(dir: &Dirs) {
    let items = find_jpgs_in_dir(&dir.work)
        .iter()
        .filter_map(path_to_item)
        .collect::<Vec<_>>();

    let mut items = items.to_vec();
    items.sort_by(family_given);

    use FileType::*;
    render(trombi_typst_src(&items, dir), dir, Trombi);
    render(labels_typst_src(&items, dir), dir, Labels);

    unix_rm_rf(&dir.render).unwrap();
    unix_mv(&dir.work, &dir.render).unwrap();

    fs::copy(
        trombi_file_for_dir(&dir.render, &dir.class_name(), Trombi),
        trombi_file_for_dir(&dir.class , &dir.class_name(), Trombi),
    ).unwrap();

    fs::copy(
        trombi_file_for_dir(&dir.render, &dir.class_name(), Labels),
        trombi_file_for_dir(&dir.class , &dir.class_name(), Labels),
    ).unwrap();

}
