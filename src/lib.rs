//! A [KYAML] serializer for [serde].
//!
//! KYAML ([KEP-5295]) is a strict, unambiguous subset of YAML designed as an
//! output format: always flow style (`{}` and `[]`), double-quoted string
//! values, lazily-quoted keys, trailing commas, cuddled brackets, and
//! 2-space indentation. Every KYAML document is valid YAML, so reading KYAML
//! needs nothing more than a YAML parser — [`from_str`] delegates to
//! [`yaml_serde`].
//!
//! ```
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Metadata {
//!     name: String,
//! }
//!
//! #[derive(Serialize)]
//! #[serde(rename_all = "camelCase")]
//! struct Pod {
//!     api_version: String,
//!     kind: String,
//!     metadata: Metadata,
//! }
//!
//! let pod = Pod {
//!     api_version: "v1".to_string(),
//!     kind: "Pod".to_string(),
//!     metadata: Metadata { name: "example".to_string() },
//! };
//!
//! assert_eq!(serde_kyaml::to_document(&pod).unwrap(), r#"---
//! {
//!   apiVersion: "v1",
//!   kind: "Pod",
//!   metadata: {
//!     name: "example",
//!   },
//! }
//! "#);
//! ```
//!
//! # Relation to the Go implementation
//!
//! Output matches the Go reference implementation
//! (`sigs.k8s.io/yaml/kyaml`), with these deliberate divergences:
//!
//! - **Floats keep their float spelling.** The Go encoder round-trips
//!   through `encoding/json`, so `1.0` collapses to `1`. This crate renders
//!   numbers with [`serde_json::Number`]'s formatter: `1.0` stays `1.0`
//!   (preserving float-ness), and scientific notation kicks in at different
//!   magnitudes (`1e-6` rather than `0.000001`, `1e+16` rather than
//!   `10000000000000000`).
//! - **Non-finite floats become `null`** (`serde_json::to_value` semantics);
//!   Go returns an error.
//! - **Out-of-range float-looking keys are quoted.** Go leaves a key like
//!   `1e999` unquoted because its float parse fails with a range error;
//!   quoting is safer, since a YAML 1.1 parser would read it as `.inf`.
//! - Comments, anchors/aliases, and multi-document streams are not
//!   representable in serde data and are not supported.
//!
//! Enums use serde's default external tagging (a unit variant is a string, a
//! data variant a single-key map); `None` and unit structs render as `null`.
//!
//! [KYAML]: https://github.com/kubernetes/enhancements/tree/master/keps/sig-cli/5295-kyaml
//! [KEP-5295]: https://github.com/kubernetes/enhancements/tree/master/keps/sig-cli/5295-kyaml

mod de;
mod error;
mod ser;
mod string;

pub use de::from_str;
pub use error::{Error, Result};
use serde::Serialize;

/// Serializes `value` as a bare KYAML value, with no `---` header and no
/// trailing newline. Mirrors the Go encoder's `Marshal`.
pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
	render(value, false)
}

/// Serializes `value` as a KYAML document: `"---\n"`, the value, and a
/// trailing newline. Mirrors the Go encoder's `FromObject`.
pub fn to_document<T: Serialize>(value: &T) -> Result<String> {
	Ok(format!("---\n{}\n", to_string(value)?))
}

/// Serializes `value` as a single line of KYAML: `", "` separators, no
/// trailing commas, no line folding. Still valid KYAML (and YAML).
pub fn to_string_compact<T: Serialize>(value: &T) -> Result<String> {
	render(value, true)
}

/// Serializes `value` as a bare KYAML value into `writer` (the same output
/// as [`to_string`]).
pub fn to_writer<W: std::io::Write, T: Serialize>(mut writer: W, value: &T) -> Result<()> {
	writer.write_all(to_string(value)?.as_bytes())?;
	Ok(())
}

fn render<T: Serialize>(value: &T, compact: bool) -> Result<String> {
	let value = serde_json::to_value(value)?;
	let mut out = String::new();
	ser::render_value(&value, 0, ser::Flags { compact, lazy_quote: false }, &mut out);
	Ok(out)
}
