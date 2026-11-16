use super::*;
use crate::tsx::{Any, Bytes, Dict, List, Set};
use std::{cell::RefCell, rc::Rc};
include!("node_cases.rs");
include!("node_errors.rs");

#[test]
fn node_exact_stringify_and_round_trips() {
    for &(input, indent, expected) in NODE_STRINGS {
        let value = parse(input).unwrap();
        let output = stringify(&value, indent);
        assert_eq!(output.as_str(), expected, "{input} {indent:?}");
        if indent.is_none_or(|s| s.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))) {
            let reparsed = parse(&output).unwrap();
            assert_eq!(stringify(&reparsed, indent), output);
        }
    }
    assert_eq!(stringify_value(&f64::NAN, None).unwrap().as_str(), "null");
    assert_eq!(
        stringify_value(&f64::INFINITY, None).unwrap().as_str(),
        "null"
    );
    assert_eq!(
        stringify_value(&f64::NEG_INFINITY, None).unwrap().as_str(),
        "null"
    );
    assert_eq!(
        stringify_value(&(0.1 + 0.2), None).unwrap().as_str(),
        "0.30000000000000004"
    );
}

#[test]
fn node_exact_syntax_errors() {
    for &(input, expected) in NODE_ERRORS {
        let Thrown::Error(e) = parse(input).unwrap_err() else {
            panic!("expected Error");
        };
        // the inputs Node rejects are rejected here too, as a SyntaxError;
        // the wording is serde_json's (`expected` is what V8 says)
        assert_eq!(
            e.name.as_str(),
            "SyntaxError",
            "input: {input:?}, V8: {expected}"
        );
        assert!(!e.message.is_empty());
    }
}

#[test]
fn inline_json_test_suite_corpus() {
    let yes = [
        "null",
        "true",
        "false",
        "0",
        "-0",
        "123",
        "-123",
        "0.0",
        "1e9",
        "1E+9",
        "1e-9",
        " \t\n\r[1] \t\n\r",
        "[]",
        "{}",
        "[[]]",
        "{\"a\":{}}",
        "[null,true,false]",
        r#""\"\\\/\b\f\n\r\t""#,
        r#""\u0000""#,
        r#""\ud83d\ude00""#,
        "\"中文😀\"",
        r#"{"":0,"__proto__":2}"#,
        r#"{"x":1,"x":2}"#,
        "1e-400",
        "\"\u{2028}\u{2029}\"",
        "[1,2,3]",
        "[{} , []]",
        "\"\u{7f}\"",
    ];
    // serde_json rejects what Node accepts in three rare cases: numbers that
    // overflow to Infinity (`1e400`), lone surrogate escapes (`"\uD800"`)
    // and nesting deeper than 128 levels
    for input in ["1e400", "-1e400", r#""\uD800""#, r#""\uDC00""#] {
        assert!(parse(input).is_err(), "serde_json: {input:?}");
    }
    let no = [
        "",
        " ",
        "+1",
        ".1",
        "01",
        "-01",
        "1.",
        "1e",
        "1e+",
        "--1",
        "NaN",
        "Infinity",
        "undefined",
        "[1,]",
        "[,1]",
        "[1,,2]",
        "{\"a\":1,}",
        "{a:1}",
        "{'a':1}",
        "//x\n1",
        "/*x*/1",
        "[1/*x*/]",
        "true false",
        "nul",
        "False",
        "TRUE",
        "[",
        "{",
        "[1 2]",
        "{\"a\" 1}",
        "\u{feff}null",
        "\u{a0}null",
        "\u{b}null",
        "\u{c}null",
        "\"a\nb\"",
        "\"a\0b\"",
        r#""\x00""#,
        r#""\u123""#,
        r#""\uGGGG""#,
        "\"unterminated",
        "[1]x",
        "{}[]",
        "null\u{a0}",
        "0x10",
        "-",
        "[true:false]",
    ];
    for input in yes {
        assert!(parse(input).is_ok(), "y_: {input:?}");
    }
    for input in no {
        assert!(parse(input).is_err(), "n_: {input:?}");
    }
}

