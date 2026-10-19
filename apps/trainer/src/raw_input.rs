//! Low-latency, dedicated Windows Raw Input mouse capture worker.
//!
//! Architecture:
//! - Spawns a background OS thread running a message-only window (`HWND_MESSAGE`).
//! - Registers usage page 0x01, usage 0x02 (mouse) with `RIDEV_INPUTSINK | RIDEV_DEVNOTIFY`.
//!   Registered AFTER the Bevy event loop exists so our worker takes over raw mouse messages.
//!   Bevy's `MouseMotion` will be zero by design.
//! - Uses `GetRawInputData` to capture every discrete input event with its own
//!   `QueryPerformanceCounter` timestamp in monotonic microseconds relative to session start.
//! - QPC multiplication is performed in u128 arithmetic to guarantee zero overflow.
//! - Discards events when the main window does not have focus.

use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEMOUSE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    RegisterClassExW, TranslateMessage, MSG, WM_CLOSE, WM_INPUT, WNDCLASSEXW,
};

/// Discrete raw input event captured with microsecond resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawInputEvent {
    pub t_us: u64,
    pub dx: i32,
    pub dy: i32,
}

static WORKER_STOP: AtomicBool = AtomicBool::new(false);
static IS_FOCUSED: AtomicBool = AtomicBool::new(true);

pub struct RawInputWorker {
    pub receiver: Receiver<RawInputEvent>,
    pub qpc_freq: i64,
    pub qpc_start: i64,
    _hwnd: usize,
    _join_handle: Option<thread::JoinHandle<()>>,
}

impl RawInputWorker {
    /// Update window focus state so events outside our window are safely discarded.
    pub fn set_focused(focused: bool) {
        IS_FOCUSED.store(focused, Ordering::Relaxed);
    }

    /// Spawns the dedicated raw input thread and returns the channel receiver.
    pub fn spawn() -> Self {
        WORKER_STOP.store(false, Ordering::Relaxed);
        IS_FOCUSED.store(true, Ordering::Relaxed);

        let mut freq = 0i64;
        let mut start = 0i64;
        unsafe {
            QueryPerformanceFrequency(&mut freq);
            QueryPerformanceCounter(&mut start);
        }

        let (tx, rx) = crossbeam_channel::unbounded();
        let (hwnd_tx, hwnd_rx) = std::sync::mpsc::channel::<usize>();

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

            // Pass channel and timing state to thread-local context
            WORKER_CONTEXT.with(|ctx| {
                *ctx.borrow_mut() = Some(WorkerContext {
                    sender: tx,
                    qpc_freq: freq,
                    qpc_start: start,
                });
            });

            hwnd_tx.send(hwnd as usize).unwrap();

            let mut msg: MSG = std::mem::zeroed();
            while !WORKER_STOP.load(Ordering::Relaxed) {
                if GetMessageW(&mut msg, 0 as HWND, 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });

        let hwnd = hwnd_rx.recv().unwrap();

        Self {
            receiver: rx,
            qpc_freq: freq,
            qpc_start: start,
            _hwnd: hwnd,
            _join_handle: Some(join_handle),
        }
    }
}

impl Drop for RawInputWorker {
    fn drop(&mut self) {
        WORKER_STOP.store(true, Ordering::Relaxed);
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
        // Immediate QPC read per event to eliminate batch timing distortion
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
            // Discard events if window is not focused
            if IS_FOCUSED.load(Ordering::Relaxed) {
                let mouse = raw_data.data.mouse;
                let dx = mouse.lLastX;
                let dy = mouse.lLastY;

                if dx != 0 || dy != 0 {
                    WORKER_CONTEXT.with(|ctx| {
                        if let Some(ctx) = ctx.borrow().as_ref() {
                            let qpc_diff = (qpc_now - ctx.qpc_start).max(0) as u128;
                            let t_us = ((qpc_diff * 1_000_000u128) / (ctx.qpc_freq as u128)) as u64;

                            let _ = ctx.sender.try_send(RawInputEvent { t_us, dx, dy });
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

    DefWindowProcW(hwnd, msg, wparam, lparam)
}
