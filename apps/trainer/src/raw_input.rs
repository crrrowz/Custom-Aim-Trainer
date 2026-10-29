//! Low-latency, dedicated Windows Raw Input mouse capture worker.
//!
//! Features:
//! 1. Clean worker thread shutdown via PostMessageW(WM_CLOSE).
//! 2. Thread priority set to THREAD_PRIORITY_TIME_CRITICAL for minimal dispatch jitter.
//! 3. Ordered event stream capturing BOTH discrete motion and mouse button transitions.
//! 4. Per-event QPC monotonic microsecond timestamps.
//! 5. True per-device tracking: records RAWINPUTHEADER.hDevice per event.
//! 6. Resolves device names via GetRawInputDeviceInfoW(RIDI_DEVICENAME).
//! 7. Discards absolute mouse packets (RDP, tablets, VMs).
//! 8. Strict foreground window HWND check: only records when target window is non-null AND focused.

use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
};
use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_NORMAL, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, RegisterRawInputDevices, HRAWINPUT,
    MOUSE_MOVE_ABSOLUTE, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_DEVNOTIFY,
    RIDEV_INPUTSINK, RIDI_DEVICENAME, RID_INPUT, RIM_TYPEMOUSE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, PostMessageW, PostQuitMessage, RegisterClassExW, TranslateMessage, MSG,
    RI_MOUSE_LEFT_BUTTON_DOWN, RI_MOUSE_LEFT_BUTTON_UP, RI_MOUSE_RIGHT_BUTTON_DOWN,
    RI_MOUSE_RIGHT_BUTTON_UP, WM_CLOSE, WM_DESTROY, WM_INPUT, WNDCLASSEXW,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawInputPayload {
    Motion { dx: i32, dy: i32 },
    LeftButtonDown,
    LeftButtonUp,
    RightButtonDown,
    RightButtonUp,
}

/// Discrete raw input event captured with microsecond resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawInputEvent {
    pub device_id: usize,
    pub t_us: u64,
    pub payload: RawInputPayload,
}

static TARGET_HWND: AtomicUsize = AtomicUsize::new(0);
static ABSOLUTE_EVENTS_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static WINDOW_FOCUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub struct RawInputWorker {
    pub receiver: Receiver<RawInputEvent>,
    pub qpc_freq: i64,
    pub qpc_start: i64,
    hwnd: isize,
    join_handle: Option<thread::JoinHandle<()>>,
}

unsafe impl Send for RawInputWorker {}
unsafe impl Sync for RawInputWorker {}

impl RawInputWorker {
    /// Update the target Bevy window HWND so GetForegroundWindow() is accurately verified per event.
    pub fn set_target_hwnd(hwnd: HWND) {
        TARGET_HWND.store(hwnd as usize, Ordering::Relaxed);
    }

    /// Update whether the game window is currently focused.
    /// When unfocused, the raw input thread priority lowers to NORMAL to save CPU/power,
    /// and restores to TIME_CRITICAL once focus is regained.
    pub fn set_window_focused(focused: bool) {
        WINDOW_FOCUSED.store(focused, Ordering::Relaxed);
    }

    /// Read whether the window focus flag is currently set for the raw input thread.
    pub fn is_window_focused() -> bool {
        WINDOW_FOCUSED.load(Ordering::Relaxed)
    }

    /// Returns the number of absolute mouse events discarded (e.g. tablet or RDP).
    pub fn absolute_events_discarded() -> u64 {
        ABSOLUTE_EVENTS_COUNT.load(Ordering::Relaxed)
    }

    /// Queries the OS for the friendly/interface name of a raw input device.
    pub fn get_device_name(device_handle: usize) -> String {
        if device_handle == 0 {
            return "Windows Injected / System Synthesized Mouse".to_string();
        }
        unsafe {
            let handle = device_handle as HANDLE;
            let mut size = 0u32;
            GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, std::ptr::null_mut(), &mut size);
            if size == 0 {
                return format!("Device_{:#x}", device_handle);
            }
            let mut buf = vec![0u16; size as usize];
            let res = GetRawInputDeviceInfoW(
                handle,
                RIDI_DEVICENAME,
                buf.as_mut_ptr() as *mut _,
                &mut size,
            );
            if res != u32::MAX && res > 0 {
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                String::from_utf16_lossy(&buf[..len])
            } else {
                format!("Device_{:#x}", device_handle)
            }
        }
    }

    /// Spawns the dedicated raw input thread and returns the worker handle.
    pub fn spawn() -> Self {
        ABSOLUTE_EVENTS_COUNT.store(0, Ordering::Relaxed);

        let mut freq = 0i64;
        let mut start = 0i64;
        unsafe {
            QueryPerformanceFrequency(&mut freq);
            QueryPerformanceCounter(&mut start);
        }

        let (tx, rx) = crossbeam_channel::unbounded();
        let (hwnd_tx, hwnd_rx) = std::sync::mpsc::channel::<isize>();

        let join_handle = thread::spawn(move || unsafe {
            // Elevate thread priority to TIME_CRITICAL for minimal dispatch jitter
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);

            let hinstance = GetModuleHandleW(std::ptr::null());
            let class_name = [
                ('A' as u16),
                ('i' as u16),
                ('m' as u16),
                ('F' as u16),
                ('o' as u16),
                ('r' as u16),
                ('g' as u16),
                ('e' as u16),
                ('R' as u16),
                ('a' as u16),
                ('w' as u16),
                0,
            ];

            let mut wc: WNDCLASSEXW = std::mem::zeroed();
            wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
            wc.lpfnWndProc = Some(raw_input_wndproc);
            wc.hInstance = hinstance;
            wc.lpszClassName = class_name.as_ptr();
            RegisterClassExW(&wc);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                (-3isize) as HWND, // HWND_MESSAGE
                0 as _,
                hinstance,
                std::ptr::null(),
            );

            // Register only mouse (usage 0x01, 0x02)
            let device = RAWINPUTDEVICE {
                usUsagePage: HID_USAGE_PAGE_GENERIC,
                usUsage: HID_USAGE_GENERIC_MOUSE,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: hwnd,
            };

            let ok =
                RegisterRawInputDevices(&device, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
            assert!(ok != 0, "Failed to register Raw Input device for mouse!");

            WORKER_CONTEXT.with(|ctx| {
                *ctx.borrow_mut() = Some(WorkerContext {
                    sender: tx,
                    qpc_freq: freq,
                    qpc_start: start,
                });
            });

            hwnd_tx.send(hwnd as isize).unwrap();

            let mut current_priority_time_critical = true;
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, 0 as HWND, 0, 0) > 0 {
                // Dynamically throttle thread priority when the game window is unfocused to save power
                let is_focused = WINDOW_FOCUSED.load(Ordering::Relaxed);
                if is_focused != current_priority_time_critical {
                    let new_priority = if is_focused {
                        THREAD_PRIORITY_TIME_CRITICAL
                    } else {
                        THREAD_PRIORITY_NORMAL
                    };
                    SetThreadPriority(GetCurrentThread(), new_priority);
                    current_priority_time_critical = is_focused;
                }

                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });

        let hwnd = hwnd_rx.recv().unwrap();

        Self {
            receiver: rx,
            qpc_freq: freq,
            qpc_start: start,
            hwnd,
            join_handle: Some(join_handle),
        }
    }
}

