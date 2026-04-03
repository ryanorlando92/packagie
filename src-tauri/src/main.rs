#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use calamine::{open_workbook, Data, Reader, Xlsx};
use chrono::{Duration as ChronoDuration, NaiveDate};
use serde::Serialize;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;
use tauri::Listener;
use tokio::sync::oneshot;
use tokio::time::{timeout, sleep};
use tokio::task::spawn;


#[derive(Clone, Serialize)]
struct ProgressStatus {
    current: usize,
    total: usize,
    message: String,
}

fn emit_progress(app: &AppHandle, current: usize, total: usize, message: &str) {
    let _ = app.emit(
        "import-progress",
        ProgressStatus {
            current,
            total,
            message: message.to_string(),
        },
    );
}

fn format_excel_date(data: &Data) -> String {
    let raw = data.to_string();
    if raw.trim().is_empty() {
        return String::new();
    }
    if raw.contains('/') || raw.contains('-') {
        return raw;
    }
    if let Ok(serial) = raw.parse::<f64>() {
        if let Some(epoch) = NaiveDate::from_ymd_opt(1899, 12, 30) {
            let date = epoch + ChronoDuration::days(serial as i64);
            return date.format("%m/%d/%Y").to_string();
        }
    }
    raw
}

#[tauri::command]
fn get_hardware_key() -> Result<String, String> {
    machine_uid::get().map_err(|_| "Failed to fetch Hardware UUID".to_string())
}

