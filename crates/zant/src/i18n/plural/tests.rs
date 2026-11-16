use super::operands::js_string;
use super::{Category, Operands, Rules, cldr_rules, data};

fn locale(tag: &str) -> Rules {
    Rules::parse(&cldr_rules(tag).unwrap()).unwrap()
}

fn sample_parts(text: &str) -> (u64, usize, &str) {
    let (mantissa, exponent) = text
        .find(['c', 'e'])
        .map_or((text, ""), |at| (&text[..at], &text[at..]));
    let precision = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    (
        mantissa.replace('.', "").parse().unwrap(),
        precision,
        exponent,
    )
}

fn expand_sample(sample: &str) -> Vec<String> {
    let Some((start, end)) = sample.split_once('~') else {
        return vec![sample.into()];
    };
    let (start, precision, exponent) = sample_parts(start);
    let (end, end_precision, end_exponent) = sample_parts(end);
    assert_eq!((precision, exponent), (end_precision, end_exponent));
    assert!(start <= end);
    let scale = 10_u64.pow(precision as u32);
    (start..=end)
        .map(|value| {
            if precision == 0 {
                format!("{value}{exponent}")
            } else {
                format!("{}.{:0precision$}{exponent}", value / scale, value % scale)
            }
        })
        .collect()
}

#[test]
fn every_cldr_sample() {
    // Read the original document independently so the test also detects locales
    // or categories accidentally omitted by the embedded-data loader.
    let json: serde_json::Value = serde_json::from_str(data::JSON).unwrap();
    let locales = json["supplemental"]["plurals-type-cardinal"]
        .as_object()
        .unwrap();
    let mut categories = 0;
    let mut vectors = 0;
    for (tag, entries) in locales {
        let rules = locale(tag);
        let entries = entries.as_object().unwrap();
        assert_eq!(rules.categories().count(), entries.len(), "{tag}");
        for (key, text) in entries {
            categories += 1;
            let expected =
                Category::from_str(key.strip_prefix("pluralRule-count-").unwrap()).unwrap();
            for samples in text.as_str().unwrap().split('@').skip(1) {
                let list = samples
                    .strip_prefix("integer")
                    .or_else(|| samples.strip_prefix("decimal"))
                    .expect("known CLDR sample type");
                for sample in list.split(',').map(str::trim) {
                    if matches!(sample, "…" | "...") {
                        continue;
                    }
                    for text in expand_sample(sample) {
                        let operands = Operands::from_decimal(&text).unwrap();
                        assert_eq!(rules.select(&operands), expected, "{tag}: {text}");
                        vectors += 1;
                    }
                }
            }
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(rules.select(&Operands::from_f64(value)), Category::Other);
        }
    }
    eprintln!(
        "CLDR conformance: {} locales, {categories} categories, {vectors} sample vectors",
        locales.len()
    );
    assert_eq!(locales.len(), 225);
    assert!(vectors > 10_000);
}

