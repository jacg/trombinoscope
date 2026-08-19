use std::{fs, path::Path};

use snafu::ResultExt;

use crate::{
    error::{
        CreateSubdirSnafu, ReadDirSnafu, ReadDirEntrySnafu,
        BootstrapRollbackFailedSnafu, RemoveSubdirAfterRollbackSnafu,
    },
    find_jpgs_in_dir, is_jpg, unix_mv,
    CONFIG_FILENAME, MAITRES_DE_CLASSE_FILENAME,
};
pub use crate::error::{Error, Result};

/// A single entry found directly inside the class directory, as seen by
/// [`classify_situation`]. Deliberately doesn't carry a full `Path`/`DirEntry`, so that
/// function stays pure and easy to exercise with hand-built cases in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelEntry {
    pub name:   String,
    pub is_dir: bool,
    pub is_jpg: bool,
}

/// What state a freshly-handed-over class directory is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Situation {
    /// Photos are loose at top level: nothing has been set up yet. Bootstrapping will
    /// move these (by filename, relative to the class directory) into the originals
    /// subdirectory.
    Fresh { jpgs: Vec<String> },
    /// The originals subdirectory already exists and holds photos: business as usual.
    Established,
    /// Doesn't cleanly look like either of the above; human-readable explanation of why.
    Weird(String),
}

/// Decide which [`Situation`] a class directory is in, from a description of its direct
/// children and, if the originals subdirectory exists, how many jpgs are inside it.
/// Pure and filesystem-free, so it's straightforward to exercise with `rstest`.
pub fn classify_situation(
    top_level: &[TopLevelEntry],
    originals_subdir: &str,
    subdir_jpg_count: Option<usize>,
) -> Situation {
    let visible = |e: &&TopLevelEntry| !e.name.starts_with('.');

    let loose_jpgs: Vec<String> = top_level.iter()
        .filter(visible)
        .filter(|e|  !e.is_dir && e.is_jpg)
        .map(|e| e.name.clone())
        .collect();

    let other_dirs: Vec<String> = top_level.iter()
        .filter(visible)
        .filter(|e| e.is_dir && e.name != originals_subdir)
        .map(|e| e.name.clone())
        .collect();

    let evidence_files: Vec<String> = top_level.iter()
        .filter(visible)
        .filter(|e| !e.is_dir && (e.name == CONFIG_FILENAME || e.name == MAITRES_DE_CLASSE_FILENAME))
        .map(|e| e.name.clone())
        .collect();

    match subdir_jpg_count {
        Some(0) => Situation::Weird(format!(
            "« {originals_subdir} » existe déjà, mais ne contient aucune photo."
        )),
        Some(_) if !loose_jpgs.is_empty() => Situation::Weird(format!(
            "« {originals_subdir} » contient déjà des photos, mais {} photo(s) traînent aussi au premier niveau : {}.",
            loose_jpgs.len(), loose_jpgs.join(", "),
        )),
        Some(_) => Situation::Established,
        None if !loose_jpgs.is_empty() && other_dirs.is_empty() && evidence_files.is_empty() =>
            Situation::Fresh { jpgs: loose_jpgs },
        None => {
            let mut reasons = Vec::new();
            if loose_jpgs.is_empty() {
                reasons.push(format!(
                    "aucune photo trouvée au premier niveau, ni de répertoire « {originals_subdir} »"
                ));
            }
            if !other_dirs.is_empty() {
                reasons.push(format!("répertoire(s) inattendu(s) : {}", other_dirs.join(", ")));
            }
            if !evidence_files.is_empty() {
                reasons.push(format!(
                    "fichier(s) laissant penser qu'on a déjà travaillé ici : {}", evidence_files.join(", "),
                ));
            }
            Situation::Weird(format!(
                "« {originals_subdir} » n'existe pas, mais la situation n'est pas claire pour autant : {}.",
                reasons.join(" ; "),
            ))
        }
    }
}

/// Read the direct children of `class_dir` and, if `originals_subdir` exists inside it,
/// count the jpgs it contains, then delegate to [`classify_situation`].
pub fn scan_class_dir(class_dir: &Path, originals_subdir: &str) -> Result<Situation> {
    let mut top_level = Vec::new();
    for entry in fs::read_dir(class_dir).context(ReadDirSnafu { dir: class_dir })? {
        let entry = entry.context(ReadDirEntrySnafu { dir: class_dir })?;
        let path  = entry.path();
        top_level.push(TopLevelEntry {
            name:   entry.file_name().to_string_lossy().into_owned(),
            is_dir: path.is_dir(),
            is_jpg: is_jpg(&path),
        });
    }

    let subdir = class_dir.join(originals_subdir);
    let subdir_jpg_count = if subdir.is_dir() {
        Some(find_jpgs_in_dir(&subdir)?.len())
    } else {
        None
    };

    Ok(classify_situation(&top_level, originals_subdir, subdir_jpg_count))
}

/// Move `jpgs` (filenames, relative to `class_dir`) into a new `originals_subdir` of
/// `class_dir`, creating that subdirectory first.
///
/// If any individual move fails partway through, everything already moved is moved back
/// and the subdirectory just created is removed again, so a failure leaves the class
/// directory exactly as [`scan_class_dir`] found it (i.e. still `Situation::Fresh`,
/// ready to retry) rather than in some new, unclassified state.
pub fn bootstrap_originaux(class_dir: &Path, originals_subdir: &str, jpgs: &[String]) -> Result<()> {
    let subdir = class_dir.join(originals_subdir);
    fs::create_dir(&subdir).context(CreateSubdirSnafu { dir: subdir.clone() })?;

    let mut moved = Vec::new();
    for jpg in jpgs {
        match unix_mv(class_dir.join(jpg), subdir.join(jpg)) {
            Ok(())          => moved.push(jpg.clone()),
            Err(move_error) => return roll_back(&subdir, class_dir, &moved, move_error),
        }
    }
    Ok(())
}

