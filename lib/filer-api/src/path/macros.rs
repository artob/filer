// This is free and unencumbered software released into the public domain.

/// Returns a `&'static` [`Name`](crate::path::Name) from a string literal,
/// checked at compile time.
///
/// The argument may be any `&'static str` constant expression. An invalid
/// name is a compile error, never a run-time panic.
///
/// # Examples
///
/// ```
/// use filer_api::{name, path::Name};
///
/// let file: &'static Name = name!("app.toml");
/// assert_eq!(file.as_str(), "app.toml");
/// ```
///
/// Invalid names do not compile:
///
/// ```compile_fail
/// let _ = filer_api::name!("config/app.toml");
/// ```
///
/// ```compile_fail
/// let _ = filer_api::name!("..");
/// ```
#[macro_export]
macro_rules! name {
    ($name:expr) => {{
        const NAME: &'static $crate::path::Name = $crate::path::Name::__from_literal($name);
        NAME
    }};
}

/// Returns a `&'static` [`RelPath`](crate::path::RelPath) from a string
/// literal, checked at compile time.
///
/// The argument may be any `&'static str` constant expression. An invalid
/// path is a compile error, never a run-time panic.
///
/// # Examples
///
/// ```
/// use filer_api::{path::RelPath, rel};
///
/// let file: &'static RelPath = rel!("config/app.toml");
/// assert_eq!(file.components().count(), 2);
/// assert!(rel!("").is_empty());
/// ```
///
/// Invalid paths do not compile:
///
/// ```compile_fail
/// let _ = filer_api::rel!("config/../app.toml");
/// ```
///
/// ```compile_fail
/// let _ = filer_api::rel!("/etc/passwd");
/// ```
#[macro_export]
macro_rules! rel {
    ($path:expr) => {{
        const PATH: &'static $crate::path::RelPath = $crate::path::RelPath::__from_literal($path);
        PATH
    }};
}

#[cfg(test)]
mod tests {
    use crate::path::{Name, RelPath};

    #[test]
    fn expands_to_static_references() {
        let name: &'static Name = name!("a");
        let path: &'static RelPath = rel!("a/b");
        assert_eq!(name, Name::new("a").unwrap());
        assert_eq!(path, RelPath::new("a/b").unwrap());
        assert_eq!(rel!(""), RelPath::empty());
    }

    #[test]
    fn accepts_constant_expressions() {
        const DIR: &str = "config";
        assert_eq!(name!(DIR).as_str(), "config");
        assert_eq!(
            rel!(concat!("config", "/", "app.toml")).as_str(),
            "config/app.toml"
        );
    }
}
