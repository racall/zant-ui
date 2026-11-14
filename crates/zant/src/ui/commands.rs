//! `ui::focus` / `ui::scrollIntoView` / `ui::scrollToEnd` — the imperative
//! command queue of `src/ui.zig:279-377` (WP-2.4).
//!
//! Commands are queued while an action or a build runs; the runtime drains
//! them after the next layout, when the nodes they name exist and have
//! bounds:
//!
//! ```ignore
//! let commands = ui::take_commands_for(runtime_ptr);
//! for command in &commands {
//!     match runtime.find_key(command.key()) {
//!         Some(id) => match command.kind { .. },
//!         // an unowned command whose key this window does not have goes
//!         // back to the queue for another window (dropped after 3 frames)
//!         None => ui::requeue_command(*command),
//!     }
//! }
//! ```
//!
//! Ported literally: a fixed capacity of 8 commands (`MAX_COMMANDS`), a
//! 96-byte key buffer (`Command::key_buf`), an owner window pointer, and the
//! age / requeue rule.
//!
//! **Owner safety:** the owner is the runtime's own pointer, exactly like
//! Zig's `?*const anyopaque`. The `ui` layer only stores and compares it — it
//! is never dereferenced, so the runtime may pass any stable per-window
//! address (`Rc::as_ptr`, a boxed runtime, …) for the lifetime of a frame.
#![forbid(unsafe_code)]

use std::cell::RefCell;

/// Zig `max_commands`: capacity of the command queue. Commands queued when it
/// is full are dropped.
pub const MAX_COMMANDS: usize = 8;

/// Zig `Command.key_buf` length: keys longer than this are dropped.
const KEY_BUF_LEN: usize = 96;

/// Zig `requeueCommand`: an unowned command is dropped after this many frames
/// of waiting for a window that has its key.
const MAX_COMMAND_AGE: u8 = 3;

/// Zig `CommandKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Focus,
    ScrollIntoView,
    ScrollToEnd,
    /// The app-drawn Windows caption buttons (`ui::window_controls`): the
    /// actions queue the window operation instead of reaching the runtime.
    WindowMinimize,
    WindowMaximize,
    WindowClose,
}

/// Imperative request queued by [`focus`] / [`scroll_into_view`] /
/// [`scroll_to_end`]; the runtime runs it after the next layout.
#[derive(Clone, Copy)]
pub struct Command {
    pub kind: CommandKind,
    /// The key copied into a fixed buffer (Zig `key_buf`).
    key_buf: [u8; KEY_BUF_LEN],
    /// Length of the key in `key_buf` (Zig `key_len: u8`).
    key_len: u8,
    /// The window (runtime) it is for: the one that was handling an event or
    /// building when it was queued; `None` = any window that has the key.
    ///
    /// This is the runtime's pointer, only compared, never dereferenced.
    pub owner: Option<*const ()>,
    /// Frames it waited for a window with its key (dropped after a few).
    pub age: u8,
}

impl Command {
    /// The node key, as a `&str` (Zig `Command.key`).
    pub fn key(&self) -> &str {
        // Keys only enter through `&str` arguments, so the bytes are valid
        // UTF-8; the copy is byte-exact.
        std::str::from_utf8(&self.key_buf[..self.key_len as usize])
            .expect("Zant UI: command key is valid UTF-8")
    }

    fn new(kind: CommandKind, owner: Option<*const ()>, key: &str) -> Command {
        let mut key_buf = [0u8; KEY_BUF_LEN];
        key_buf[..key.len()].copy_from_slice(key.as_bytes());
        Command {
            kind,
            key_buf,
            key_len: u8::try_from(key.len()).expect("Zant UI: command key fits the buffer"),
            owner,
            age: 0,
        }
    }
}

struct CommandQueue {
    commands: Vec<Command>,
    /// Zig `command_owner`: the runtime handling an event / building a frame.
    owner: Option<*const ()>,
}

impl CommandQueue {
    fn new() -> CommandQueue {
        CommandQueue {
            commands: Vec::with_capacity(MAX_COMMANDS),
            owner: None,
        }
    }
}

thread_local! {
    /// Per-thread command queue (Zig's `threadlocal var command_queue`).
    static QUEUE: RefCell<CommandQueue> = RefCell::new(CommandQueue::new());
}

/// The runtime handling an event / building a frame (set by the runtime);
/// commands queued meanwhile belong to its window. Returns the previous one.
pub fn set_command_owner(owner: Option<*const ()>) -> Option<*const ()> {
    QUEUE.with(|queue| std::mem::replace(&mut queue.borrow_mut().owner, owner))
}

