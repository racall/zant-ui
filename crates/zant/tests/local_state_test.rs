//! `ui::local_state` slots live while
//! their instance keeps being built, are freed on the rebuild that drops it
//! and survive `ui::memo` skips.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use zant::runtime::Runtime;
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::memo::memo;
use zant::ui::options::{BoxOptions, MemoOptions, TextOptions};
use zant::ui::{child_id, column, local_state, text};

thread_local! {
    static UNMOUNTED: Cell<u32> = const { Cell::new(0) };
}

struct Counter {
    clicks: u32,
}

impl Counter {
    fn new() -> Counter {
        Counter { clicks: 0 }
    }
}

impl Drop for Counter {
    fn drop(&mut self) {
        UNMOUNTED.with(|c| c.set(c.get() + 1));
    }
}

struct Inner {
    rows: Vec<u64>,
    memo_hash: u64,
    in_memo: bool,
    seen: [Option<Weak<RefCell<Counter>>>; 8],
}

impl Inner {
    fn new(rows: &[u64]) -> Inner {
        Inner {
            rows: rows.to_vec(),
            memo_hash: 1,
            in_memo: false,
            seen: std::array::from_fn(|_| None),
        }
    }

    fn row(&mut self, id: u64, index: usize) -> Node {
        let state = local_state(child_id(0, id), Counter::new);
        self.seen[index] = Some(Rc::downgrade(&state));
        text("row", TextOptions::default())
    }
}

/// Builds one frame from the shared inner state (the memo build needs an `Rc`
/// to the same state).
fn build(inner: &Rc<RefCell<Inner>>) -> Node {
    let mut children = Vec::new();
    let (in_memo, memo_hash) = {
        let mut guard = inner.borrow_mut();
        let rows = guard.rows.clone();
        for (i, id) in rows.iter().enumerate() {
            children.push(guard.row(*id, i));
        }
        (guard.in_memo, guard.memo_hash)
    };
    if in_memo {
        let memo_inner = Rc::clone(inner);
        children.push(memo(
            MemoOptions {
                key: Some("memo".into()),
                hash: memo_hash,
            },
            move || {
                let mut guard = memo_inner.borrow_mut();
                column(BoxOptions {
                    children: vec![guard.row(99, 7)],
                    ..Default::default()
                })
            },
        ));
    }
    column(BoxOptions {
        children,
        ..Default::default()
    })
}

struct State {
    inner: Rc<RefCell<Inner>>,
}

impl App for State {
    fn render(&mut self) -> Node {
        build(&self.inner)
    }
}

fn clicks(inner: &Rc<RefCell<Inner>>, index: usize) -> u32 {
    let state = inner.borrow().seen[index]
        .as_ref()
        .expect("seen slot")
        .upgrade()
        .expect("slot alive");
    state.borrow().clicks
}

fn set_clicks(inner: &Rc<RefCell<Inner>>, index: usize, clicks: u32) {
    let state = inner.borrow().seen[index]
        .as_ref()
        .expect("seen slot")
        .upgrade()
        .expect("slot alive");
    state.borrow_mut().clicks = clicks;
}

fn seen(inner: &Rc<RefCell<Inner>>, index: usize) -> Weak<RefCell<Counter>> {
    inner.borrow().seen[index].clone().expect("seen slot")
}

#[test]
fn local_state_persists_across_rebuilds_and_frees_on_unmount() {
    UNMOUNTED.with(|c| c.set(0));
    let inner = Rc::new(RefCell::new(Inner::new(&[1, 2, 3])));
    let mut runtime = Runtime::init(
        State {
            inner: Rc::clone(&inner),
        },
        RunOptions::default(),
    );

    runtime.rebuild_for_test();
    assert_eq!(runtime.local_states.borrow().count(), 3);
    set_clicks(&inner, 1, 5);
    let second = seen(&inner, 1);

    runtime.rebuild_for_test();
    assert!(Weak::ptr_eq(&second, &seen(&inner, 1)));
    assert_eq!(clicks(&inner, 1), 5);

    // row 2 leaves: its slot is freed after that rebuild
    inner.borrow_mut().rows = vec![1, 3];
    runtime.rebuild_for_test();
    assert_eq!(runtime.local_states.borrow().count(), 2);
    assert_eq!(UNMOUNTED.with(|c| c.get()), 1);

    // coming back is a fresh mount
    inner.borrow_mut().rows = vec![1, 2, 3];
    runtime.rebuild_for_test();
    assert_eq!(clicks(&inner, 1), 0);
}

#[test]
fn local_state_inside_a_skipped_memo_stays_mounted() {
    UNMOUNTED.with(|c| c.set(0));
    let inner = Rc::new(RefCell::new(Inner::new(&[])));
    {
        let mut guard = inner.borrow_mut();
        guard.in_memo = true;
    }
    let mut runtime = Runtime::init(
        State {
            inner: Rc::clone(&inner),
        },
        RunOptions::default(),
    );

    runtime.rebuild_for_test();
    set_clicks(&inner, 7, 4);
    let inside = seen(&inner, 7);

    // same hash: the memo builder does not run, the slot is kept
    runtime.rebuild_for_test();
    runtime.rebuild_for_test();
    assert_eq!(runtime.local_states.borrow().count(), 1);
    assert_eq!(UNMOUNTED.with(|c| c.get()), 0);

    // a new hash rebuilds it with the same slot
    inner.borrow_mut().memo_hash = 2;
    runtime.rebuild_for_test();
    assert!(Weak::ptr_eq(&inside, &seen(&inner, 7)));
    assert_eq!(clicks(&inner, 7), 4);

    // the memo leaves
    inner.borrow_mut().in_memo = false;
    runtime.rebuild_for_test();
    assert_eq!(runtime.local_states.borrow().count(), 0);
    assert_eq!(UNMOUNTED.with(|c| c.get()), 1);
}
