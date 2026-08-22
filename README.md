# serde_kyaml

A [KYAML] serializer for [serde].

KYAML ([KEP-5295][KYAML]) is a strict, unambiguous subset of YAML adopted by
Kubernetes as an output format. It sidesteps the classic YAML footguns
(significant whitespace, the Norway problem, sexagesimal numbers) while
remaining valid YAML that any YAML parser can read:

- always flow style: `{}` for mappings, `[]` for lists
- string values are always double-quoted; keys are quoted only when a YAML
  parser could misread them (`no`, `1e10`, `11:00`, ...)
- 2-space indentation, trailing commas, and "cuddled" brackets for lists of
  maps/lists
- an optional `---` document header to disambiguate from malformed JSON

This crate was vibe ported (by Claude, supervised by a human) from the Go
reference implementation, [`sigs.k8s.io/yaml/kyaml`][go-impl], including its
test suite: the escape tables, key-quoting predicates, and layout fixtures
are ported verbatim, and round-trip tests verify that every output parses
back to the identical value through a real YAML parser.

## Usage

```rust
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Pod {
    api_version: String,
    kind: String,
}

let pod = Pod {
    api_version: "v1".to_string(),
    kind: "Pod".to_string(),
};

print!("{}", serde_kyaml::to_document(&pod).unwrap());
```

```yaml
---
{
  apiVersion: "v1",
  kind: "Pod",
}
```

The API mirrors the Go encoder:

| Function             | Output                                             |
| -------------------- | -------------------------------------------------- |
| `to_string`          | bare value, no header, no trailing newline         |
| `to_document`        | `---\n` + value + `\n`, per the KEP                |
| `to_string_compact`  | single line, `", "` separators, no trailing commas |
| `to_writer`          | `to_string` output into an `io::Write`             |
| `from_str`           | deserialization, delegated to [`yaml_serde`]       |

Since KYAML is plain YAML on the way in, there is no KYAML-specific
deserializer — `from_str` simply hands the input to `yaml_serde`.

## How it works

A value is serialized to a `serde_json::Value` tree (with the
`preserve_order` feature, so struct field order survives), then rendered by a
port of the Go `renderNode`/`renderMapping`/`renderSequence`/`renderString`
logic. This mirrors the Go pipeline, which round-trips objects through
`encoding/json` before rendering.

## Deliberate divergences from the Go implementation

- **Floats keep their float spelling.** Go's JSON round-trip collapses `1.0`
  to `1`; this crate keeps `1.0`, preserving float-ness. Scientific notation
  also kicks in at different magnitudes (`1e-6` rather than `0.000001`,
  `1e+16` rather than `10000000000000000`).
- **Non-finite floats become `null`** (`serde_json` semantics); Go errors.
- **Out-of-range float-looking keys like `1e999` are quoted.** Go leaves
  them bare because its float parse fails with a range error; quoting is
  safer, since a YAML 1.1 parser would read `1e999` as `.inf`.
- Comments, anchors/aliases, and multi-document streams have no serde
  representation and are not supported.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

[KYAML]: https://github.com/kubernetes/enhancements/tree/master/keps/sig-cli/5295-kyaml
[serde]: https://serde.rs
[go-impl]: https://github.com/kubernetes-sigs/yaml/tree/master/kyaml
[`yaml_serde`]: https://crates.io/crates/yaml_serde