/// Queues a command for a specific window (`zant.desktop.Window.focus`).
pub fn queue_command_for(owner: Option<*const ()>, kind: CommandKind, key: &str) {
    if key.len() > KEY_BUF_LEN {
        return;
    }
    QUEUE.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.commands.len() >= MAX_COMMANDS {
            return;
        }
        queue.commands.push(Command::new(kind, owner, key));
    });
}

fn queue_command(kind: CommandKind, key: &str) {
    let owner = QUEUE.with(|queue| queue.borrow().owner);
    queue_command_for(owner, kind, key);
}

/// Focuses the focusable node with `key` after the next layout (call it from
/// an action, e.g. to focus a search field when it opens).
pub fn focus(key: &str) {
    queue_command(CommandKind::Focus, key);
}

/// Scrolls the nearest scroll container so the node with `key` is at its top
/// (animated unless motion is off).
pub fn scroll_into_view(key: &str) {
    queue_command(CommandKind::ScrollIntoView, key);
}

/// Scrolls the scroll container with `key` to its end (bottom) after the next
/// layout, without animation: chat views call it while a reply streams in to
/// keep the newest text visible.
pub fn scroll_to_end(key: &str) {
    queue_command(CommandKind::ScrollToEnd, key);
}

/// Minimizes this window (the app-drawn Windows caption's button).
pub fn window_minimize() {
    queue_command(CommandKind::WindowMinimize, "");
}

/// Toggles maximize/restore (the app-drawn Windows caption's button).
pub fn window_maximize() {
    queue_command(CommandKind::WindowMaximize, "");
}

/// Closes this window (the app-drawn Windows caption's button).
pub fn window_close() {
    queue_command(CommandKind::WindowClose, "");
}

/// Moves all queued commands out of the queue (runtime).
///
/// Zig copies them into a caller-provided `[max_commands]Command` buffer
/// because it has no allocator here; Rust returns an owned `Vec`.
pub fn take_commands() -> Vec<Command> {
    QUEUE.with(|queue| std::mem::take(&mut queue.borrow_mut().commands))
}

/// Moves the commands for window `owner` (and unowned ones) out of the queue;
/// other windows' commands stay queued, in their original order.
pub fn take_commands_for(owner: *const ()) -> Vec<Command> {
    QUEUE.with(|queue| {
        let mut queue = queue.borrow_mut();
        let mut taken = Vec::new();
        // Zig keeps the other windows' commands in place and compacts the
        // array; `retain` preserves the same relative order.
        queue.commands.retain(|command| {
            let mine = command.owner.is_none() || command.owner == Some(owner);
            if mine {
                taken.push(*command);
            }
            !mine
        });
        taken
    })
}