#[test]
fn unicode_duplicates_and_depth() {
    let value = parse(r#"{"b":1,"a":2,"b":3}"#).unwrap();
    let JsonValue::Object(fields) = value else {
        panic!()
    };
    assert_eq!(fields[0], (Str::Static("b"), JsonValue::Number(3.0)));
    assert_eq!(fields[1].0.as_str(), "a");
    for (input, expected) in [(r#""\ud83d\ude00""#, "😀"), (r#""\u4e2d\u6587""#, "中文")] {
        assert_eq!(parse_as::<Str>(input).unwrap().as_str(), expected);
    }
    // serde_json's recursion limit (128) keeps deep input off the stack
    for depth in [1, 127] {
        let input = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        let parsed = parse(&input).unwrap();
        assert_eq!(stringify(&parsed, None).as_str(), input);
    }
    for depth in [129, 100_000] {
        assert!(
            parse(&"[".repeat(depth))
                .unwrap_err()
                .to_string()
                .contains("recursion limit")
        );
    }
}

#[derive(Debug, PartialEq)]
struct Item {
    count: u32,
}
impl Json for Item {
    fn to_json(&self) -> Option<JsonValue> {
        Some(object_builder(4).field("count", &self.count).build())
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        Ok(Self {
            count: read_field(value, "count", path)?,
        })
    }
}
#[derive(Debug, PartialEq)]
struct Model {
    items: Vec<Item>,
    nullable: Option<Str>,
    optional: Option<bool>,
}
impl Json for Model {
    fn to_json(&self) -> Option<JsonValue> {
        Some(
            object_builder(4)
                .field("items", &self.items)
                .field("nullable", &self.nullable)
                .optional("optional", &self.optional)
                .build(),
        )
    }
    fn from_json(value: &JsonValue, path: &mut JsonPath) -> Result<Self, String> {
        Ok(Self {
            items: read_field(value, "items", path)?,
            nullable: read_field(value, "nullable", path)?,
            optional: read_optional(value, "optional", path)?,
        })
    }
}
#[test]
fn generated_struct_helpers_and_paths() {
    let model = Model {
        items: vec![Item { count: 3 }],
        nullable: None,
        optional: None,
    };
    let text = stringify_value(&model, None).unwrap();
    assert_eq!(text.as_str(), r#"{"items":[{"count":3}],"nullable":null}"#);
    assert_eq!(parse_as::<Model>(&text).unwrap(), model);
    assert_eq!(parse_as::<Model>(r#"{"items":[]}"#).unwrap().nullable, None);
    assert_eq!(
        parse_as::<Model>(r#"{"items":[],"optional":null}"#)
            .unwrap()
            .optional,
        None
    );
    let error =
        parse_as::<Model>(r#"{"items":[{"count":1},{"count":2},{"count":"x"}]}"#).unwrap_err();
    assert_eq!(
        error.to_string(),
        "TypeError: JSON does not match the expected type: expected a number at $.items[2].count, found a string"
    );
    assert!(
        parse_as::<Item>("{}")
            .unwrap_err()
            .to_string()
            .contains("missing field at $.count")
    );
    assert!(
        parse_as::<Item>("[]")
            .unwrap_err()
            .to_string()
            .contains("expected an object at $")
    );
    let mut path = JsonPath::new();
    let error = path
        .field("a.b", |p| {
            p.index(1, |p| bool::from_json(&JsonValue::Null, p))
        })
        .unwrap_err();
    assert_eq!(error, "expected a boolean at $[\"a.b\"][1], found null");
    assert_eq!(path.to_string(), "$");
    assert!(stringify_value(&Missing, None).is_none());
    let built = object_builder(4)
        .field("skip", &Missing)
        .field("x", &2)
        .build();
    assert_eq!(stringify(&built, None).as_str(), "{\"x\":2}");
}

fn round_trip<T: Json + PartialEq + std::fmt::Debug>(value: T) {
    let text = stringify_value(&value, None).unwrap();
    assert_eq!(parse_as::<T>(&text).unwrap(), value);
}
#[test]
fn conversions_cover_runtime_types() {
    round_trip(1.25f32);
    round_trip(1.25f64);
    round_trip(true);
    round_trip(Str::Static("中文😀"));
    round_trip("abc".to_string());
    round_trip(Some(7u8));
    round_trip(None::<u8>);
    round_trip(vec![1u8, 2]);
    round_trip(List::from_vec(vec![1u8, 2]));
    round_trip(Rc::new(RefCell::new(7u8)));
    round_trip(Bytes::from(vec![1, 2, 255]));
    round_trip(Any::from("text"));
    round_trip((true,));
    round_trip((1u8, false));
    round_trip((1u8, false, Str::Static("x")));
    round_trip((1, 2, 3, 4));
    round_trip((1, 2, 3, 4, 5));
    round_trip((1, 2, 3, 4, 5, 6));
    assert!(parse_as::<(u8, u8)>("[1]").is_err());
    assert!(parse_as::<(u8, u8)>("[1,2,3]").is_err());
    assert!(
        parse_as::<(u8, u8)>("[1,\"x\"]")
            .unwrap_err()
            .to_string()
            .contains("$[1]")
    );
    round_trip(Dict::from_entries([
        (Str::Static("z"), 2u8),
        (Str::Static("a"), 1),
    ]));
    let map = parse_as::<Dict<f64, u8>>(
        r#"{"0x10":1,"  ":2,"+3":3,"no":4,"Infinity":5,"0b11":6,"0o10":7}"#,
    )
    .unwrap();
    assert_eq!(map.get(&16.0), Some(1));
    assert_eq!(map.get(&0.0), Some(2));
    assert_eq!(map.get(&3.0), Some(6));
    assert_eq!(map.get(&8.0), Some(7));
    assert_eq!(map.get(&f64::NAN), Some(4));
    assert_eq!(map.get(&f64::INFINITY), Some(5));
    assert_eq!(
        stringify_value(&Dict::from_entries([(1e21, true)]), None)
            .unwrap()
            .as_str(),
        r#"{"1e+21":true}"#
    );
    assert_eq!(
        stringify_value(&Dict::from_entries([(Str::Static("x"), Missing)]), None)
            .unwrap()
            .as_str(),
        r#"{"x":null}"#
    );
    assert_eq!(
        stringify_value(&vec![Missing], None).unwrap().as_str(),
        "[null]"
    );
    assert_eq!(
        stringify_value(&Set::from_values([1.0, 2.0]), None)
            .unwrap()
            .as_str(),
        "{}"
    );
    assert!(
        parse_as::<Set<f64>>("{}")
            .err()
            .unwrap()
            .to_string()
            .contains("a Set cannot be read from JSON")
    );
    let error = Error::new("Error", Str::Static("oops"));
    assert_eq!(stringify_value(&error, None).unwrap().as_str(), "{}");
    assert_eq!(
        stringify_value(&Thrown::error(error), None)
            .unwrap()
            .as_str(),
        "{}"
    );
    round_trip(Thrown::Boolean(true));
    round_trip(Thrown::Number(2.5));
    round_trip(Thrown::String(Str::Static("x")));
    round_trip(Thrown::Null);
}

#[test]
fn integers_reject_fractional_nonfinite_and_rounded_overflow() {
    macro_rules! check { ($($ty:ty),*) => {$(
        round_trip(42 as $ty);
        for bad in [JsonValue::Number(1.5), JsonValue::Number(f64::INFINITY), JsonValue::Number(f64::NAN), JsonValue::String(Str::Static("1"))] {
            assert!(<$ty>::from_json(&bad, &mut JsonPath::new()).is_err());
        }
        let upper = if <$ty>::MIN == 0 { 2f64.powi(<$ty>::BITS as i32) } else { 2f64.powi(<$ty>::BITS as i32 - 1) };
        assert!(<$ty>::from_json(&JsonValue::Number(upper), &mut JsonPath::new()).is_err());
        assert!(<$ty>::from_json(&JsonValue::Number(<$ty>::MIN as f64), &mut JsonPath::new()).is_ok());
    )*}; }
    check!(
        i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
    );
    assert!(parse_as::<u64>("-1").is_err());
    assert_eq!(parse_as::<i8>("127").unwrap(), 127);
    assert!(parse_as::<i8>("128").is_err());
    assert!(parse_as::<f32>("1e100").is_err());
    assert_eq!(parse_as::<f64>("-0").unwrap().to_bits(), (-0f64).to_bits());
}

#[test]
#[ignore = "release parser throughput measurement"]
fn parse_one_megabyte_benchmark() {
    let object = r#"{"id":123,"name":"中文 and emoji 😀","active":true,"items":[1,2,3]}"#;
    let count = 1_048_576 / (object.len() + 1);
    let text = format!("[{}]", vec![object; count].join(","));
    let start = std::time::Instant::now();
    let value = parse(&text).unwrap();
    let elapsed = start.elapsed();
    println!(
        "parsed {} bytes in {:.3} ms",
        text.len(),
        elapsed.as_secs_f64() * 1000.0
    );
    assert!(matches!(value, JsonValue::Array(ref items) if items.len() == count));
    if !cfg!(debug_assertions) {
        assert!(elapsed.as_millis() < 30, "{elapsed:?}");
    }
}

/// A value `JSON.stringify` leaves out (JavaScript's `undefined`).
#[derive(Clone, Debug, Default, PartialEq)]
struct Missing;

impl Json for Missing {
    fn to_json(&self) -> Option<JsonValue> {
        None
    }
    fn from_json(_: &JsonValue, _: &mut JsonPath) -> Result<Self, String> {
        Ok(Missing)
    }
}

#[test]
fn large_objects_keep_first_positions_for_duplicate_keys() {
    let mut text = String::from("{");
    for i in 0..40 {
        text.push_str(&format!("\"k{i}\":{i},"));
    }
    text.push_str("\"k3\":-3,\"k30\":-30,\"z\":1}");
    let JsonValue::Object(entries) = parse(&text).unwrap() else {
        panic!("an object")
    };
    assert_eq!(entries.len(), 41);
    assert_eq!(entries[3], (Str::Static("k3"), JsonValue::Number(-3.0)));
    assert_eq!(entries[30], (Str::Static("k30"), JsonValue::Number(-30.0)));
    assert_eq!(entries[40].0.as_str(), "z");
}

#[test]
fn byte_arrays_decode_in_one_pass_in_any_key_order() {
    let bytes: crate::tsx::Bytes = parse_as(r#"{"1":2,"0":1,"2":255}"#).unwrap();
    assert_eq!(&bytes[..], &[1, 2, 255]);
    let missing = parse_as::<crate::tsx::Bytes>(r#"{"0":1,"2":3,"x":4}"#).unwrap_err();
    assert!(format!("{missing:?}").contains("byte index"), "{missing:?}");
    // a duplicate key replaced in place is one index
    let replaced: crate::tsx::Bytes = parse_as(r#"{"0":1,"0":2}"#).unwrap();
    assert_eq!(&replaced[..], &[2]);
    let past_end = parse_as::<crate::tsx::Bytes>(r#"{"0":1,"5":2}"#).unwrap_err();
    assert!(
        format!("{past_end:?}").contains("$[\"5\"]")
            || format!("{past_end:?}").contains("$.5")
            || format!("{past_end:?}").contains("byte index"),
        "{past_end:?}"
    );
    assert!(parse_as::<crate::tsx::Bytes>(r#"{"01":1}"#).is_err());
}