#[test]
fn f64_spelling_and_operands() {
    for (value, text, integer, fraction, f, t) in [
        (0.0, "0", "0", "", 0, 0),
        (1.0, "1", "1", "", 0, 0),
        (-1.0, "-1", "1", "", 0, 0),
        (1.5, "1.5", "1", "5", 5, 5),
        (
            0.1 + 0.2,
            "0.30000000000000004",
            "0",
            "30000000000000004",
            30_000_000_000_000_004,
            30_000_000_000_000_004,
        ),
        (1e21, "1e+21", "1000000000000000000000", "", 0, 0),
        (1e-7, "1e-7", "0", "0000001", 1, 1),
        (123456789012.0, "123456789012", "123456789012", "", 0, 0),
        (-0.0, "0", "0", "", 0, 0),
        (1e-6, "0.000001", "0", "000001", 1, 1),
        (
            1e20,
            "100000000000000000000",
            "100000000000000000000",
            "",
            0,
            0,
        ),
        (1e23, "1e+23", "100000000000000000000000", "", 0, 0),
        (
            1000000000000000100.0,
            "1000000000000000100",
            "1000000000000000100",
            "",
            0,
            0,
        ),
        (
            1.0000000000000002,
            "1.0000000000000002",
            "1",
            "0000000000000002",
            2,
            2,
        ),
    ] {
        assert_eq!(js_string(value), text, "{value:?}");
        let operands = Operands::from_f64(value);
        assert!(operands.finite);
        assert_eq!(operands.integer, integer);
        assert_eq!(operands.fraction, fraction);
        assert_eq!(operands.integer_value(b'i', None), integer.parse().ok());
        assert_eq!(
            operands.integer_value(b'v', None),
            Some(fraction.len() as u64)
        );
        assert_eq!(
            operands.integer_value(b'w', None),
            Some(fraction.trim_end_matches('0').len() as u64)
        );
        assert_eq!(operands.integer_value(b'f', None), Some(f));
        assert_eq!(operands.integer_value(b't', None), Some(t));
        assert_eq!(operands.integer_value(b'c', None), Some(0));
        assert_eq!(operands.integer_value(b'e', None), Some(0));
        assert_eq!(
            operands.integer_value(b'n', None),
            if t == 0 { integer.parse().ok() } else { None }
        );
    }
    for (value, text) in [
        (f64::NAN, "NaN"),
        (f64::INFINITY, "Infinity"),
        (f64::NEG_INFINITY, "-Infinity"),
    ] {
        assert_eq!(js_string(value), text);
        assert!(!Operands::from_f64(value).finite);
    }
    assert_eq!(js_string(f64::from_bits(1)), "5e-324");
    assert_eq!(js_string(f64::MAX), "1.7976931348623157e+308");
    assert!(Operands::from_f64(f64::from_bits(1)).finite);
    assert!(Operands::from_f64(f64::MAX).finite);
    assert_eq!(locale("en").select(&Operands::from_f64(1.0)), Category::One);
    assert_eq!(
        locale("en").select(&Operands::from_decimal("1.0").unwrap()),
        Category::Other
    );
}

#[test]
fn decimal_operands() {
    for (text, integer, fraction, c) in [
        ("-1.230", "1", "230", 0),
        ("+0001.03", "1", "03", 0),
        ("1c6", "1000000", "", 6),
        ("1.1c6", "1100000", "", 6),
        ("1.20050c3", "1200", "50", 3),
        ("1.0000001c6", "1000000", "1", 6),
        ("1e-7", "0", "0000001", 0),
        ("1.20e+1", "12", "0", 0),
    ] {
        let operands = Operands::from_decimal(text).unwrap();
        assert_eq!(operands.integer, integer);
        assert_eq!(operands.fraction, fraction);
        assert_eq!(operands.c, c);
        assert_eq!(operands.integer_value(b'e', None), Some(u64::from(c)));
    }
    for text in [
        "", " ", "NaN", "inf", "Infinity", "1.", ".1", "1.2.3", "1c-1", "1e", "1c2e3", "--1",
        "1e32768", "1 2",
    ] {
        assert!(Operands::from_decimal(text).is_none(), "{text}");
    }
    let rules = Rules::parse(&[(Category::One, "w = 2 and f = 230 and t = 23")]).unwrap();
    assert_eq!(
        rules.select(&Operands::from_decimal("1.230").unwrap()),
        Category::One
    );
}

#[test]
fn locale_fallbacks() {
    assert_eq!(cldr_rules("en"), cldr_rules("en-US"));
    assert_eq!(cldr_rules("zh"), cldr_rules("zh-Hans-CN"));
    assert_eq!(cldr_rules("zh"), cldr_rules("zh-TW"));
    assert_eq!(cldr_rules("pt"), cldr_rules("pt-BR"));
    assert_ne!(cldr_rules("pt-PT"), cldr_rules("pt-BR"));
    assert_eq!(
        locale("pt-PT").select(&Operands::from_f64(0.0)),
        Category::Other
    );
    assert_eq!(
        locale("pt-BR").select(&Operands::from_f64(0.0)),
        Category::One
    );
    assert_eq!(cldr_rules("kok-Latn-IN"), cldr_rules("kok-Latn"));
    assert_eq!(cldr_rules("PT-pt"), cldr_rules("pt-PT"));
    assert_eq!(cldr_rules("pt-PT-u-nu-latn"), cldr_rules("pt-PT"));
    assert_eq!(cldr_rules("xx"), None);
}

