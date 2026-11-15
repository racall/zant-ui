use zant::{
    runtime::Runtime,
    tsx::hot::{Restore, Snapshot},
    ui::{self, App},
};

#[derive(Default)]
struct Root<T: Default> {
    count: u32,
    value: T,
}
macro_rules! app {
    ($ty:ty) => {
        impl App for Root<$ty> {
            fn render(&mut self) -> ui::Node {
                let slot = ui::local_state::local_state_hot(42, || 3u32);
                ui::text(slot.borrow().to_string(), Default::default())
            }
            fn hot_save(&self) -> Option<String> {
                let mut snapshot = Snapshot::new();
                zant::hot_put!(snapshot, "count", self.count);
                zant::hot_put!(snapshot, "value", self.value);
                Some(snapshot.finish())
            }
            fn hot_load(&mut self, state: &str) {
                let restore = Restore::parse(state);
                zant::hot_get!(restore, "count", self.count);
                zant::hot_get!(restore, "value", self.value);
            }
        }
    };
}
app!(String);
app!(bool);

#[test]
fn runtime_root_snapshot_restores_and_changed_type_resets_only_one_field() {
    let old = Runtime::init(Root::<String>::default(), Default::default());
    let snapshot = {
        let mut root = old.app_state_mut::<Root<String>>();
        root.count = 19;
        root.value = "saved".into();
        root.hot_save().unwrap()
    };
    let new = Runtime::init(Root::<String>::default(), Default::default());
    let mut root = new.app_state_mut::<Root<String>>();
    root.hot_load(&snapshot);
    assert_eq!(root.count, 19);
    assert_eq!(root.value, "saved");
    drop(root);
    let changed = Runtime::init(Root::<bool>::default(), Default::default());
    let mut root = changed.app_state_mut::<Root<bool>>();
    root.hot_load(&snapshot);
    assert_eq!(root.count, 19);
    assert!(!root.value);
    root.hot_load("foreign");
    assert_eq!(root.count, 19);
}

#[test]
fn local_state_hot_restores_lazily_before_first_runtime_build() {
    let mut old = Runtime::init(Root::<bool>::default(), Default::default());
    old.rebuild_for_test();
    *old.local_states
        .borrow_mut()
        .slot_hot(42, || 0u32)
        .value
        .borrow_mut() = 27;
    // Ordinary slots are omitted.
    old.local_states.borrow_mut().slot(99, || 5u32);
    let snapshot = old.local_states.borrow().save_hot();
    assert!(snapshot.get("99").is_none());
    let mut new = Runtime::init(Root::<bool>::default(), Default::default());
    new.local_states
        .borrow_mut()
        .set_pending_hot(snapshot.clone());
    new.rebuild_for_test();
    assert_eq!(
        *new.local_states
            .borrow_mut()
            .slot_hot(42, || 0u32)
            .value
            .borrow(),
        27
    );
    let mut changed = zant::ui::LocalStateStore::new();
    changed.set_pending_hot(snapshot);
    assert!(!*changed.slot_hot(42, || false).value.borrow());
    // A pending value is consumed once.
    changed.begin_build();
    changed.sweep();
    assert_eq!(*changed.slot_hot(42, || 1u32).value.borrow(), 1);
}