impl Drop for RawInputWorker {
    fn drop(&mut self) {
        unsafe {
            PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0);
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

struct WorkerContext {
    sender: Sender<RawInputEvent>,
    qpc_freq: i64,
    qpc_start: i64,
}

thread_local! {
    static WORKER_CONTEXT: std::cell::RefCell<Option<WorkerContext>> = const { std::cell::RefCell::new(None) };
}

unsafe extern "system" fn raw_input_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_INPUT {
        // Read QPC per discrete event for true microsecond dispatch timestamping
        let mut qpc_now = 0i64;
        QueryPerformanceCounter(&mut qpc_now);

        let mut raw_data: RAWINPUT = std::mem::zeroed();
        let mut data_size = std::mem::size_of::<RAWINPUT>() as u32;
        let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;

        let res = GetRawInputData(
            lparam as HRAWINPUT,
            RID_INPUT,
            &mut raw_data as *mut _ as _,
            &mut data_size,
            header_size,
        );

        if res != u32::MAX && res > 0 && raw_data.header.dwType == RIM_TYPEMOUSE {
            // Strict foreground window checking: target HWND must be known AND match foreground window
            let target = TARGET_HWND.load(Ordering::Relaxed) as HWND;
            let foreground = GetForegroundWindow();

            if target != 0 as HWND && foreground == target {
                let mouse = raw_data.data.mouse;

                // Absolute mouse check: discard absolute coordinates from tablets, RDP, or VMs
                if (mouse.usFlags & MOUSE_MOVE_ABSOLUTE) != 0 {
                    ABSOLUTE_EVENTS_COUNT.fetch_add(1, Ordering::Relaxed);
                } else {
                    let dx = mouse.lLastX;
                    let dy = mouse.lLastY;
                    let device_id = raw_data.header.hDevice as usize;

                    WORKER_CONTEXT.with(|ctx| {
                        if let Some(ctx) = ctx.borrow().as_ref() {
                            let qpc_diff = (qpc_now - ctx.qpc_start).max(0) as u128;
                            let t_us = ((qpc_diff * 1_000_000u128) / (ctx.qpc_freq as u128)) as u64;

                            // 1. Dispatch motion event if non-zero
                            if dx != 0 || dy != 0 {
                                let _ = ctx.sender.try_send(RawInputEvent {
                                    device_id,
                                    t_us,
                                    payload: RawInputPayload::Motion { dx, dy },
                                });
                            }

                            // 2. Dispatch button state transitions in exact QPC chronological sequence
                            let btn_flags = mouse.Anonymous.Anonymous.usButtonFlags;
                            if (btn_flags & (RI_MOUSE_LEFT_BUTTON_DOWN as u16)) != 0 {
                                let _ = ctx.sender.try_send(RawInputEvent {
                                    device_id,
                                    t_us,
                                    payload: RawInputPayload::LeftButtonDown,
                                });
                            }
                            if (btn_flags & (RI_MOUSE_LEFT_BUTTON_UP as u16)) != 0 {
                                let _ = ctx.sender.try_send(RawInputEvent {
                                    device_id,
                                    t_us,
                                    payload: RawInputPayload::LeftButtonUp,
                                });
                            }
                            if (btn_flags & (RI_MOUSE_RIGHT_BUTTON_DOWN as u16)) != 0 {
                                let _ = ctx.sender.try_send(RawInputEvent {
                                    device_id,
                                    t_us,
                                    payload: RawInputPayload::RightButtonDown,
                                });
                            }
                            if (btn_flags & (RI_MOUSE_RIGHT_BUTTON_UP as u16)) != 0 {
                                let _ = ctx.sender.try_send(RawInputEvent {
                                    device_id,
                                    t_us,
                                    payload: RawInputPayload::RightButtonUp,
                                });
                            }
                        }
                    });
                }
            }
        }
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    if msg == WM_CLOSE {
        DestroyWindow(hwnd);
        return 0;
    }

    if msg == WM_DESTROY {
        PostQuitMessage(0);
        return 0;
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}
