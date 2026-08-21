//! String rendering: quoting, escaping, and YAML line folding, plus the
//! predicates deciding whether a mapping key can be left unquoted.
//!
//! This is a port of `renderString`, `appendEscapedRune`, `needsQuotes`, and
//! `isTypeAmbiguous` from kubernetes-sigs/yaml's kyaml package.

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use crate::ser::{Flags, write_indent};

const FOLD: &str = "\\\n";

/// Renders a string (single-line or multi-line) into `out`, without a
/// trailing newline. With `flags.lazy_quote`, single-line strings that cannot
/// be misread as another YAML type are emitted bare (used for mapping keys).
pub(crate) fn render_string(val: &str, indent: usize, flags: Flags, out: &mut String) {
    let multi = val.contains('\n');

    if !multi && flags.lazy_quote && !needs_quotes(val) {
        out.push_str(val);
        return;
    }

    // Accumulate into a buffer so the escaper can peek backwards.
    let mut buf = String::new();

    buf.push('"');
    if multi && !flags.compact {
        buf.push_str(FOLD);
    }

    for c in val.chars() {
        append_escaped_char(c, indent, flags.compact, &mut buf);
    }

    let after_newline = buf.ends_with('\n');
    if multi && !flags.compact {
        if !after_newline {
            buf.push_str(FOLD);
        }
        write_indent(indent, &mut buf);
    }
    buf.push('"');

    out.push_str(&buf);
}

fn append_escaped_char(c: char, indent: usize, compact: bool, buf: &mut String) {
    let after_newline = buf.ends_with('\n');

    if after_newline {
        write_indent(indent, buf);
        // Leading whitespace on a folded line must be escaped to survive
        // YAML's fold handling; lines without it get one alignment space
        // (dropped by the parser along with all other leading whitespace).
        if c.is_whitespace() && c != '\n' {
            buf.push('\\');
        } else {
            buf.push(' ');
        }
    }

    // Mode-dependent overrides: regular mode folds newlines for readability
    // and keeps tabs literal; compact mode escapes both inline.
    match c {
        '\n' => {
            buf.push_str(if compact { "\\n" } else { "\\n\\\n" });
            return;
        }
        '\t' => {
            if compact {
                buf.push_str("\\t");
            } else {
                buf.push('\t');
            }
            return;
        }
        _ => {}
    }
    if c == '"' || c == '\\' {
        buf.push('\\');
        buf.push(c);
        return;
    }
    if is_print(c) {
        buf.push(c);
        return;
    }
    match c {
        '\u{07}' => buf.push_str("\\a"),
        '\u{08}' => buf.push_str("\\b"),
        '\u{0c}' => buf.push_str("\\f"),
        '\r' => buf.push_str("\\r"),
        '\u{0b}' => buf.push_str("\\v"),
        '\0' => buf.push_str("\\0"),
        '\u{1b}' => buf.push_str("\\e"),
        '\u{85}' => buf.push_str("\\N"),
        '\u{a0}' => buf.push_str("\\_"),
        '\u{2028}' => buf.push_str("\\L"),
        '\u{2029}' => buf.push_str("\\P"),
        _ => {
            use std::fmt::Write;
            let n = c as u32;
            if n < 0x20 || n == 0x7f {
                write!(buf, "\\x{n:02x}").unwrap();
            } else if n < 0x10000 {
                write!(buf, "\\u{n:04x}").unwrap();
            } else {
                write!(buf, "\\U{n:08x}").unwrap();
            }
        }
    }
}

/// Mirror of Go's `unicode.IsPrint`: categories L, M, N, P, S plus the ASCII
/// space. Notably U+00A0 (Zs), U+2028/U+2029 (Zl/Zp), and format characters
/// (Cf) are NOT printable and get escaped.
fn is_print(c: char) -> bool {
    c == ' '
        || matches!(
            c.general_category_group(),
            GeneralCategoryGroup::Letter
                | GeneralCategoryGroup::Mark
                | GeneralCategoryGroup::Number
                | GeneralCategoryGroup::Punctuation
                | GeneralCategoryGroup::Symbol
        )
}

fn is_letter(c: char) -> bool {
    c.general_category_group() == GeneralCategoryGroup::Letter
}

fn is_number(c: char) -> bool {
    c.general_category_group() == GeneralCategoryGroup::Number
}

