mod ime;
mod monitor;
mod ui;

use std::sync::{mpsc, Arc};
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

    let overlay = ui::OverlayWindow::new()?;
    println!("[Main] Overlay window created.");
    // overlay.show()?;
    // println!("[Main] Overlay window shown.");
    let overlay_arc = Arc::new(overlay);

    // Start monitoring in a background thread
    let tx_monitor = tx.clone();
    thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        if let Err(e) = monitor::run_monitor(tx_monitor) {
            eprintln!("Monitor error: {:?}", e);
        }
    });

    // Thread to handle IME status updates and move the overlay
    let overlay_ui = overlay_arc.clone();
    thread::spawn(move || {
        println!("[Main] UI update thread started.");
        // Just move rx here, no need for Arc
        while let Ok(status) = rx.recv() {
            let _ = overlay_ui.update_status(status);
            // let _ = overlay_ui.update_position();
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
