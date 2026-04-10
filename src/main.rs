mod ime;
mod monitor;
mod ui;

use std::sync::mpsc;
use std::thread;
use windows::core::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn main() -> Result<()> {
    let (tx, rx) = mpsc::channel();

    // Initialize COM for the main thread
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let overlay_arc = ui::OverlayWindow::new()?;
    println!("[Main] Overlay window created.");

    // Start monitoring in a background thread
    let tx_monitor = tx.clone();
    thread::spawn(move || {
        if let Err(e) = monitor::run_monitor(tx_monitor) {
            eprintln!("Monitor error: {:?}", e);
        }
    });

    // Thread to handle IME status updates and move the overlay
    let overlay_ui = overlay_arc.clone();
    thread::spawn(move || {
        println!("[Main] UI update thread started.");
        while let Ok(event) = rx.recv() {
            match event {
                monitor::MonitorEvent::LanguageChanged => {
                    // TSF is unused
                }
                monitor::MonitorEvent::StatusChanged(status) => {
                    println!("[Main] Status changed event received");
                    let _ = overlay_ui.update_status(status);
                }
            }
        }
        println!("[Main] UI update thread exiting.");
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
