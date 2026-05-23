#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod db;

use app::TodoApp;

fn app_title(_: &TodoApp) -> String {
    String::from("Todo App — iced + rusqlite")
}

// ═══════════════════════════════════════════════════════════════════════
//  Native desktop entry point
// ═══════════════════════════════════════════════════════════════════════

#[cfg(not(target_arch = "wasm32"))]
fn main() -> iced::Result {
    iced::application(TodoApp::new, TodoApp::update, TodoApp::view)
    .title(app_title)
        .theme(TodoApp::theme)
        .window_size((480.0, 640.0))
        .run()
}

// ═══════════════════════════════════════════════════════════════════════
//  WebAssembly entry point
// ═══════════════════════════════════════════════════════════════════════

#[cfg(target_arch = "wasm32")]
fn main() {
    // Install the OPFS SAH-pool VFS before iced boots, so that
    // `Connection::open("todos.db")` inside `TodoApp::new` goes through OPFS.
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = init_opfs().await {
            web_sys::console::warn_1(
                &format!("OPFS unavailable ({e}), DB will be in-memory only").into(),
            );
        } else {
            web_sys::console::log_1(&"OPFS VFS installed — data will persist.".into());
        }

        // Now start iced.  On wasm `run()` sets up the event loop via
        // requestAnimationFrame and returns immediately.
        iced::application(TodoApp::new, TodoApp::update, TodoApp::view)
            .title(app_title)
            .theme(TodoApp::theme)
            .run()
            .expect("iced failed to start");
    });
}

#[cfg(target_arch = "wasm32")]
async fn init_opfs() -> Result<(), Box<dyn std::error::Error>> {
    use sqlite_wasm_rs::sahpool_vfs::{install as install_opfs_sahpool, OpfsSAHPoolCfg};
    install_opfs_sahpool(&OpfsSAHPoolCfg::default(), true).await?;
    Ok(())
}