#[test]
fn grammar_and_semantics() {
    let spaced = Rules::parse(&[
        (Category::One, "\u{200e}n\u{200f}=\u{85}1\u{2028}"),
        (Category::Other, "\u{200e}@integer ignored"),
    ])
    .unwrap();
    assert_eq!(spaced.select(&Operands::from_f64(1.0)), Category::One);
    let rules = Rules::parse(&[
        (Category::Few, "n mod 10 = 2..4,7,9..11"),
        (Category::One, "n=1 or i=2 and v=1 @integer ignored"),
        (Category::Other, "@decimal ignored"),
    ])
    .unwrap();
    assert_eq!(
        rules.categories().collect::<Vec<_>>(),
        [Category::One, Category::Few, Category::Other]
    );
    for (text, expected) in [
        ("1.00", Category::One),
        ("2.0", Category::One),
        ("2", Category::Few),
        ("2.1", Category::One),
        ("2.10", Category::Other),
        ("3.5", Category::Other),
        ("7", Category::Few),
        ("19", Category::Few),
    ] {
        assert_eq!(
            rules.select(&Operands::from_decimal(text).unwrap()),
            expected,
            "{text}"
        );
    }
    let rules = Rules::parse(&[(Category::Few, "i = 0..3"), (Category::One, "i = 1")]).unwrap();
    assert_eq!(rules.select(&Operands::from_f64(1.0)), Category::One);
    let rules = Rules::parse(&[(Category::One, "n != 2..4")]).unwrap();
    assert_eq!(
        rules.select(&Operands::from_decimal("3.5").unwrap()),
        Category::One
    );
    let rules = Rules::parse(&[(Category::One, "i % 10 = 1")]).unwrap();
    assert_eq!(
        rules.select(&Operands::from_decimal("100000000000000000000000001").unwrap()),
        Category::One
    );
    let rules = Rules::parse(&[(Category::One, "n = 1")]).unwrap();
    assert_eq!(
        rules.select(&Operands::from_decimal("1.00000000000000000001").unwrap()),
        Category::Other
    );
    let rules = Rules::parse(&[(Category::One, "i % 18446744073709551615 = 1")]).unwrap();
    assert_eq!(
        rules.select(&Operands::from_decimal("18446744073709551616").unwrap()),
        Category::One
    );
    let rules = Rules::parse(&[(Category::One, "c = 6 and e = 6")]).unwrap();
    assert_eq!(
        rules.select(&Operands::from_decimal("1c6").unwrap()),
        Category::One
    );
    assert_eq!(
        Rules::parse(&[]).unwrap().categories().collect::<Vec<_>>(),
        [Category::Other]
    );
    for category in Category::ALL {
        assert_eq!(Category::from_str(category.as_str()), Some(category));
        assert_eq!(category.as_str().parse::<Category>(), Ok(category));
    }
    assert_eq!(Category::from_str("unknown"), None);
}

#[test]
fn parse_errors() {
    for text in [
        "x = 1",
        "n =",
        "n in 2..4",
        "n is 1",
        "n is not 1",
        "n not in 1",
        "n within 1..2",
        "n == 1",
        "n % 0 = 1",
        "n % = 1",
        "n = 4..2",
        "n = 1,",
        "n = 1 and",
        "",
        "@integer 1",
        "n = 1.5",
        "n = -1",
        "n = 18446744073709551616",
        "n = 1 orx i = 2",
        "nn = 1",
    ] {
        assert!(Rules::parse(&[(Category::One, text)]).is_err(), "{text}");
    }
    assert!(Rules::parse(&[(Category::Other, "n = 1")]).is_err());
    assert!(Rules::parse(&[(Category::One, "n = 1"), (Category::One, "n = 2")]).is_err());
    assert!(Rules::parse(&[(Category::Other, ""), (Category::Other, "")]).is_err());
}