/// Gives back an unowned command whose key this window does not have, so
/// another window can run it (dropped after a few frames).
pub fn requeue_command(command: Command) {
    if command.owner.is_some() || command.age >= MAX_COMMAND_AGE {
        return;
    }
    QUEUE.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.commands.len() >= MAX_COMMANDS {
            return;
        }
        let mut copy = command;
        copy.age += 1;
        queue.commands.push(copy);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The queue is thread-local; every test starts from a clean one.
    fn reset() {
        let _ = take_commands();
        set_command_owner(None);
    }

    #[test]
    fn window_operations_queue_without_a_key() {
        reset();
        let owner = Some(owner_of(&0u8));
        set_command_owner(owner);
        window_minimize();
        window_maximize();
        window_close();
        let commands = take_commands();
        let kinds: Vec<CommandKind> = commands.iter().map(|command| command.kind).collect();
        assert_eq!(
            vec![
                CommandKind::WindowMinimize,
                CommandKind::WindowMaximize,
                CommandKind::WindowClose
            ],
            kinds
        );
        assert!(commands.iter().all(|command| command.key().is_empty()));
        assert!(commands.iter().all(|command| command.owner == owner));
    }

    fn owner_of(test: &u8) -> *const () {
        test as *const u8 as *const ()
    }

    fn keys(commands: &[Command]) -> Vec<&str> {
        commands.iter().map(Command::key).collect()
    }

    fn kinds(commands: &[Command]) -> Vec<CommandKind> {
        commands.iter().map(|command| command.kind).collect()
    }

    #[test]
    fn commands_are_queued_in_order_and_drained() {
        reset();
        focus("a");
        scroll_into_view("bb");
        scroll_to_end("ccc");
        let queued = take_commands();
        assert_eq!(
            kinds(&queued),
            [
                CommandKind::Focus,
                CommandKind::ScrollIntoView,
                CommandKind::ScrollToEnd
            ]
        );
        assert_eq!(keys(&queued), ["a", "bb", "ccc"]);
        assert!(queued.iter().all(|command| command.owner.is_none()));
        assert!(queued.iter().all(|command| command.age == 0));
        assert!(take_commands().is_empty(), "taking drains the queue");
        reset();
    }

    #[test]
    fn queue_is_capped_at_eight_commands() {
        reset();
        for index in 0..MAX_COMMANDS + 4 {
            focus(&format!("key-{index}"));
        }
        let queued = take_commands();
        assert_eq!(queued.len(), MAX_COMMANDS);
        assert_eq!(queued[0].key(), "key-0");
        assert_eq!(queued[MAX_COMMANDS - 1].key(), "key-7");
        reset();
    }

    #[test]
    fn keys_longer_than_the_buffer_are_dropped() {
        reset();
        let longest = "k".repeat(KEY_BUF_LEN);
        let too_long = "k".repeat(KEY_BUF_LEN + 1);
        focus(&longest);
        focus(&too_long);
        let queued = take_commands();
        assert_eq!(queued.len(), 1, "only the 96-byte key fits");
        assert_eq!(queued[0].key(), longest);

        // multibyte keys are measured in bytes, like Zig
        let multibyte = "é".repeat(KEY_BUF_LEN / 2);
        assert_eq!(multibyte.len(), KEY_BUF_LEN);
        focus(&multibyte);
        let queued = take_commands();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].key(), multibyte);
        reset();
    }

    #[test]
    fn commands_default_to_the_current_owner() {
        reset();
        let a = 1u8;
        let b = 2u8;
        assert_eq!(set_command_owner(Some(owner_of(&a))), None);
        focus("a");
        assert_eq!(
            set_command_owner(Some(owner_of(&b))),
            Some(owner_of(&a)),
            "setCommandOwner returns the previous owner"
        );
        focus("b");
        assert_eq!(set_command_owner(None), Some(owner_of(&b)));

        let queued = take_commands();
        assert_eq!(queued[0].owner, Some(owner_of(&a)));
        assert_eq!(queued[1].owner, Some(owner_of(&b)));
        reset();
    }

    #[test]
    fn take_commands_for_filters_by_owner_and_keeps_others() {
        reset();
        let a = 1u8;
        let b = 2u8;
        focus("unowned");
        queue_command_for(Some(owner_of(&a)), CommandKind::Focus, "for-a");
        queue_command_for(Some(owner_of(&b)), CommandKind::ScrollToEnd, "for-b");

        let for_a = take_commands_for(owner_of(&a));
        assert_eq!(keys(&for_a), ["unowned", "for-a"]);
        assert_eq!(for_a[0].owner, None);
        assert_eq!(for_a[1].owner, Some(owner_of(&a)));

        // the other window's command stays queued
        let for_b = take_commands_for(owner_of(&b));
        assert_eq!(keys(&for_b), ["for-b"]);
        assert_eq!(for_b[0].kind, CommandKind::ScrollToEnd);
        assert!(take_commands().is_empty());

        // an unowned command is taken by whichever window drains first
        focus("any");
        assert_eq!(keys(&take_commands_for(owner_of(&a))), ["any"]);
        assert!(take_commands().is_empty());
        reset();
    }

    #[test]
    fn requeue_keeps_unowned_commands_until_the_third_retry() {
        reset();
        focus("late");
        let mut command = take_commands()[0];
        assert!(command.owner.is_none());
        // three requeues are accepted, incrementing the age each time
        for expected_age in 1..=MAX_COMMAND_AGE {
            requeue_command(command);
            let queued = take_commands();
            assert_eq!(queued.len(), 1, "the command is queued again");
            assert_eq!(queued[0].age, expected_age);
            assert_eq!(queued[0].key(), "late");
            command = queued[0];
        }
        // age 3 is the last chance: the next requeue drops it
        requeue_command(command);
        assert!(take_commands().is_empty());
        reset();
    }

    #[test]
    fn requeue_drops_owned_commands() {
        reset();
        let a = 1u8;
        queue_command_for(Some(owner_of(&a)), CommandKind::Focus, "owned");
        let command = take_commands()[0];
        requeue_command(command);
        assert!(
            take_commands().is_empty(),
            "owned commands are not shared with other windows"
        );
        reset();
    }

    #[test]
    fn requeue_drops_when_the_queue_is_full() {
        reset();
        focus("lost");
        let command = take_commands()[0];
        for index in 0..MAX_COMMANDS {
            focus(&format!("key-{index}"));
        }
        requeue_command(command);
        let queued = take_commands();
        assert_eq!(queued.len(), MAX_COMMANDS);
        assert!(
            !keys(&queued).contains(&"lost"),
            "a full queue drops the requeued command"
        );
        reset();
    }

    #[test]
    fn set_command_owner_returns_the_previous_owner() {
        reset();
        let a = 1u8;
        assert_eq!(set_command_owner(Some(owner_of(&a))), None);
        assert_eq!(set_command_owner(None), Some(owner_of(&a)));
        assert_eq!(set_command_owner(None), None);
        reset();
    }
}