#[tauri::command]
async fn auto_login(app: tauri::AppHandle, username: String, pass: String) -> Result<(), String> {
    let dutchie_window = app.get_webview_window("dutchie").ok_or("Dutchie window not found.")?;

    let safe_user = serde_json::to_string(&username).unwrap_or_default();
    let safe_pass = serde_json::to_string(&pass).unwrap_or_default();

    let js_payload = format!(r#"
        (function() {{
            if (window.__packagie_injected) return;
            window.__packagie_injected = true;

            const injectedUser = {};
            const injectedPass = {};
            let attempts = 0;

            const setNativeValue = (element, value) => {{
                const valueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                valueSetter.call(element, value);
                element.dispatchEvent(new Event('input', {{ bubbles: true }}));
            }};

            const attemptLogin = () => {{
                attempts++;
                
                const userField = document.querySelector('input[data-testid="login-page_input_username"], input[placeholder="Username"], input[name="username"]');
                const passField = document.querySelector('input[type="password"]');

                if (userField && passField) {{
                    setNativeValue(userField, injectedUser);
                    setNativeValue(passField, injectedPass);
                    
                    const loginBtn = document.querySelector('button[type="submit"]');
                    if (loginBtn) {{
                        setTimeout(() => loginBtn.click(), 500); 
                    }}
                }} else if (attempts < 3) {{
                    setTimeout(attemptLogin, 400);
                }}
            }};
            attemptLogin();
        }})();
    "#, safe_user, safe_pass);

    spawn(async move {
        for _ in 0..3 {
            sleep(Duration::from_millis(1800)).await;
            let _ = dutchie_window.eval(&js_payload);
        }
    });

    Ok(())
}

#[tauri::command]
async fn start_import(app: AppHandle, file_path: String) -> Result<(), String> {
    let dutchie_window = app
        .get_webview_window("dutchie")
        .ok_or("Dutchie window not found. Please restart the app.")?;

    dutchie_window.set_focus().map_err(|e| e.to_string())?;
    emit_progress(&app, 0, 0, "Reading Excel File...");

    let mut excel: Xlsx<_> =
        open_workbook(&file_path).map_err(|e| format!("Excel Error: {}", e))?;
    let sheet = excel
        .sheet_names()
        .first()
        .cloned()
        .ok_or("No sheets found")?;
    let range = excel.worksheet_range(&sheet).map_err(|e| e.to_string())?;
    let total_rows = range.get_size().0.saturating_sub(1);

    for (i, row) in range.rows().skip(1).enumerate() {

        if !dutchie_window.is_focused().unwrap_or(true) {
            
            let (tx, rx) = oneshot::channel();

            app.dialog()
                .message("Automation paused because the window lost focus.\n\nPlease ensure the Receive Inventory window is active, then click OK to resume.")
                .title("Packagie Paused")
                .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
                .show(move |_| {
                    // This closure fires ONLY when the user clicks 'OK'
                    let _ = tx.send(()); 
                });

            let _ = rx.await;

            sleep(Duration::from_millis(500)).await;
        }

        let current_row = i + 1;

        let metrc = row.get(0).map(|d| d.to_string()).unwrap_or_default();
        if metrc.is_empty() {
            break;
        }

        let qty = row.get(3).map(|d| d.to_string()).unwrap_or_default();
        let ndc = row.get(4).map(|d| d.to_string()).unwrap_or_default();
        let lot = row.get(5).map(|d| d.to_string()).unwrap_or_default();
        let exp_date = format_excel_date(row.get(6).unwrap_or(&Data::Empty));
        let pack_date = format_excel_date(row.get(7).unwrap_or(&Data::Empty));

        emit_progress(
            &app,
            current_row,
            total_rows,
            &format!("Processing Row {}...", current_row),
        );

        let js_payload = format!(
            r#"
            (async function() {{
                const delay = ms => new Promise(r => setTimeout(r, ms));
                
                const injectField = async (identifier, val, isDate = false) => {{
                    if (!val || val === "") return;
                    let el = document.getElementById(identifier);
                    if (!el) {{ try {{ el = document.querySelector(identifier); }} catch(e) {{}} }}
                    if (!el) return;

                    const wrapper = el.closest('.MuiInputBase-root');
                    if (wrapper) {{
                        const clearBtn = wrapper.querySelector('button[data-testid="clear-date-input"]');
                        if (clearBtn) {{ clearBtn.click(); await delay(100); }}
                    }}

                    el.focus(); 
                    await delay(50);

                    const ns = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                    ns.call(el, val);
                    el.dispatchEvent(new Event("input", {{ bubbles: true }}));
                    el.dispatchEvent(new Event("change", {{ bubbles: true }}));
                    el.dispatchEvent(new KeyboardEvent("keydown", {{ key: "Enter", keyCode: 13, bubbles: true }}));

                    el.blur();
                    await delay(150);
                }};

                if (!document.querySelector("div[data-testid=receive-inventory-details_sr_product]")) {{
                    document.querySelector("button[data-testid=receive-inventory_button_add]")?.click();
                    await delay(200); 
                }}

                const prod = document.querySelector("input[data-testid=receive-inventory-details_sr_product]");
                if (prod) {{
                    prod.focus(); prod.click();
                    await delay(200);
                    
                    const search = document.querySelector("input[data-testid=receive-package-modal-products-dropdown-search-input]");
                    if (search) {{
                        search.focus();
                        const ns = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                        ns.call(search, "{ndc}");
                        search.dispatchEvent(new Event("input", {{bubbles: true}}));
                        
                        await delay(200); 
                        
                        const opt = document.querySelector("li[data-option-index='0']");
                        if (opt) {{
                            opt.dispatchEvent(new MouseEvent('mousedown', {{ bubbles: true, cancelable: true }}));
                            await delay(50);
                            opt.dispatchEvent(new MouseEvent('mouseup', {{ bubbles: true, cancelable: true }}));
                            opt.click();
                        }}
                    }}
                }}
                await delay(200);

                await injectField("input[data-testid=receive-inventory-details_sr_quantity]", "{qty}");
                await injectField("input-input_Package ID", "{ndc}");
                await injectField("input-input_External package ID", "{metrc}");
                await injectField("input-input_Lot name/batch ID", "{lot}");
                await injectField("input-input_Expiration date", "{exp_date}", true);
                await delay(50);
                await injectField("input-input_Packaging date", "{pack_date}", true);

                await delay(200);

                const saveBtn = document.querySelector("button[data-testid=receive-inventory-details_button_save]");
                if (saveBtn && !saveBtn.disabled) {{
                    saveBtn.click();
                    await delay(100);
                    window.__TAURI__.event.emit("row_complete");
                }}
            }})();
        "#
        );

        let (tx, rx) = oneshot::channel();
        let tx_mutex = Arc::new(Mutex::new(Some(tx)));

        let event_id = app.listen_any("row_complete", move |_| {
            if let Some(sender) = tx_mutex.lock().unwrap().take() {
                let _ = sender.send(());
            }
        });

        dutchie_window.eval(&js_payload).map_err(|e| e.to_string())?;

        let _ = timeout(Duration::from_secs(10), rx).await;

        app.unlisten(event_id);

        sleep(Duration::from_millis(100)).await;
    }

    emit_progress(&app, total_rows, total_rows, "Import Complete!");
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if _window.label() == "main" {
                    let app = _window.app_handle();
                    if let Some(dutchie) = app.get_webview_window("dutchie") {
                        let _ = dutchie.destroy();
                    }
                }
            }
        })
        .setup(|app| {
            WebviewWindowBuilder::new(
                app,
                "dutchie",
                WebviewUrl::External(
                    "https://verano.backoffice.dutchie.com/products/inventory/receive-inventory"
                        .parse()
                        .unwrap(),
                ),
            )
            .title("Packagie - Receive Inventory")
            .inner_size(1100.0, 850.0)
            .build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![start_import, get_hardware_key, auto_login])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
