// Some platform-gated paths compile only on one desktop OS.
#![allow(dead_code, unused_imports, unused_variables)]

mod app;
#[doc(hidden)]
pub mod bench;
mod domain;
mod error;
mod infra;
mod platform;

use app::runtime::KuraRuntime;
use tauri::Manager;

pub fn run() {
    let builder = tauri::Builder::default();

    // Register first so a second launch focuses the open window instead.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        // A kura:// link opened while Kura runs arrives as an argument.
        for url in args.iter().filter(|arg| arg.starts_with("kura://")) {
            use tauri::Emitter;
            let _ = app.emit("deep-link://new-url", vec![url.clone()]);
        }
    }));

    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            #[cfg(any(windows, target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }
            app.manage(KuraRuntime::new());
            {
                use tauri::Emitter;
                let handle = app.handle().clone();
                let runtime = app.state::<KuraRuntime>();
                if let Err(error) = runtime.directory_watch.start(move |paths| {
                    let _ = handle.emit(infra::directory_watch::DIRECTORY_CHANGED_EVENT, paths);
                }) {
                    eprintln!("kura: open folders won't refresh on their own: {error}");
                }
            }
            // Reopen the extra windows that were open when Kura quit.
            for (label, folder) in app.state::<KuraRuntime>().extra_windows.saved() {
                if let Err(error) = app::commands::open_files_window(app.handle(), &label, &folder)
                {
                    eprintln!("kura: could not reopen a window: {error}");
                }
            }
            app::cloud_commands::start(app.handle());
            #[cfg(target_os = "macos")]
            infra::devices::start_device_change_listener(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // A closed window stops watching its folders and isn't reopened.
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(runtime) = window.try_state::<KuraRuntime>() {
                    let _ = runtime
                        .directory_watch
                        .set_directories(window.label(), Vec::new());
                    runtime.extra_windows.closed(window.label());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app::cloud_commands::cloud_add_remote,
            app::cloud_commands::cloud_cancel_sign_in,
            app::cloud_commands::cloud_conflicts,
            app::cloud_commands::cloud_quota,
            app::cloud_commands::cloud_remotes,
            app::cloud_commands::cloud_remove_remote,
            app::cloud_commands::cloud_resolve_conflict,
            app::cloud_commands::cloud_set_credentials,
            app::cloud_commands::cloud_sign_in,
            app::commands::app_snapshot,
            app::commands::archive_create,
            app::commands::archive_extract,
            app::commands::archive_list,
            app::commands::clipboard_native_file_refs,
            app::commands::clipboard_set_local,
            app::commands::clipboard_snapshot,
            app::commands::clipboard_write_file_bytes,
            app::commands::clipboard_write_file_refs,
            app::commands::compare_apply_text_merge,
            app::commands::compare_files,
            app::commands::compare_folders,
            app::commands::devices_snapshot,
            app::commands::devices_unmount,
            app::commands::duplicates_cancel,
            app::commands::duplicates_scan,
            app::commands::explorer_calculate_directory_sizes,
            app::commands::explorer_cancel_drag_preparation,
            app::commands::explorer_directory_size_snapshot,
            app::commands::explorer_generate_image_thumbnail,
            app::commands::explorer_library_record_last_opened,
            app::commands::explorer_library_record_recent,
            app::commands::explorer_library_snapshot,
            app::commands::explorer_list_directory,
            app::commands::explorer_watch_directories,
            app::commands::app_open_window,
            app::commands::app_window_set_folder,
            app::commands::explorer_open_association,
            app::commands::explorer_open_path,
            app::commands::explorer_open_with,
            app::commands::explorer_path_exists,
            app::commands::explorer_path_is_directory,
            app::commands::explorer_prepare_drag_items,
            app::commands::explorer_prepare_open_item,
            app::commands::explorer_preview_item,
            app::commands::explorer_queue_create_item,
            app::commands::explorer_queue_delete_items,
            app::commands::explorer_put_back_items,
            app::commands::explorer_trash_fully_visible,
            app::commands::explorer_volume_space,
            app::commands::explorer_set_tag,
            app::commands::explorer_read_tags,
            app::commands::explorer_share,
            app::commands::explorer_queue_paste_blob,
            app::commands::explorer_queue_paste_items,
            app::commands::explorer_queue_paste_text,
            app::commands::explorer_queue_rename_item,
            app::commands::explorer_queue_rename_items,
            app::commands::explorer_save_preview_item,
            app::commands::file_metadata_snapshot,
            app::commands::file_sync_apply,
            app::commands::file_sync_compare,
            app::commands::file_sync_pair_remove,
            app::commands::file_sync_pair_save,
            app::commands::file_sync_pairs_snapshot,
            app::commands::file_tools_checksum,
            app::commands::file_tools_create_symlink,
            app::commands::file_tools_read_symlink,
            app::commands::navigation_names_snapshot,
            app::commands::navigation_names_update,
            app::commands::open_terminal_at_path,
            app::commands::operation_queue_cancel,
            app::commands::operation_queue_pause,
            app::commands::operation_queue_pause_all,
            app::commands::operation_queue_redo,
            app::commands::operation_queue_resolve_conflict,
            app::commands::operation_queue_resume,
            app::commands::operation_queue_resume_all,
            app::commands::operation_queue_retry_transfer,
            app::commands::operation_queue_snapshot,
            app::commands::operation_queue_undo,
            app::commands::operation_queue_session_started,
            app::commands::saved_searches_delete,
            app::commands::saved_searches_save,
            app::commands::saved_searches_snapshot,
            app::commands::search_cancel_scan,
            app::commands::search_get_status,
            app::commands::search_init,
            app::commands::search_query,
            app::commands::search_start_scan,
            app::commands::search_stop_watching,
            app::commands::settings_open_with_associations,
            app::commands::settings_save,
            app::commands::settings_snapshot,
            app::commands::transfers_snapshot,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Kura")
        .run(|app, event| {
            // Windows closed by quitting are reopened next launch.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(runtime) = app.try_state::<KuraRuntime>() {
                    runtime.extra_windows.exiting();
                }
            }
        });
}