/// Whether a string used as a mapping key must be quoted.
pub(crate) fn needs_quotes(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if is_type_ambiguous(s) {
        return true;
    }
    let count = s.chars().count();
    for (i, c) in s.chars().enumerate() {
        if is_letter(c) || is_number(c) || c == '_' {
            continue;
        }
        if i > 0 && i < count - 1 && matches!(c, '-' | '.' | '/') {
            continue;
        }
        return true;
    }
    false
}

/// Whether a YAML parser might read the unquoted form of `s` as something
/// other than a string (bool, int, float, null, timestamp, ...).
pub(crate) fn is_type_ambiguous(s: &str) -> bool {
    // Null-like strings: https://yaml.org/type/null.html
    // Bool-like strings: https://yaml.org/type/bool.html
    // Bare "t"/"f" come from Go's strconv.ParseBool set ("0"/"1" are covered
    // by the int check below).
    if s.len() <= 5 {
        match s.to_lowercase().as_str() {
            "null" | "~" => return true,
            "true" | "t" | "y" | "yes" | "on" | "false" | "f" | "n" | "no" | "off" => return true,
            ".inf" | "-.inf" | "+.inf" | ".nan" => return true,
            _ => {}
        }
    }

    // Number-like strings, with YAML 1.1's `_` digit separators stripped
    // first: https://yaml.org/type/int.html and https://yaml.org/type/float.html
    let stripped: String = s.chars().filter(|&c| c != '_').collect();
    if parses_as_go_int(&stripped) || parses_as_go_float(&stripped) {
        return true;
    }

    // Sexagesimal strings like "11:00" (YAML 1.1, removed in 1.2).
    if is_sexagesimal(s) {
        return true;
    }

    // Time-like strings: http://yaml.org/type/timestamp.html
    if is_timestamp(s) {
        return true;
    }

    false
}

/// Mirror of Go's `strconv.ParseInt(s, 0, 64)` succeeding: optional sign,
/// then `0x`/`0o`/`0b` prefixed or legacy-octal (leading `0`) or decimal
/// digits, fitting in an i64. Overflow counts as NOT int-like (the float
/// check usually picks those up), matching Go.
fn parses_as_go_int(s: &str) -> bool {
    let (neg, rest) = match s.as_bytes().first() {
        Some(b'+') => (false, &s[1..]),
        Some(b'-') => (true, &s[1..]),
        _ => (false, s),
    };
    let bytes = rest.as_bytes();
    let (radix, body) = match bytes {
        [b'0', b'x' | b'X', body @ ..] => (16, body),
        [b'0', b'o' | b'O', body @ ..] => (8, body),
        [b'0', b'b' | b'B', body @ ..] => (2, body),
        [b'0', body @ ..] if !body.is_empty() => (8, body),
        _ => (10, bytes),
    };
    if body.is_empty() || !body.iter().all(|b| (*b as char).is_digit(radix)) {
        return false;
    }
    // Safe: body is ASCII digits, so slicing arithmetic stays on char bounds.
    let body = std::str::from_utf8(body).unwrap();
    let signed = if neg {
        format!("-{body}")
    } else {
        body.to_string()
    };
    i64::from_str_radix(&signed, radix).is_ok()
}

/// Mirror of Go's `strconv.ParseFloat(s, 64)` succeeding: decimal and
/// scientific notation, `inf`/`infinity` (optionally signed) and unsigned
/// `nan` (case-insensitive), plus Go-syntax hex floats like `0x1p-2`.
///
/// Deliberate divergence: Go treats out-of-range values like `1e999` as
/// errors (leaving such keys unquoted); we quote them, which is safer — a
/// YAML 1.1 parser really would read `1e999` as a float.
fn parses_as_go_float(s: &str) -> bool {
    if is_go_hex_float(s) {
        return true;
    }
    let (signed, body) = match s.as_bytes().first() {
        Some(b'+' | b'-') => (true, &s[1..]),
        _ => (false, s),
    };
    // Go rejects a signed NaN; Rust's f64 parser accepts it.
    if signed && body.eq_ignore_ascii_case("nan") {
        return false;
    }
    s.parse::<f64>().is_ok()
}

