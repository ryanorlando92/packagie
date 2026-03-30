#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use calamine::{open_workbook, Data, Reader, Xlsx};
use chrono::{Duration as ChronoDuration, NaiveDate};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::sleep;
use keyring::Entry;
use secrecy::{ExposeSecret, SecretString};

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
fn save_credentials(password: String) -> Result<(), String> {
    println!("\n[KEYRING DEBUG] Saving password to Packagie vault...");
    let secret_pass = SecretString::from(password);
    
    // Use a hardcoded, safe string to bypass OS parsing bugs!
    let entry = Entry::new("PackagieVault", "packagie_internal_user").map_err(|e| e.to_string())?;
    
    match entry.set_password(secret_pass.expose_secret()) {
        Ok(_) => {
            println!("[KEYRING DEBUG] SUCCESS: Password locked in vault.");
            Ok(())
        },
        Err(e) => {
            println!("[KEYRING DEBUG] FAILED: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
fn has_saved_password() -> bool {
    // Check the static vault
    if let Ok(entry) = Entry::new("PackagieVault", "packagie_internal_user") {
        entry.get_password().is_ok()
    } else {
        false
    }
}

#[tauri::command]
async fn auto_login(app: tauri::AppHandle, username: String) -> Result<(), String> {
    let dutchie_window = app.get_webview_window("dutchie").ok_or("Dutchie window not found.")?;

    // Pull from the static vault
    let entry = Entry::new("PackagieVault", "packagie_internal_user").map_err(|e| e.to_string())?;
    let stored_pass = entry.get_password().map_err(|_| "No password found".to_string())?;
    
    let secret_pass = SecretString::from(stored_pass);

    // We still use the 'username' parameter here so we can inject it into the web page!
    let safe_user = serde_json::to_string(&username).unwrap_or_default();
    let safe_pass = serde_json::to_string(secret_pass.expose_secret()).unwrap_or_default();

    let js_payload = format!(r#"
        (function() {{
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
                const userField = document.querySelector('input[data-testid="login-page_input_username"], input[placeholder="Username"]');
                const passField = document.querySelector('input[type="password"]');

                if (userField && passField) {{
                    setNativeValue(userField, injectedUser);
                    setNativeValue(passField, injectedPass);
                    
                    const loginBtn = document.querySelector('button[type="submit"]');
                    if (loginBtn) {{
                        setTimeout(() => loginBtn.click(), 500); 
                    }}
                }} else if (attempts < 20) {{
                    setTimeout(attemptLogin, 500);
                }}
            }};
            attemptLogin();
        }})();
    "#, safe_user, safe_pass);

    dutchie_window.eval(&js_payload).map_err(|e| e.to_string())?;
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
                
                // NEW: Added isDate parameter for aggressive popper closing
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

                    el.focus(); el.click(); await delay(50);
                    const ns = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                    ns.call(el, val);
                    el.dispatchEvent(new Event("input", {{ bubbles: true }}));
                    el.dispatchEvent(new Event("change", {{ bubbles: true }}));
                    el.dispatchEvent(new KeyboardEvent("keydown", {{ key: "Enter", keyCode: 13, bubbles: true }}));
                    
                    // Steal focus away from the date picker so it saves
                    if (isDate) {{
                        await delay(100);
                        el.dispatchEvent(new KeyboardEvent("keydown", {{ key: "Escape", keyCode: 27, bubbles: true }}));
                        
                        const header = document.querySelector("h2.MuiDialogTitle-root") || document.querySelector("h2");
                        if (header) {{
                            header.dispatchEvent(new MouseEvent("mousedown", {{ bubbles: true }}));
                            header.dispatchEvent(new MouseEvent("mouseup", {{ bubbles: true }}));
                            header.click();
                        }}
                    }}

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
                        
                        // Wait for Dutchie API
                        await delay(200); 
                        
                        // Find the dropdown option
                        const opt = document.querySelector("li[data-option-index='0']");
                        if (opt) {{
                            // Force MUI's required mousedown event
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

                // Save
                const saveBtn = document.querySelector("button[data-testid=receive-inventory-details_button_save]");
                if (saveBtn && !saveBtn.disabled) {{
                    saveBtn.click();
                }}
            }})();
        "#
        );

        dutchie_window
            .eval(&js_payload)
            .map_err(|e| e.to_string())?;
        sleep(Duration::from_millis(5500)).await;

        if current_row > 10 {
            sleep(Duration::from_millis(500)).await;
        }
        if current_row > 20 {
            sleep(Duration::from_millis(500)).await;
        }
        if current_row > 27 {
            sleep(Duration::from_millis(500)).await;
        }
        if current_row > 34 {
            sleep(Duration::from_millis(500)).await;
        }
        if current_row > 40 {
            sleep(Duration::from_millis(1000)).await;
        }
        if current_row > 45 {
            sleep(Duration::from_millis(1000)).await;
        }
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
        .invoke_handler(tauri::generate_handler![start_import, save_credentials, has_saved_password, auto_login])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
