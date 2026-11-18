//! Unit tests.

use super::*;
use std::cell::Cell as StdCell;

fn eq_str(expected: &str, got: Str) {
    assert_eq!(expected, got.as_str());
}

#[test]
fn tray_menu_items_run_their_callbacks_by_id() {
    let hits = Rc::new(StdCell::new([0u32; 3]));
    let counter = |index: usize| {
        let hits = Rc::clone(&hits);
        Action::new(move |_| {
            let mut values = hits.get();
            values[index] += 1;
            hits.set(values);
        })
    };
    let mut tray = Tray {
        applied: None,
        selects: vec![Some(counter(0)), None, Some(counter(1))],
        click_action: Some(counter(2)),
        failed: false,
    };
    let mut root = ();
    let mut cx = ActionCx::new(&mut root);
    tray.select(&mut cx, 1);
    tray.select(&mut cx, 2);
    tray.select(&mut cx, 3);
    tray.select(&mut cx, 9);
    tray.click(&mut cx);
    assert_eq!(hits.get(), [1, 1, 1]);
}

#[test]
fn tray_changes_are_detected_by_value() {
    let last = Applied {
        icon: b"png".to_vec(),
        icon_light: None,
        template: true,
        tooltip: String::from("App"),
        items: vec![AppliedItem {
            title: String::from("Open"),
            checked: false,
            disabled: false,
            separator: false,
        }],
    };
    let open = TrayOptions {
        icon: b"png",
        template: true,
        tooltip: Str::from("App"),
        items: vec![TrayItem {
            title: Str::from("Open"),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(same(&last, &open));
    let quit = TrayOptions {
        items: vec![TrayItem {
            title: Str::from("Quit"),
            ..Default::default()
        }],
        ..open.clone()
    };
    assert!(!same(&last, &quit));
    let renamed = TrayOptions {
        tooltip: Str::from("App 2"),
        ..open
    };
    assert!(!same(&last, &renamed));
    let light = TrayOptions {
        icon_light: Some(b"light"),
        ..quit
    };
    assert!(!same(&last, &light));
}

#[test]
fn hotkey_events_reach_the_hook_with_that_id() {
    set_system_enabled(false);
    let hits = Rc::new(StdCell::new(0u32));
    let counter = Rc::clone(&hits);
    let mut hotkey = Hotkey::default();
    let mut root = ();
    let mut cx = ActionCx::new(&mut root);
    assert!(hotkey.update(
        "ctrl+alt+k",
        Action::new(move |_| counter.set(counter.get() + 1)),
    ));
    let id = hotkey.id();
    assert!(hotkey_pressed(&mut cx, id));
    assert!(!hotkey_pressed(&mut cx, id + 100));
    assert_eq!(hits.get(), 1);
    // the same shortcut is not re-registered
    assert!(hotkey.update("ctrl+alt+k", Action::new(|_| {})));
    drop(hotkey);
    assert!(!hotkey_pressed(&mut cx, id), "dropping unregisters");
    assert!(!Hotkey::default().update("not a shortcut+", Action::new(|_| {})));
    assert!(!Hotkey::default().update("", Action::new(|_| {})));
}

#[test]
fn a_recorded_shortcut_reaches_tsx_as_its_string() {
    let got = Rc::new(RefCell::new(String::new()));
    let sink = Rc::clone(&got);
    let action = shortcut_action(TextAction::new(move |_cx, text| {
        *sink.borrow_mut() = text.to_owned();
    }));
    let shortcut = parse_shortcut("ctrl+alt+k").expect("valid shortcut");
    let mut root = ();
    let mut cx = ActionCx::new(&mut root);
    action.call(&mut cx, shortcut);
    let text = got.borrow().clone();
    eq_str(&desktop_shortcut(shortcut).format(), Str::copy(&text));
    let parsed = parse_shortcut(&text).expect("round trip");
    assert!(parsed.eql(&shortcut));
    assert!(parse_shortcut("").is_none());
}
