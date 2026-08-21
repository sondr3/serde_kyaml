//! Port of the table tests from kubernetes-sigs/yaml's kyaml_test.go.
//!
//! Expected regular-mode strings are written with a trailing newline (via
//! indoc) purely for readability; `check` strips it for `to_string` and uses
//! it as-is for `to_document`. Float spellings follow serde_json's number
//! formatter, not Go's encoding/json (see the crate docs for the divergence
//! list).

use std::collections::BTreeMap;

use indoc::indoc;
use serde::Serialize;
use serde_json::json;

fn check<T: Serialize>(input: &T, regular: &str, compact: &str) {
    let bare = regular.strip_suffix('\n').unwrap_or(regular);
    assert_eq!(serde_kyaml::to_string(input).unwrap(), bare, "to_string");
    assert_eq!(
        serde_kyaml::to_document(input).unwrap(),
        format!("---\n{bare}\n"),
        "to_document"
    );
    assert_eq!(serde_kyaml::to_string_compact(input).unwrap(), compact, "compact");
}

#[test]
fn integers() {
    check(&123_i32, "123", "123");
    check(&-123_i32, "-123", "-123");
    check(&0_i32, "0", "0");
    check(&12345678900_i64, "12345678900", "12345678900");
    check(&i64::MIN, "-9223372036854775808", "-9223372036854775808");
    check(&u64::MAX, "18446744073709551615", "18446744073709551615");
    check(&0_u64, "0", "0");
}

#[test]
fn floats() {
    check(&3.5_f32, "3.5", "3.5");
    check(&-3.5_f64, "-3.5", "-3.5");
    // Divergences from Go (which collapses through encoding/json): floats
    // keep their float spelling.
    check(&0.0_f64, "0.0", "0.0");
    check(&1.0_f64, "1.0", "1.0");
    check(&1e100_f64, "1e+100", "1e+100");
    check(&1e-6_f64, "1e-6", "1e-6");
    // Non-finite floats are not representable; serde_json maps them to null.
    check(&f64::NAN, "null", "null");
    check(&f64::INFINITY, "null", "null");
}

