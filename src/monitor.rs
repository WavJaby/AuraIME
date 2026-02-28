use crate::ime::ImeStatus;
use std::sync::mpsc;
use windows::core::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn run_monitor(tx: mpsc::Sender<ImeStatus>) -> Result<()> {
    println!("[Monitor] Starting monitor thread...");
    unsafe {
        // Polling loop: check foreground window's keyboard layout periodically
        // TSF sinks are thread-local and won't fire for other threads' language changes,
        // so we poll GetKeyboardLayout on the foreground window's thread.
        println!("[Monitor] Entering polling loop.");
        let mut msg = MSG::default();
        loop {
            // Process any pending COM/window messages
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return Ok(());
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Check foreground window's keyboard layout
            let fg_hwnd = GetForegroundWindow();
            if let Some(status) = crate::ime::get_status_from_hwnd(fg_hwnd) {
                let _ = tx.send(status);
            }

            windows::Win32::System::Threading::Sleep(50);
        }
    }
}
