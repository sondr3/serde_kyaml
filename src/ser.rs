//! The KYAML value-tree renderer: a port of `renderNode`, `renderMapping`,
//! and `renderSequence` from kubernetes-sigs/yaml's kyaml package, operating
//! on `serde_json::Value` instead of `yaml.Node`.

use serde_json::{Map, Value};

use crate::string::render_string;

#[derive(Clone, Copy, Default)]
pub(crate) struct Flags {
    pub compact: bool,
    pub lazy_quote: bool,
}

const KEY_FLAGS: Flags = Flags {
    // Keys are always rendered single-line, even in regular mode.
    compact: true,
    lazy_quote: true,
};

/// Renders a value into `out`, assuming the cursor is at the start of the
/// node, without a trailing newline. `indent` is the level the node itself
/// sits at; nested content indents further.
pub(crate) fn render_value(value: &Value, indent: usize, flags: Flags, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            use std::fmt::Write;
            write!(out, "{n}").unwrap();
        }
        // String values render their continuation lines one level deeper.
        Value::String(s) => render_string(s, indent + 1, flags, out),
        Value::Array(items) => render_sequence(items, indent, flags, out),
        Value::Object(map) => render_mapping(map, indent, flags, out),
    }
}

fn render_sequence(items: &[Value], indent: usize, flags: Flags, out: &mut String) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }
    if flags.compact {
        out.push('[');
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            render_value(item, 0, flags, out);
        }
        out.push(']');
        return;
    }

    // Lists whose elements are all maps or lists cuddle the brackets to
    // economize vertical space: `[{...}, {...}]`.
    if items.iter().all(is_cuddled_kind) {
        out.push('[');
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            render_value(item, indent, flags, out);
        }
        out.push(']');
    } else {
        out.push_str("[\n");
        write_indent(indent, out);
        for item in items {
            write_indent(1, out);
            render_value(item, indent + 1, flags, out);
            out.push_str(",\n");
            write_indent(indent, out);
        }
        out.push(']');
    }
}

fn is_cuddled_kind(value: &Value) -> bool {
    matches!(value, Value::Object(_) | Value::Array(_))
}

fn render_mapping(map: &Map<String, Value>, indent: usize, flags: Flags, out: &mut String) {
    if map.is_empty() {
        out.push_str("{}");
        return;
    }
    if flags.compact {
        out.push('{');
        for (i, (key, value)) in map.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            render_string(key, 0, KEY_FLAGS, out);
            out.push_str(": ");
            render_value(value, 0, flags, out);
        }
        out.push('}');
        return;
    }

    out.push_str("{\n");
    for (key, value) in map {
        write_indent(indent + 1, out);
        render_string(key, indent + 1, KEY_FLAGS, out);
        out.push_str(": ");
        render_value(value, indent + 1, flags, out);
        out.push_str(",\n");
    }
    write_indent(indent, out);
    out.push('}');
}

pub(crate) fn write_indent(level: usize, out: &mut String) {
    for _ in 0..level {
        out.push_str("  ");
    }
}