/// Move `moved` back from `subdir` to `class_dir`, then remove `subdir`. On success,
/// still returns `move_error` (the failure that triggered the rollback); on a rollback
/// failure, returns a `BootstrapRollbackFailed` describing both.
fn roll_back(subdir: &Path, class_dir: &Path, moved: &[String], move_error: Error) -> Result<()> {
    for jpg in moved {
        if let Err(rollback_error) = unix_mv(subdir.join(jpg), class_dir.join(jpg)) {
            return BootstrapRollbackFailedSnafu {
                dir:             subdir.to_path_buf(),
                move_error:     move_error.to_string(),
                rollback_error: rollback_error.to_string(),
            }.fail();
        }
    }
    fs::remove_dir(subdir).context(RemoveSubdirAfterRollbackSnafu { dir: subdir.to_path_buf() })?;
    Err(move_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use pretty_assertions::assert_eq;

    fn file(name: &str) -> TopLevelEntry {
        TopLevelEntry { name: name.into(), is_dir: false, is_jpg: is_jpg(name) }
    }
    fn dir(name: &str) -> TopLevelEntry {
        TopLevelEntry { name: name.into(), is_dir: true, is_jpg: false }
    }

    #[rstest(top_level, subdir_jpg_count, expected,
        case(
            vec![file("Alice @ Dupont.jpg"), file("Bob @ Martin.JPEG")],
            None,
            Situation::Fresh { jpgs: vec!["Alice @ Dupont.jpg".into(), "Bob @ Martin.JPEG".into()] },
        ),
        case(
            vec![dir("Originaux"), file("config.txt"), file("maitres-de-classe.txt")],
            Some(12),
            Situation::Established,
        ),
        case(
            Vec::<TopLevelEntry>::new(),
            None,
            Situation::Weird(
                "« Originaux » n'existe pas, mais la situation n'est pas claire pour autant : \
                 aucune photo trouvée au premier niveau, ni de répertoire « Originaux ».".into()
            ),
        ),
        case(
            vec![dir("Originaux")],
            Some(0),
            Situation::Weird("« Originaux » existe déjà, mais ne contient aucune photo.".into()),
        ),
        case(
            vec![file("Alice @ Dupont.jpg"), dir("Originaux")],
            Some(3),
            Situation::Weird(
                "« Originaux » contient déjà des photos, mais 1 photo(s) traînent aussi \
                 au premier niveau : Alice @ Dupont.jpg.".into()
            ),
        ),
        case(
            vec![file("Alice @ Dupont.jpg"), file("config.txt")],
            None,
            Situation::Weird(
                "« Originaux » n'existe pas, mais la situation n'est pas claire pour autant : \
                 fichier(s) laissant penser qu'on a déjà travaillé ici : config.txt.".into()
            ),
        ),
        case(
            vec![file("Alice @ Dupont.jpg"), dir("Divers")],
            None,
            Situation::Weird(
                "« Originaux » n'existe pas, mais la situation n'est pas claire pour autant : \
                 répertoire(s) inattendu(s) : Divers.".into()
            ),
        ),
        case(
            vec![file("Alice @ Dupont.jpg"), file(".DS_Store")],
            None,
            Situation::Fresh { jpgs: vec!["Alice @ Dupont.jpg".into()] },
        ),
    )]
    fn test_classify_situation(
        top_level: Vec<TopLevelEntry>,
        subdir_jpg_count: Option<usize>,
        expected: Situation,
    ) {
        assert_eq!(classify_situation(&top_level, "Originaux", subdir_jpg_count), expected);
    }

    #[test]
    fn bootstrap_moves_all_jpgs_into_new_subdir() {
        let tmp = std::env::temp_dir().join(format!("trombi-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Alice @ Dupont.jpg"), b"fake").unwrap();
        std::fs::write(tmp.join("Bob @ Martin.jpg"),   b"fake").unwrap();

        let jpgs = vec!["Alice @ Dupont.jpg".to_string(), "Bob @ Martin.jpg".to_string()];
        bootstrap_originaux(&tmp, "Originaux", &jpgs).unwrap();

        assert!(!tmp.join("Alice @ Dupont.jpg").exists());
        assert!(!tmp.join("Bob @ Martin.jpg").exists());
        assert!(tmp.join("Originaux").join("Alice @ Dupont.jpg").exists());
        assert!(tmp.join("Originaux").join("Bob @ Martin.jpg").exists());

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn bootstrap_rolls_back_on_partial_failure() {
        let tmp = std::env::temp_dir().join(format!("trombi-test-rollback-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Alice @ Dupont.jpg"), b"fake").unwrap();
        // "Bob" is *not* actually created on disk, so its move will fail partway through.
        let jpgs = vec!["Alice @ Dupont.jpg".to_string(), "Bob @ Martin.jpg".to_string()];

        let result = bootstrap_originaux(&tmp, "Originaux", &jpgs);

        assert!(result.is_err());
        assert!(tmp.join("Alice @ Dupont.jpg").exists(), "Alice should have been moved back");
        assert!(!tmp.join("Originaux").exists(), "the subdirectory created should be removed again");

        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
