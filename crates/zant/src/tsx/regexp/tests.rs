use super::*;
use crate::tsx::json::{JsonValue as V, parse};
#[path = "node_tests_0.rs"]
mod node0;
#[path = "node_tests_1.rs"]
mod node1;
#[path = "node_tests_2.rs"]
mod node2;

fn s(s: &str) -> Str {
    Str::copy(s)
}
fn array(v: impl IntoIterator<Item = V>) -> V {
    V::Array(v.into_iter().collect())
}
fn num(n: f64) -> V {
    V::Number(n)
}
fn optional(s: Option<Str>) -> V {
    s.map_or(V::Null, V::String)
}
fn error(e: Thrown) -> V {
    let e = e.as_error();
    array([V::String(e.name), V::String(e.message)])
}
fn snapshot(m: Option<RegExpMatch>) -> V {
    m.map_or(V::Null, |m| {
        array([
            array((0..m.len() as usize).map(|i| optional(m.get(i as f64)))),
            num(m.index()),
            V::String(m.input()),
            if m.has_groups() {
                V::Object(
                    m.0.found
                        .named_groups()
                        .map(|(name, range)| {
                            (
                                s(name),
                                optional(range.map(|r| string(&m.0.input.units[r]))),
                            )
                        })
                        .collect(),
                )
            } else {
                V::Null
            },
        ])
    })
}
fn list(v: List<Str>) -> V {
    array(v.into_iter().map(V::String))
}
fn check(op: &str, p: &str, f: &str, input: &str, extra: &str, expected: &str) {
    let expected = parse(expected).unwrap();
    let regex = RegExp::new(p, f);
    if op == "error" {
        assert_eq!(error(regex.unwrap_err()), expected);
        return;
    }
    let re = regex.unwrap();
    let input = s(input);
    let n = || extra.parse::<f64>().unwrap();
    let actual = match op {
        "exec" => snapshot(match_(&input, &re)),
        "source" => V::String(re.to_str()),
        "sequence" => {
            re.set_last_index(n());
            array((0..4).map(|_| {
                let m = re.exec(&input);
                array([m.map_or(V::Null, |m| num(m.index())), num(re.last_index())])
            }))
        }
        "match" => match_all_global(&input, &re).map_or(V::Null, list),
        "all" => {
            re.set_last_index(n());
            let matches = match_all(&input, &re).unwrap();
            array([
                array(
                    matches
                        .iter()
                        .map(|m| array([optional(m.get(0.0)), num(m.index())])),
                ),
                num(re.last_index()),
            ])
        }
        "replace" => V::String(replace(&input, &re, extra)),
        "replace_all" => V::String(replace_all(&input, &re, extra).unwrap()),
        "split" => list(split(
            &input,
            &re,
            if extra.is_empty() { None } else { Some(n()) },
        )),
        "search" => {
            re.set_last_index(n());
            array([num(search(&input, &re)), num(re.last_index())])
        }
        "all_error" => error(match_all(&input, &re).unwrap_err()),
        "replace_error" => error(replace_all(&input, &re, extra).unwrap_err()),
        "callback" => V::String(replace_with(&input, &re, |m| {
            Str::from(format!("{}{}", m.get(1.0).unwrap(), m.index()))
        })),
        "callback_all" => {
            V::String(replace_all_with(&input, &re, |m| Str::from(m.index().to_string())).unwrap())
        }
        _ => panic!("unknown operation {op}"),
    };
    assert_eq!(actual, expected, "{op} /{p}/{f}");
}

#[test]
fn handles_share_state_and_matches_are_snapshots() {
    let re = RegExp::new("(?<x>a)", "dgimsuy").unwrap();
    assert!(re.global() && re.ignore_case() && re.multiline() && re.sticky());
    assert!(re.unicode() && re.dot_all() && re.has_indices());
    assert_eq!(&*re.flags(), "dgimsuy");
    assert_eq!(&*re.source(), "(?<x>a)");
    let clone = re.clone();
    assert!(clone.test(&s("aa")));
    assert_eq!(re.last_index(), 1.0);
    let m = clone.exec(&s("aa")).unwrap().clone();
    assert_eq!(&*m.group("x").unwrap(), "a");
    assert!(m.group("missing").is_none());
    for i in [-1.0, 0.5, f64::NAN, f64::INFINITY, 99.0] {
        assert!(m.get(i).is_none());
    }
    assert!(m.has_groups());
    assert_eq!(&*m.input(), "aa");
}

#[test]
fn callbacks_run_after_matching_and_can_change_last_index() {
    // JS: let r=/a/g; 'aa'.replace(r,()=>{const i=r.lastIndex;r.lastIndex=9;return String(i)}) // '09'
    let re = RegExp::new("a", "g").unwrap();
    assert_eq!(
        &*replace_with(&s("aa"), &re, |_| {
            let old = re.last_index();
            re.set_last_index(9.0);
            Str::from(old.to_string())
        }),
        "09"
    );
    assert_eq!(re.last_index(), 9.0);
}

#[test]
fn search_and_split_preserve_last_index_bits() {
    let re = RegExp::new("a", "g").unwrap();
    for n in [-0.0, f64::NAN, -3.2] {
        re.set_last_index(n);
        assert_eq!(search(&s("ba"), &re), 1.0);
        assert_eq!(re.last_index().to_bits(), n.to_bits());
        split(&s("aba"), &re, None);
        assert_eq!(re.last_index().to_bits(), n.to_bits());
    }
}
