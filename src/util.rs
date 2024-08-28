use std::path::Path;

pub fn filename_to_given_family(path: impl AsRef<Path>) -> Option<(String, String)> {
    let basename = path.as_ref().file_name()?;
    let stem: String = Path::new(basename).file_stem()?.to_str().map(Into::into)?;
    let mut split = stem.split('@');
    Some((
        split.next()?.trim().into(),
        if let Some(name) = split.next() { name.trim() } else { "Séparer prénom du nom par un `@`" }.into()
    ))
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