/// `[+-]? 0x hexdigits [. hexdigits] [pP] [+-]? decdigits`, with mantissa
/// digits required on at least one side of the dot and the exponent required.
fn is_go_hex_float(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    if !(b.len() > i + 1 && b[i] == b'0' && matches!(b[i + 1], b'x' | b'X')) {
        return false;
    }
    i += 2;
    let mut mantissa_digits = 0;
    while i < b.len() && b[i].is_ascii_hexdigit() {
        i += 1;
        mantissa_digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_hexdigit() {
            i += 1;
            mantissa_digits += 1;
        }
    }
    if mantissa_digits == 0 || i >= b.len() || !matches!(b[i], b'p' | b'P') {
        return false;
    }
    i += 1;
    if i < b.len() && matches!(b[i], b'+' | b'-') {
        i += 1;
    }
    let mut exponent_digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        exponent_digits += 1;
    }
    exponent_digits > 0 && i == b.len()
}

/// Mirror of the Go regex `^[+-]?[1-9][0-9_]*(:[0-5]?[0-9])+(\.[0-9_]*)?$`
/// (applied to the original string, underscores intact).
fn is_sexagesimal(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    if !(i < b.len() && (b'1'..=b'9').contains(&b[i])) {
        return false;
    }
    i += 1;
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
        i += 1;
    }
    let mut groups = 0;
    while i < b.len() && b[i] == b':' {
        i += 1;
        if i >= b.len() || !b[i].is_ascii_digit() {
            return false;
        }
        // [0-5]?[0-9]: take two digits when the first is 0-5, else one.
        if (b'0'..=b'5').contains(&b[i]) && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
            i += 2;
        } else {
            i += 1;
        }
        groups += 1;
    }
    if groups == 0 {
        return false;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
            i += 1;
        }
    }
    i == b.len()
}

/// Whether `s` matches one of the YAML timestamp layouts recognized by the Go
/// implementation (a subset of http://yaml.org/type/timestamp.html):
///   - `YYYY-M-D`
///   - `YYYY-M-D H:M:S[.frac]`               (space separator, no zone)
///   - `YYYY-M-D[Tt]H:M:S[.frac](Z|±hh:mm)`  (zone required)
///
/// with 1-2 digit month/day/hour/minute/second fields and calendar-valid
/// dates (including leap years).
fn is_timestamp(s: &str) -> bool {
    let b = s.as_bytes();

    // Fast path: all date formats start with exactly 4 digits then '-'.
    if b.len() < 5 || !b[..4].iter().all(u8::is_ascii_digit) || b[4] != b'-' {
        return false;
    }
    let year: i32 = s[..4].parse().unwrap();

    let mut i = 5;
    let Some(month) = take_1_2_digits(b, &mut i) else {
        return false;
    };
    if !(1..=12).contains(&month) || i >= b.len() || b[i] != b'-' {
        return false;
    }
    i += 1;
    let Some(day) = take_1_2_digits(b, &mut i) else {
        return false;
    };
    if day < 1 || day > days_in_month(year, month) {
        return false;
    }
    if i == b.len() {
        return true; // date-only
    }

    let sep = b[i];
    if !matches!(sep, b'T' | b't' | b' ') {
        return false;
    }
    i += 1;

    let Some(hour) = take_1_2_digits(b, &mut i) else {
        return false;
    };
    if hour > 23 || i >= b.len() || b[i] != b':' {
        return false;
    }
    i += 1;
    let Some(minute) = take_1_2_digits(b, &mut i) else {
        return false;
    };
    if minute > 59 || i >= b.len() || b[i] != b':' {
        return false;
    }
    i += 1;
    let Some(second) = take_1_2_digits(b, &mut i) else {
        return false;
    };
    if second > 59 {
        return false;
    }

    // Optional fraction: '.' + 1..=9 digits.
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let mut digits = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
        if digits == 0 || digits > 9 {
            return false;
        }
    }

    if sep == b' ' {
        // Space-separated layout has no zone.
        return i == b.len();
    }

    // T-separated layouts require a zone: 'Z' or ±hh:mm.
    if i < b.len() && b[i] == b'Z' {
        return i + 1 == b.len();
    }
    if i < b.len() && matches!(b[i], b'+' | b'-') {
        i += 1;
        let rest = &b[i..];
        return rest.len() == 5
            && rest[0].is_ascii_digit()
            && rest[1].is_ascii_digit()
            && rest[2] == b':'
            && rest[3].is_ascii_digit()
            && rest[4].is_ascii_digit();
    }
    false
}