#[test]
fn scalars() {
    check(&true, "true", "true");
    check(&false, "false", "false");
    check(&"", r#""""#, r#""""#);
    check(&"abc", r#""abc""#, r#""abc""#);
    check(&Option::<i32>::None, "null", "null");
    check(&(), "null", "null");
    // Scalar string values are always quoted, even unambiguous ones.
    check(&"safe", r#""safe""#, r#""safe""#);
    check(
        &"naked \"with\" 'quotes' embedded",
        r#""naked \"with\" 'quotes' embedded""#,
        r#""naked \"with\" 'quotes' embedded""#,
    );
}

#[test]
fn multi_line_string_top_level() {
    check(
        &"This\n is a\n  multi-line\n string",
        indoc! {r#"
            "\
               This\n\
              \ is a\n\
              \  multi-line\n\
              \ string\
              "
        "#},
        r#""This\n is a\n  multi-line\n string""#,
    );
}

#[test]
fn multi_line_strings_in_mapping() {
    #[derive(Serialize)]
    struct MultiLine {
        simple: &'static str,
        leading_spaces: &'static str,
        leading_tabs: &'static str,
        blank_lines: &'static str,
    }
    let input = MultiLine {
        simple: "This is a multi-line string.\nIt has multiple lines.\n",
        leading_spaces: "This is a multi-line string.\n  It can\n    retain space indentation.\n",
        leading_tabs: "This is a multi-line string.\n\tIt can\n\t\tretain tab indentation.\n",
        blank_lines: "This is a multi-line string.\n\nIt can retain blank lines.\n",
    };
    // The leading_tabs continuation lines contain real tab characters after
    // the escaping backslash, so this block is built with escapes rather
    // than indoc.
    let regular = concat!(
        "{\n",
        "  simple: \"\\\n",
        "     This is a multi-line string.\\n\\\n",
        "     It has multiple lines.\\n\\\n",
        "    \",\n",
        "  leading_spaces: \"\\\n",
        "     This is a multi-line string.\\n\\\n",
        "    \\  It can\\n\\\n",
        "    \\    retain space indentation.\\n\\\n",
        "    \",\n",
        "  leading_tabs: \"\\\n",
        "     This is a multi-line string.\\n\\\n",
        "    \\\tIt can\\n\\\n",
        "    \\\t\tretain tab indentation.\\n\\\n",
        "    \",\n",
        "  blank_lines: \"\\\n",
        "     This is a multi-line string.\\n\\\n",
        "     \\n\\\n",
        "     It can retain blank lines.\\n\\\n",
        "    \",\n",
        "}\n",
    );
    let compact = concat!(
        "{simple: \"This is a multi-line string.\\nIt has multiple lines.\\n\", ",
        "leading_spaces: \"This is a multi-line string.\\n  It can\\n    retain space indentation.\\n\", ",
        "leading_tabs: \"This is a multi-line string.\\n\\tIt can\\n\\t\\tretain tab indentation.\\n\", ",
        "blank_lines: \"This is a multi-line string.\\n\\nIt can retain blank lines.\\n\"}",
    );
    check(&input, regular, compact);
}

#[test]
fn unquoted_keys() {
    let input: BTreeMap<&str, i32> = [
        ("_", 1),
        ("_with_underscore_", 2),
        ("safe", 3),
        ("with-dash", 4),
        ("with.dot", 5),
        ("with/slash", 6),
    ]
    .into();
    check(
        &input,
        indoc! {"
            {
              _: 1,
              _with_underscore_: 2,
              safe: 3,
              with-dash: 4,
              with.dot: 5,
              with/slash: 6,
            }
        "},
        "{_: 1, _with_underscore_: 2, safe: 3, with-dash: 4, with.dot: 5, with/slash: 6}",
    );
}

#[test]
fn dash_dot_slash_keys() {
    let input: BTreeMap<&str, i32> =
        [("-", 123), ("-leading-dash", 123), ("trailing-dash-", 123)].into();
    check(
        &input,
        indoc! {r#"
            {
              "-": 123,
              "-leading-dash": 123,
              "trailing-dash-": 123,
            }
        "#},
        r#"{"-": 123, "-leading-dash": 123, "trailing-dash-": 123}"#,
    );

    let input: BTreeMap<&str, i32> = [(".", 1), (".leading.dot", 2), ("trailing.dot.", 3)].into();
    check(
        &input,
        indoc! {r#"
            {
              ".": 1,
              ".leading.dot": 2,
              "trailing.dot.": 3,
            }
        "#},
        r#"{".": 1, ".leading.dot": 2, "trailing.dot.": 3}"#,
    );

    let input: BTreeMap<&str, i32> =
        [("/", 1), ("/leading/slash", 2), ("trailing/slash/", 3)].into();
    check(
        &input,
        indoc! {r#"
            {
              "/": 1,
              "/leading/slash": 2,
              "trailing/slash/": 3,
            }
        "#},
        r#"{"/": 1, "/leading/slash": 2, "trailing/slash/": 3}"#,
    );
}

#[test]
fn null_like_keys() {
    let input: BTreeMap<&str, i32> = [
        ("NULL", 1),
        ("Null", 2),
        ("null", 3),
        ("unambiguous", 4),
        ("~", 5),
    ]
    .into();
    check(
        &input,
        indoc! {r#"
            {
              "NULL": 1,
              "Null": 2,
              "null": 3,
              unambiguous: 4,
              "~": 5,
            }
        "#},
        r#"{"NULL": 1, "Null": 2, "null": 3, unambiguous: 4, "~": 5}"#,
    );
}

#[test]
fn bool_like_keys() {
    let input: BTreeMap<&str, i32> = [
        ("ON", 1),
        ("On", 2),
        ("TRUE", 3),
        ("True", 4),
        ("YES", 5),
        ("Yes", 6),
        ("on", 7),
        ("true", 8),
        ("yes", 9),
    ]
    .into();
    check(
        &input,
        indoc! {r#"
            {
              "ON": 1,
              "On": 2,
              "TRUE": 3,
              "True": 4,
              "YES": 5,
              "Yes": 6,
              "on": 7,
              "true": 8,
              "yes": 9,
            }
        "#},
        r#"{"ON": 1, "On": 2, "TRUE": 3, "True": 4, "YES": 5, "Yes": 6, "on": 7, "true": 8, "yes": 9}"#,
    );

    let input: BTreeMap<&str, i32> = [
        ("FALSE", 1),
        ("False", 2),
        ("NO", 3),
        ("No", 4),
        ("OFF", 5),
        ("Off", 6),
        ("false", 7),
        ("no", 8),
        ("off", 9),
    ]
    .into();
    check(
        &input,
        indoc! {r#"
            {
              "FALSE": 1,
              "False": 2,
              "NO": 3,
              "No": 4,
              "OFF": 5,
              "Off": 6,
              "false": 7,
              "no": 8,
              "off": 9,
            }
        "#},
        r#"{"FALSE": 1, "False": 2, "NO": 3, "No": 4, "OFF": 5, "Off": 6, "false": 7, "no": 8, "off": 9}"#,
    );
}

#[test]
fn number_like_keys() {
    let input: BTreeMap<&str, i32> = [("-1", 1), ("1", 2), ("_1", 3), ("__1__2__", 4)].into();
    check(
        &input,
        indoc! {r#"
            {
              "-1": 1,
              "1": 2,
              "_1": 3,
              "__1__2__": 4,
            }
        "#},
        r#"{"-1": 1, "1": 2, "_1": 3, "__1__2__": 4}"#,
    );

    let input: BTreeMap<&str, i32> = [
        ("+.inf", 1),
        ("-.inf", 2),
        (".inf", 3),
        (".nan", 4),
        ("3.14", 5),
    ]
    .into();
    check(
        &input,
        indoc! {r#"
            {
              "+.inf": 1,
              "-.inf": 2,
              ".inf": 3,
              ".nan": 4,
              "3.14": 5,
            }
        "#},
        r#"{"+.inf": 1, "-.inf": 2, ".inf": 3, ".nan": 4, "3.14": 5}"#,
    );
}

#[test]
fn date_like_keys() {
    let input: BTreeMap<&str, i32> = [
        ("11:00", 1),
        ("2006", 2),
        ("2006-1-2", 3),
        ("2006-1-2T15:4:5.999999999-08:00", 4),
    ]
    .into();
    check(
        &input,
        indoc! {r#"
            {
              "11:00": 1,
              "2006": 2,
              "2006-1-2": 3,
              "2006-1-2T15:4:5.999999999-08:00": 4,
            }
        "#},
        r#"{"11:00": 1, "2006": 2, "2006-1-2": 3, "2006-1-2T15:4:5.999999999-08:00": 4}"#,
    );
}

#[test]
fn unsafe_keys() {
    let input: BTreeMap<&str, i32> = [("not safe", 1), ("with\\backslash", 2)].into();
    check(
        &input,
        indoc! {r#"
            {
              "not safe": 1,
              "with\\backslash": 2,
            }
        "#},
        r#"{"not safe": 1, "with\\backslash": 2}"#,
    );
}

#[test]
fn multi_line_keys() {
    // Keys are always rendered single-line, even in regular mode.
    let input: BTreeMap<&str, i32> = [("1\n 2\n  \n3", 123), ("4\n 5\n  \n6", 456)].into();
    check(
        &input,
        indoc! {r#"
            {
              "1\n 2\n  \n3": 123,
              "4\n 5\n  \n6": 456,
            }
        "#},
        r#"{"1\n 2\n  \n3": 123, "4\n 5\n  \n6": 456}"#,
    );
}

#[test]
fn int_keys() {
    let input: BTreeMap<i32, i32> = [(1, 1), (2, 2), (3, 3)].into();
    check(
        &input,
        indoc! {r#"
            {
              "1": 1,
              "2": 2,
              "3": 3,
            }
        "#},
        r#"{"1": 1, "2": 2, "3": 3}"#,
    );
}

#[test]
fn empty_collections() {
    #[derive(Serialize)]
    struct Empty {}
    check(&Empty {}, "{}", "{}");
    check(&BTreeMap::<String, i32>::new(), "{}", "{}");
    check(&Vec::<i32>::new(), "[]", "[]");
    check(&[0_i32; 0], "[]", "[]");
}

#[test]
fn flat_collections() {
    check(
        &vec![1, 2, 3],
        indoc! {"
            [
              1,
              2,
              3,
            ]
        "},
        "[1, 2, 3]",
    );
    check(
        &[0, 0, 0],
        indoc! {"
            [
              0,
              0,
              0,
            ]
        "},
        "[0, 0, 0]",
    );
    let map: BTreeMap<&str, i32> = [("a", 1), ("b", 2), ("c", 3)].into();
    check(
        &map,
        indoc! {"
            {
              a: 1,
              b: 2,
              c: 3,
            }
        "},
        "{a: 1, b: 2, c: 3}",
    );
}

#[allow(non_snake_case)]
#[derive(Serialize)]
struct SimpleStruct {
    I: i64,
    S: String,
}

#[test]
fn structs() {
    check(
        &SimpleStruct {
            I: 0,
            S: String::new(),
        },
        indoc! {r#"
            {
              I: 0,
              S: "",
            }
        "#},
        r#"{I: 0, S: ""}"#,
    );
    check(
        &SimpleStruct {
            I: 1,
            S: "one".to_string(),
        },
        indoc! {r#"
            {
              I: 1,
              S: "one",
            }
        "#},
        r#"{I: 1, S: "one"}"#,
    );
}

#[test]
fn cuddled_list_of_structs() {
    #[allow(non_snake_case)]
    #[derive(Serialize)]
    struct OneField {
        I: i64,
    }
    let input = vec![OneField { I: 1 }, OneField { I: 2 }, OneField { I: 3 }];
    check(
        &input,
        indoc! {"
            [{
              I: 1,
            }, {
              I: 2,
            }, {
              I: 3,
            }]
        "},
        "[{I: 1}, {I: 2}, {I: 3}]",
    );
}

#[test]
fn cuddled_list_of_lists() {
    let input = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]];
    check(
        &input,
        indoc! {"
            [[
              1,
              2,
              3,
            ], [
              4,
              5,
              6,
            ], [
              7,
              8,
              9,
            ]]
        "},
        "[[1, 2, 3], [4, 5, 6], [7, 8, 9]]",
    );
}

#[test]
fn cuddled_list_of_maps() {
    let input: Vec<BTreeMap<&str, i32>> = vec![
        [("a", 1), ("b", 2)].into(),
        [("c", 3), ("d", 4)].into(),
        [("e", 5), ("f", 6)].into(),
    ];
    // "f" is bool-like (Go's strconv.ParseBool accepts it) and gets quoted.
    check(
        &input,
        indoc! {r#"
            [{
              a: 1,
              b: 2,
            }, {
              c: 3,
              d: 4,
            }, {
              e: 5,
              "f": 6,
            }]
        "#},
        r#"[{a: 1, b: 2}, {c: 3, d: 4}, {e: 5, "f": 6}]"#,
    );
}

#[test]
fn mixed_list_is_not_cuddled() {
    let input = json!(["a string", 12345, true, {"fld": 1}, ["a", "b", "c"]]);
    check(
        &input,
        indoc! {r#"
            [
              "a string",
              12345,
              true,
              {
                fld: 1,
              },
              [
                "a",
                "b",
                "c",
              ],
            ]
        "#},
        r#"["a string", 12345, true, {fld: 1}, ["a", "b", "c"]]"#,
    );
}

#[test]
fn mapping_of_lists() {
    let input = json!({
        "a": [1, 2, 3],
        "b": [true, false, true],
        "c": ["naked", "dquoted", "squoted"],
    });
    check(
        &input,
        indoc! {r#"
            {
              a: [
                1,
                2,
                3,
              ],
              b: [
                true,
                false,
                true,
              ],
              c: [
                "naked",
                "dquoted",
                "squoted",
              ],
            }
        "#},
        r#"{a: [1, 2, 3], b: [true, false, true], c: ["naked", "dquoted", "squoted"]}"#,
    );
}

#[test]
fn skip_serializing_if_mirrors_omitempty() {
    #[allow(non_snake_case)]
    #[derive(Serialize, Default)]
    struct OmitEmpty {
        #[serde(skip_serializing_if = "Option::is_none")]
        P: Option<i32>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        S: Vec<i32>,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        M: BTreeMap<String, i32>,
    }
    check(&OmitEmpty::default(), "{}", "{}");

    // omitzero analogue: empty-but-present collections are kept.
    #[allow(non_snake_case)]
    #[derive(Serialize, Default)]
    struct KeepZero {
        S: Vec<i32>,
    }
    check(
        &KeepZero::default(),
        indoc! {"
            {
              S: [],
            }
        "},
        "{S: []}",
    );
}

#[test]
fn enums() {
    #[derive(Serialize)]
    enum E {
        Unit,
        Newtype(i32),
        Struct { fld: i32 },
    }
    check(&E::Unit, r#""Unit""#, r#""Unit""#);
    check(
        &E::Newtype(7),
        indoc! {"
            {
              Newtype: 7,
            }
        "},
        "{Newtype: 7}",
    );
    check(
        &E::Struct { fld: 1 },
        indoc! {"
            {
              Struct: {
                fld: 1,
              },
            }
        "},
        "{Struct: {fld: 1}}",
    );
}

#[test]
fn kep_pod_example() {
    let pod = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "example",
            "labels": {
                "app": "demo",
            },
        },
        "spec": {
            "containers": [{
                "name": "app",
                "image": "nginx:latest",
                "ports": [{
                    "containerPort": 80,
                }],
            }],
        },
    });
    assert_eq!(
        serde_kyaml::to_document(&pod).unwrap(),
        indoc! {r#"
            ---
            {
              apiVersion: "v1",
              kind: "Pod",
              metadata: {
                name: "example",
                labels: {
                  app: "demo",
                },
              },
              spec: {
                containers: [{
                  name: "app",
                  image: "nginx:latest",
                  ports: [{
                    containerPort: 80,
                  }],
                }],
              },
            }
        "#},
    );
}

#[test]
fn to_writer_matches_to_string() {
    let mut buf = Vec::new();
    serde_kyaml::to_writer(&mut buf, &vec![1, 2, 3]).unwrap();
    assert_eq!(
        String::from_utf8(buf).unwrap(),
        serde_kyaml::to_string(&vec![1, 2, 3]).unwrap()
    );
}
