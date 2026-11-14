//! Port of `tests/store_test.zig` — the Zustand-style `zant.store.Store`.

use std::cell::Cell;
use std::rc::Rc;
use zant::store::Store;

#[derive(Default)]
struct State {
    count: u32,
    enabled: bool,
}

#[test]
fn zustand_style_store_update_and_notify() {
    let calls = Rc::new(Cell::new(0u32));
    let mut store = Store::new(State::default());
    {
        let calls = Rc::clone(&calls);
        store.subscribe(move || calls.set(calls.get() + 1));
    }

    store.update(|state| state.count = 3);
    assert_eq!(store.get().count, 3);
    assert_eq!(calls.get(), 1);

    store.get_mut().enabled = true;
    store.notify();
    assert!(store.get().enabled);
    assert_eq!(calls.get(), 2);
}