fn take_1_2_digits(b: &[u8], i: &mut usize) -> Option<u32> {
    let start = *i;
    while *i < b.len() && *i - start < 2 && b[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i == start {
        return None;
    }
    std::str::from_utf8(&b[start..*i]).unwrap().parse().ok()
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap { 29 } else { 28 }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quoted(val: &str, compact: bool) -> String {
        let mut out = String::new();
        render_string(
            val,
            0,
            Flags {
                compact,
                lazy_quote: false,
            },
            &mut out,
        );
        out
    }

    /// Port of Go's TestRenderStringEscapes: (input rune, regular, compact).
    #[test]
    fn escapes() {
        let cases: &[(&str, &str, &str)] = &[
            ("\\", "\"\\\\\"", "\"\\\\\""),
            ("\"", "\"\\\"\"", "\"\\\"\""),
            ("\u{07}", "\"\\a\"", "\"\\a\""),
            ("\u{08}", "\"\\b\"", "\"\\b\""),
            ("\u{0c}", "\"\\f\"", "\"\\f\""),
            ("\n", "\"\\\n \\n\\\n\"", "\"\\n\""),
            ("\r", "\"\\r\"", "\"\\r\""),
            ("\t", "\"\t\"", "\"\\t\""),
            ("\u{0b}", "\"\\v\"", "\"\\v\""),
            ("\0", "\"\\0\"", "\"\\0\""),
            ("\u{1b}", "\"\\e\"", "\"\\e\""),
            ("\u{85}", "\"\\N\"", "\"\\N\""),
            ("\u{a0}", "\"\\_\"", "\"\\_\""),
            ("\u{2028}", "\"\\L\"", "\"\\L\""),
            ("\u{2029}", "\"\\P\"", "\"\\P\""),
            ("\u{01}", "\"\\x01\"", "\"\\x01\""),
            ("\u{7f}", "\"\\x7f\"", "\"\\x7f\""),
            ("\u{ffff}", "\"\\uffff\"", "\"\\uffff\""),
            ("\u{10ffff}", "\"\\U0010ffff\"", "\"\\U0010ffff\""),
            // IsPrint divergence probes: these must pass through literally.
            ("\u{1f600}", "\"\u{1f600}\"", "\"\u{1f600}\""),
            ("é", "\"é\"", "\"é\""),
            ("\u{0301}", "\"\u{0301}\"", "\"\u{0301}\""),
            // ...and these are non-print despite being "just text" chars.
            ("\u{ad}", "\"\\u00ad\"", "\"\\u00ad\""),
            ("\u{200b}", "\"\\u200b\"", "\"\\u200b\""),
            ("\u{feff}", "\"\\ufeff\"", "\"\\ufeff\""),
            ("\u{e000}", "\"\\ue000\"", "\"\\ue000\""),
        ];
        for (input, regular, compact) in cases {
            assert_eq!(&quoted(input, false), regular, "regular {input:?}");
            assert_eq!(&quoted(input, true), compact, "compact {input:?}");
        }
    }

    /// Port of Go's TestIsTypeAmbiguous.
    #[test]
    fn type_ambiguous() {
        let unambiguous = [
            "regular",
            "abc123",
            "hello_world",
            "hello-world",
            "1:60",
            "12345:60.00",
        ];
        for s in unambiguous {
            assert!(!is_type_ambiguous(s), "{s:?} should not be ambiguous");
        }

        let ambiguous = [
            // bools (incl. Go ParseBool's t/f and YAML 1.1 y/n/on/off)
            "yes",
            "YES",
            "y",
            "Y",
            "no",
            "NO",
            "n",
            "N",
            "on",
            "ON",
            "off",
            "OFF",
            "t",
            "T",
            "f",
            "F",
            "true",
            "True",
            "TRUE",
            "false",
            "False",
            "FALSE",
            // ints
            "1234",
            "_1_2_3_4_",
            "0123",
            "+123",
            "-123",
            "0777",
            "0o777",
            "0xff",
            "0xFF",
            "0",
            "1",
            // big decimal is float-like even though it overflows i64
            "123456789012345678901234567890",
            // floats
            ".inf",
            "-.inf",
            "+.inf",
            ".nan",
            ".INF",
            ".NAN",
            "1e10",
            "1E10",
            "3.14",
            // timestamps
            "2006",
            "2006-1-2",
            "2006-1-2T15:4:5.999999999-08:00",
            "2006-1-2t15:4:5.999999999-08:00",
            "2006-1-2 14:4:5.999999999",
            // sexagesimal
            "1:00",
            "12345:59",
            "1:2:3:4:5:00:00:01",
            "1:00.0",
            "12345:59.59",
            "1:2:3:4:5:00:00:01.02",
            // null
            "null",
            "Null",
            "NULL",
            "~",
        ];
        for s in ambiguous {
            assert!(is_type_ambiguous(s), "{s:?} should be ambiguous");
        }
    }

    #[test]
    fn timestamps() {
        assert!(is_timestamp("2001-12-14"));
        assert!(is_timestamp("2001-2-3"));
        assert!(is_timestamp("2024-02-29")); // leap year
        assert!(is_timestamp("2000-2-29")); // leap year (400 rule)
        assert!(!is_timestamp("2023-02-29"));
        assert!(!is_timestamp("1900-2-29")); // 100 rule
        assert!(!is_timestamp("2001-13-01"));
        assert!(!is_timestamp("2001-2-30"));
        assert!(is_timestamp("2001-12-14t21:59:43.10-05:00"));
        assert!(is_timestamp("2001-12-14T21:59:43Z"));
        assert!(is_timestamp("2001-12-14 21:59:43.10"));
        assert!(!is_timestamp("2001-12-14 21:59:43.10 -5"));
        assert!(!is_timestamp("2001-12-14T21:59:43")); // T requires zone
        assert!(!is_timestamp("2001-12-14z")); // not a timestamp at all
        assert!(!is_timestamp("20011-1-1")); // 5-digit year
    }

    #[test]
    fn quoting() {
        let bare = [
            "_",
            "_with_underscore_",
            "safe",
            "with-dash",
            "with.dot",
            "with/slash",
            "unambiguous",
            "e",
        ];
        for s in bare {
            assert!(!needs_quotes(s), "{s:?} should be bare");
        }
        let quoted = [
            "",
            "-",
            "-leading-dash",
            "trailing-dash-",
            ".",
            ".leading.dot",
            "trailing.dot.",
            "/",
            "/leading/slash",
            "trailing/slash/",
            "not safe",
            "with\\backslash",
            "null",
            "~",
            "yes",
            "t",
            "f",
            "1",
            "-1",
            "_1",
            "__1__2__",
            "11:00",
            "2006",
            ".inf",
            "3.14",
        ];
        for s in quoted {
            assert!(needs_quotes(s), "{s:?} should be quoted");
        }
    }

    #[test]
    fn go_parse_mirrors() {
        // second sign must be rejected
        assert!(!parses_as_go_int("+-1"));
        assert!(!parses_as_go_int("--1"));
        // prefix without digits
        assert!(!parses_as_go_int("0x"));
        // legacy octal: "08" is not valid octal
        assert!(!parses_as_go_int("08"));
        assert!(parses_as_go_int("0"));
        assert!(parses_as_go_int("0777"));
        assert!(parses_as_go_int("-0x7fffffffffffffff"));
        // i64 overflow is not int-like (mirrors Go's ErrRange handling)
        assert!(!parses_as_go_int("0xffffffffffffffffff"));

        // hex floats need the exponent
        assert!(parses_as_go_float("0x1p4"));
        assert!(parses_as_go_float("0x1.8p-2"));
        assert!(parses_as_go_float("0x.8p1"));
        assert!(!parses_as_go_float("0x1f"));
        assert!(!parses_as_go_float("0x1p"));
        // signed nan is rejected, like Go
        assert!(!parses_as_go_float("-nan"));
        assert!(parses_as_go_float("nan"));
        assert!(parses_as_go_float("-inf"));
        assert!(parses_as_go_float("Infinity"));

        assert!(is_sexagesimal("1:00"));
        assert!(is_sexagesimal("+1:2:3"));
        assert!(!is_sexagesimal("1:60"));
        assert!(!is_sexagesimal("0:00")); // must start [1-9]
        assert!(!is_sexagesimal("1:234"));
    }
}
