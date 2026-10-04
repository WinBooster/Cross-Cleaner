//! Taskbar progress indicator (Windows: ITaskbarList3, other platforms: no-op).

#[cfg(windows)]
mod imp {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Shell::{
        ITaskbarList3, TBPF_ERROR, TBPF_INDETERMINATE, TBPF_NOPROGRESS, TBPF_NORMAL, TBPF_PAUSED,
        TBPFLAG, TaskbarList,
    };

    /// Taskbar progress state (TBPFLAG).
    #[derive(Clone, Copy, Debug)]
    #[allow(dead_code)]
    pub enum TaskbarState {
        NoProgress,
        Indeterminate,
        Normal,
        Error,
        Paused,
    }

    /// Wraps ITaskbarList3 and the main window HWND.
    /// All methods are no-ops if COM creation failed (e.g. older Windows).
    pub struct TaskbarProgress {
        taskbar: Option<ITaskbarList3>,
        hwnd: HWND,
    }

    impl TaskbarProgress {
        /// Creates the progress controller for the main eframe window.
        /// Call once from the GUI thread (e.g. the first `ui` frame).
        pub fn new(frame: &eframe::Frame) -> Self {
            let hwnd = hwnd_from_frame(frame).unwrap_or_default();
            let taskbar = Self::create_taskbar();
            Self { taskbar, hwnd }
        }

        /// Sets the progress value (completed / total).
        pub fn set_progress(&self, completed: u64, total: u64) {
            if total == 0 {
                return;
            }
            if let Some(taskbar) = &self.taskbar {
                unsafe {
                    let _ = taskbar.SetProgressValue(self.hwnd, completed.min(total), total);
                }
            }
        }

        /// Changes the taskbar progress state (color / indeterminate).
        pub fn set_state(&self, state: TaskbarState) {
            if let Some(taskbar) = &self.taskbar {
                let flag: TBPFLAG = match state {
                    TaskbarState::NoProgress => TBPF_NOPROGRESS,
                    TaskbarState::Indeterminate => TBPF_INDETERMINATE,
                    TaskbarState::Normal => TBPF_NORMAL,
                    TaskbarState::Error => TBPF_ERROR,
                    TaskbarState::Paused => TBPF_PAUSED,
                };
                unsafe {
                    let _ = taskbar.SetProgressState(self.hwnd, flag);
                }
            }
        }

        /// Hides the taskbar progress.
        pub fn remove(&self) {
            self.set_state(TaskbarState::NoProgress);
        }

        fn create_taskbar() -> Option<ITaskbarList3> {
            unsafe {
                // winit already initializes COM; S_FALSE (already initialized) is fine.
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let taskbar: ITaskbarList3 =
                    CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
                taskbar.HrInit().ok()?;
                Some(taskbar)
            }
        }
    }

    fn hwnd_from_frame(frame: &eframe::Frame) -> Option<HWND> {
        let handle = frame.window_handle().ok()?;
        match handle.as_raw() {
            RawWindowHandle::Win32(win) => Some(HWND(win.hwnd.get() as *mut core::ffi::c_void)),
            _ => None,
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// No-op on non-Windows platforms.
    #[derive(Clone, Copy, Debug)]
    #[allow(dead_code)]
    pub enum TaskbarState {
        NoProgress,
        Indeterminate,
        Normal,
        Error,
        Paused,
    }

    /// No-op stub so the same code compiles on every platform.
    #[derive(Clone, Copy, Debug)]
    pub struct TaskbarProgress;

    impl TaskbarProgress {
        #[allow(unused_variables)]
        pub fn new(frame: &eframe::Frame) -> Self {
            Self
        }

        #[allow(unused_variables)]
        pub fn set_progress(&self, completed: u64, total: u64) {}

        #[allow(unused_variables)]
        pub fn set_state(&self, state: TaskbarState) {}

        pub fn remove(&self) {}
    }
}

#[allow(unused_imports)]
pub use imp::{TaskbarProgress, TaskbarState};
