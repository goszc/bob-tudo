//! Background work stays below the rest of the system. The window thread is
//! raised again so the interface does not feel stuck.

pub fn relax_thread() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN, THREAD_PRIORITY_BELOW_NORMAL,
        };
        let thread = GetCurrentThread();
        let _ = SetThreadPriority(thread, THREAD_PRIORITY_BELOW_NORMAL);
        let _ = SetThreadPriority(thread, THREAD_MODE_BACKGROUND_BEGIN);
    }
}

pub fn relax_process_keep_ui() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, GetCurrentThread, SetPriorityClass, SetThreadPriority, BELOW_NORMAL_PRIORITY_CLASS,
            THREAD_PRIORITY_ABOVE_NORMAL,
        };
        let _ = SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
    }
}

pub const WORKERS: usize = 2;
