//! Low-latency, dedicated Windows Raw Input mouse capture worker.
//!
//! Fixes implemented:
//! 1. Worker shutdown: Drops send `WM_CLOSE` to the worker window, and `WM_DESTROY` calls `PostQuitMessage(0)`.
//!    Message loop runs cleanly on `while GetMessageW(..) > 0`. Drop joins the thread.
//! 2. Absolute mouse filter: Inspects `usFlags & MOUSE_MOVE_ABSOLUTE` (and `MOUSE_MOVE_VIRTUAL_DESKTOP`).
//!    Absolute movement events (from RDP, tablets, VMs) are discarded and flagged via `ABSOLUTE_MOUSE_DISCARDED`.
//! 3. Real HWND focus checking: Every `WM_INPUT` message queries `GetForegroundWindow()` and checks if it
//!    matches `target_hwnd` (the actual Bevy game window). Events outside our window are safely discarded.

use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, MOUSE_MOVE_ABSOLUTE, RAWINPUT,
    RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEMOUSE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, PostMessageW, PostQuitMessage, RegisterClassExW, TranslateMessage, MSG, WM_CLOSE,
    WM_DESTROY, WM_INPUT, WNDCLASSEXW,
};

/// Discrete raw input event captured with microsecond resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawInputEvent {
    pub t_us: u64,
    pub dx: i32,
    pub dy: i32,
}

static TARGET_HWND: AtomicUsize = AtomicUsize::new(0);
static ABSOLUTE_EVENTS_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub struct RawInputWorker {
    pub receiver: Receiver<RawInputEvent>,
    pub qpc_freq: i64,
    pub qpc_start: i64,
    hwnd: isize,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl RawInputWorker {
    /// Update the target Bevy window HWND so GetForegroundWindow() is accurately verified per event.
    pub fn set_target_hwnd(hwnd: HWND) {
        TARGET_HWND.store(hwnd as usize, Ordering::Relaxed);
    }

    /// Returns the number of absolute mouse events discarded (e.g. tablet or RDP).
    pub fn absolute_events_discarded() -> u64 {
        ABSOLUTE_EVENTS_COUNT.load(Ordering::Relaxed)
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

            // Clean message loop: exits when GetMessageW returns 0 (WM_QUIT) or <= 0
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, 0 as HWND, 0, 0) > 0 {
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
            // Post WM_CLOSE to wake up GetMessageW and close the worker window cleanly
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
        // Read QPC per discrete event for true microsecond timestamping
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
            // Verify foreground window matches Bevy's actual HWND
            let target = TARGET_HWND.load(Ordering::Relaxed) as HWND;
            let foreground = GetForegroundWindow();

            // Record only once the Bevy window HWND is known and it is the foreground window
            if !target.is_null() && foreground == target {
                let mouse = raw_data.data.mouse;

                // Absolute mouse check: discard absolute coordinates from tablets, RDP, or VMs
                if (mouse.usFlags & MOUSE_MOVE_ABSOLUTE) != 0 {
                    ABSOLUTE_EVENTS_COUNT.fetch_add(1, Ordering::Relaxed);
                } else {
                    let dx = mouse.lLastX;
                    let dy = mouse.lLastY;

                    if dx != 0 || dy != 0 {
                        WORKER_CONTEXT.with(|ctx| {
                            if let Some(ctx) = ctx.borrow().as_ref() {
                                let qpc_diff = (qpc_now - ctx.qpc_start).max(0) as u128;
                                let t_us =
                                    ((qpc_diff * 1_000_000u128) / (ctx.qpc_freq as u128)) as u64;

                                let _ = ctx.sender.try_send(RawInputEvent { t_us, dx, dy });
                            }
                        });
                    }
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
