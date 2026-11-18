//! Process memory housekeeping.

/// Returns memory the allocator still holds but no longer uses to the system
/// (macOS: every malloc zone; Windows: the process heap is compacted).
///
/// Call it after dropping large short-lived buffers — screen captures,
/// decoded images, documents — so the freed blocks stop counting against the
/// app's memory. It is cheap and does nothing on other systems.
pub fn release_unused() {
    crate::platform::platform::release_unused_memory();
}

#[cfg(test)]
mod tests {
    #[test]
    fn releasing_unused_memory_keeps_live_data() {
        let big = vec![7u8; 8 << 20];
        drop(vec![1u8; 32 << 20]);
        super::release_unused();
        assert!(big.iter().all(|&b| b == 7));
    }
}
