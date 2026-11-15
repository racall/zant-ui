//! Process resident memory for diagnostics, sampled outside the frame loop.

/// Current resident bytes (working set on Windows), or an error if the OS
/// cannot supply a sample. This is RSS, not the process's peak or virtual size.
pub fn resident_bytes() -> std::io::Result<u64> {
    resident_bytes_impl()
}

#[cfg(target_os = "macos")]
fn resident_bytes_impl() -> std::io::Result<u64> {
    // mach_task_basic_info from mach/task_info.h. natural_t/integer_t are
    // 32-bit even on arm64; mach_vm_size_t is 64-bit.
    #[derive(Default)]
    #[repr(C)]
    struct TaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: [i32; 2],
        system_time: [i32; 2],
        policy: i32,
        suspend_count: i32,
    }
    unsafe extern "C" {
        static mach_task_self_: u32;
        fn task_info(task: u32, flavor: u32, info: *mut i32, count: *mut u32) -> i32;
    }
    let mut info = TaskBasicInfo::default();
    let mut count = (std::mem::size_of::<TaskBasicInfo>() / std::mem::size_of::<i32>()) as u32;
    // SAFETY: mach_task_self_ is the current task's send right; flavor 20 is
    // MACH_TASK_BASIC_INFO. info is a writable buffer of count integer_t's.
    let result = unsafe {
        task_info(
            mach_task_self_,
            20,
            (&mut info as *mut TaskBasicInfo).cast(),
            &mut count,
        )
    };
    if result != 0 {
        return Err(std::io::Error::other(format!("task_info failed: {result}")));
    }
    Ok(info.resident_size)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn resident_bytes_impl() -> std::io::Result<u64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("ps could not read process RSS"));
    }
    let kib = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .map_err(std::io::Error::other)?;
    kib.checked_mul(1024)
        .ok_or_else(|| std::io::Error::other("RSS overflow"))
}

#[cfg(target_os = "windows")]
fn resident_bytes_impl() -> std::io::Result<u64> {
    use std::ffi::c_void;

    // PROCESS_MEMORY_COUNTERS from psapi.h (DWORD fields followed by SIZE_T).
    #[repr(C)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(process: *mut c_void, counters: *mut Counters, cb: u32) -> i32;
    }

    // SAFETY: all fields are integers; zero is a valid initial value.
    let mut counters: Counters = unsafe { std::mem::zeroed() };
    let cb = std::mem::size_of::<Counters>() as u32;
    counters.cb = cb;
    // SAFETY: the pseudo-handle refers to this process and the writable buffer
    // is exactly cb bytes. GetCurrentProcess needs no handle cleanup.
    if unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, cb) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(counters.working_set_size as u64)
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_process_has_resident_memory() {
        assert!(super::resident_bytes().expect("process RSS") > 0);
    }
}
