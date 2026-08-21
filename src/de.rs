use serde::Deserialize;

/// Deserializes a value from KYAML text. KYAML is a strict subset of YAML,
/// so this accepts any YAML, delegating to [`yaml_serde::from_str`].
pub fn from_str<'de, T: Deserialize<'de>>(s: &'de str) -> crate::Result<T> {
    yaml_serde::from_str(s).map_err(crate::Error::Yaml)
}
