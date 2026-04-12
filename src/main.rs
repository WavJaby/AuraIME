mod ime;
mod monitor;
mod ui;

use std::sync::mpsc;
use std::thread;
use windows::core::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::*;

fn main() -> Result<()> {
    simple_logger::init().unwrap();
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let (tx, rx) = mpsc::channel();

    // Initialize COM for the main thread
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let overlay_arc = ui::OverlayWindow::new()?;
    log::debug!("Overlay window created.");

    // Start monitoring in a background thread
    let tx_monitor = tx.clone();
    thread::spawn(move || {
        if let Err(e) = monitor::run_monitor(tx_monitor) {
            log::error!("Monitor error: {:?}", e);
        }
    });

    // Thread to handle IME status updates and move the overlay
    let overlay_ui = overlay_arc.clone();
    thread::spawn(move || {
        log::info!("UI update thread started.");
        while let Ok(event) = rx.recv() {
            match event {
                monitor::MonitorEvent::StatusChanged(status) => {
                    log::debug!("Status changed event received");
                    let _ = overlay_ui.update_status(status);
                }
            }
        }
        log::info!("UI update thread exiting.");
    });

    // Standard Win32 message loop
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
