use std::sync::mpsc::Sender;
use windows::core::Interface;
use windows::Win32::System::Com::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_core::link;

pub mod caret;

pub enum MonitorEvent {
    StatusChanged(crate::ime::ImeStatus),
}

link!("msctf.dll" "system" fn TF_CreateThreadMgr(pptm: *mut *mut std::ffi::c_void) -> windows::core::HRESULT);
link!("msctf.dll" "system" fn TF_CreateInputProcessorProfiles(ppipp: *mut *mut std::ffi::c_void) -> windows::core::HRESULT);

pub fn run_monitor(tx: Sender<MonitorEvent>) -> windows::core::Result<()> {
    unsafe {
        println!("[Monitor] Entering Message Loop.");
        let mut msg = MSG::default();

        loop {
            // Process any pending COM/System messages
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return Ok(());
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Safety fallback: Check status even if no TSF event triggered
            // get_status_from_hwnd has internal state check so it's cheap to call
            let hwnd = GetForegroundWindow();
            if !hwnd.is_invalid() {
                if let Some(status) = crate::ime::get_status_from_hwnd(hwnd) {
                    match tx.send(MonitorEvent::StatusChanged(status)) {
                        Ok(_) => println!("[Monitor] StatusChanged event sent"),
                        Err(e) => eprintln!("[Monitor] Failed to send StatusChanged event: {:?}", e),
                    }
                }
            }

            // Sleep to prevent high CPU usage (approx 20 FPS)
            windows::Win32::System::Threading::Sleep(50);
        }
    }
}
