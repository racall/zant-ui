use zant::i18n::catalog::Catalog;
use zant::i18n::format::{Arg, format};

include!("../../zant-tsx/tests/fixtures/i18n_description.rs");

#[test]
fn generated_description_formats_with_the_real_runtime() {
    let catalog = Catalog::new(&DESCRIPTION);
    let en = catalog.index_of("en").unwrap();
    let zh = catalog.index_of("zh-CN").unwrap();
    let render = |chain: &[usize], key: &str, args: &[Arg<'_>]| {
        let id = catalog.keys.iter().position(|k| k.as_str() == key).unwrap();
        let (message, locale) = catalog.lookup(chain, id).unwrap();
        format(message, locale, args).as_str().to_owned()
    };
    for (n, who, expected) in [
        (0.0, "friend", "none"),
        (1.0, "friend", "1 item"),
        (2.0, "friend", "2 items for a friend"),
        (3.0, "stranger", "3 items"),
    ] {
        assert_eq!(
            render(&[en], "items", &[Arg::Num(n), Arg::Str(who)]),
            expected
        );
    }
    assert_eq!(
        render(&[zh, en], "items", &[Arg::Num(2.0), Arg::Str("friend")]),
        "朋友有 2 个"
    );
    assert_eq!(
        render(&[zh, en], "items", &[Arg::Num(1.0), Arg::Str("stranger")]),
        "1 个"
    );
    assert_eq!(
        render(&[en], "greeting", &[Arg::Num(7.0), Arg::Str("friend")]),
        "Hello 7 friend"
    );
    assert_eq!(
        render(&[en], "greeting", &[Arg::Num(8.0), Arg::Str("stranger")]),
        "Welcome 8 stranger"
    );
    assert_eq!(
        render(&[zh, en], "greeting", &[Arg::Num(7.0), Arg::Str("朋友")]),
        "朋友：7"
    );
    let (message, locale) = catalog.lookup(&[zh, en], 0).unwrap();
    assert_eq!(locale.tag.as_str(), "en");
    assert_eq!(
        format(message, locale, &[]).as_str(),
        "Quote: \" backslash: \\ newline:\n\t\r\0\u{8}\u{7f}\u{85}\u{2028}\u{2029} 中"
    );
}
