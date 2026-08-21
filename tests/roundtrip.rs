//! Round-trip tests: every KYAML document is valid YAML, so serializing and
//! re-parsing through yaml_serde must reproduce the original value exactly.

use serde_json::{Value, json};

fn roundtrip(value: &Value) {
    for (label, rendered) in [
        ("to_string", serde_kyaml::to_string(value).unwrap()),
        ("to_document", serde_kyaml::to_document(value).unwrap()),
        (
            "to_string_compact",
            serde_kyaml::to_string_compact(value).unwrap(),
        ),
    ] {
        let reparsed: Value = serde_kyaml::from_str(&rendered)
            .unwrap_or_else(|e| panic!("{label} output failed to parse as YAML: {e}\n{rendered}"));
        assert_eq!(&reparsed, value, "{label} round-trip\n{rendered}");
    }
}

#[test]
fn scalars() {
    for v in [
        json!(null),
        json!(true),
        json!(false),
        json!(0),
        json!(-9223372036854775808_i64),
        json!(18446744073709551615_u64),
        json!(3.5),
        json!(0.0),
        json!(1e100),
        json!(1e-6),
        json!(""),
        json!("abc"),
    ] {
        roundtrip(&v);
    }
}

#[test]
fn tricky_strings() {
    for s in [
        // type-ambiguous text must come back as a string, not bool/int/null
        "yes",
        "no",
        "on",
        "off",
        "true",
        "false",
        "null",
        "~",
        "1",
        "-1",
        "0x1f",
        "0o777",
        "3.14",
        "1e10",
        ".inf",
        ".nan",
        "11:00",
        "2006-1-2",
        // quoting and escapes
        "naked \"with\" 'quotes' embedded",
        "with\\backslash",
        "line1\nline2",
        "trailing newline\n",
        "\n",
        "  leading spaces\nand\n  more",
        "tabs\there\n\tand there",
        "blank\n\nlines\n",
        "control\u{01}\u{1b}\u{7f}chars",
        "unicode: café \u{1f600} \u{2028} \u{a0} \u{feff}",
        "\0nul",
        " ",
    ] {
        roundtrip(&json!(s));
    }
}

#[test]
fn tricky_keys() {
    let v = json!({
        "safe": 1,
        "with-dash": 2,
        "yes": 3,
        "null": 4,
        "1": 5,
        "11:00": 6,
        "not safe": 7,
        "multi\nline\nkey": 8,
        "2006-1-2": 9,
        "": 10,
    });
    roundtrip(&v);
}

#[test]
fn nested_structures() {
    let v = json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {"name": "demo", "labels": {"app": "x"}},
        "data": {
            "config.yaml": "a: 1\nb:\n  - two\n  - three\n",
            "empty": "",
        },
        "list_of_maps": [{"a": 1}, {"b": 2}],
        "list_of_lists": [[1, 2], [3, 4]],
        "mixed": ["s", 1, true, null, {"m": []}, []],
        "empties": {"map": {}, "list": []},
    });
    roundtrip(&v);
}
