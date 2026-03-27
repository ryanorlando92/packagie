// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use calamine::{Reader, Xlsx, open_workbook, Data}; // 1. Changed DataType to Data to avoid trait confusion
use serde::Serialize;
use std::time::Duration; // 2. Added explicit Duration import
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::sleep;

#[derive(Clone, Serialize)]
struct ProgressStatus {
    current: usize,
    total: usize,
    message: String,
}

fn emit_progress(app: &AppHandle, current: usize, total: usize, message: &str) {
    let _ = app.emit("import-progress", ProgressStatus {
        current,
        total,
        message: message.to_string(),
    });
}

// 3. Removed 'pub' to resolve the __cmd__ duplication error in a single-file main.rs
#[tauri::command]
async fn start_import(app: AppHandle, file_path: String) -> Result<(), String> {
    emit_progress(&app, 0, 0, "Initializing...");

    let dutchie_window = app.get_webview_window("dutchie").unwrap_or_else(|| {
        WebviewWindowBuilder::new(
            &app,
            "dutchie",
            WebviewUrl::External("https://verano.backoffice.dutchie.com/products/inventory/receive-inventory".parse().unwrap()),
        )
        .title("Dutchie Automation View")
        .inner_size(1024.0, 768.0)
        .build()
        .unwrap()
    });

    dutchie_window.set_focus().unwrap();

    emit_progress(&app, 0, 0, "Reading Excel File...");

    let mut excel: Xlsx<_> = open_workbook(&file_path).map_err(|e| format!("Excel Error: {}", e))?;
    let sheet_names = excel.sheet_names().to_owned();
    let sheet = sheet_names.first().ok_or("No sheets found in workbook")?;
    
    let range = excel
                .worksheet_range(sheet)
                .map_err(|e| format!("Could not read sheet '{}': {}", sheet, e))?;
    
    let total_rows = range.get_size().0.saturating_sub(1);
    if total_rows == 0 {
        return Err("No data found to process.".to_string());
    }

    for (i, row) in range.rows().skip(1).enumerate() {
        let current_row = i + 1;
        
        // 4. Used .map().unwrap_or_default() to safely extract strings and avoid trait errors
        let metrc = row.get(0).map(|d| d.to_string()).unwrap_or_default();
        if metrc.is_empty() { break; } 

        let qty = row.get(3).map(|d| d.to_string()).unwrap_or_default();
        let ndc = row.get(4).map(|d| d.to_string()).unwrap_or_default();
        let lot = row.get(5).map(|d| d.to_string()).unwrap_or_default();
        let exp_date = row.get(6).map(|d| d.to_string()).unwrap_or_default();
        let pack_date = row.get(7).map(|d| d.to_string()).unwrap_or_default();

        emit_progress(&app, current_row, total_rows, &format!("Processing Row {} of {}...", current_row, total_rows));

        let js_payload = format!(r#"
            (async function() {{
                const delay = ms => new Promise(r => setTimeout(r, ms));
                const setReactValue = async (selector, value) => {{
                    const el = document.querySelector(selector) || document.getElementById(selector);
                    if (!el) return;
                    el.focus();
                    const nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                    nativeInputValueSetter.call(el, value);
                    el.dispatchEvent(new Event("input", {{ bubbles: true }}));
                    el.dispatchEvent(new Event("change", {{ bubbles: true }}));
                    el.dispatchEvent(new Event("blur", {{ bubbles: true }}));
                    await delay(150);
                }};

                if (!document.querySelector("div[data-testid=receive-inventory-details_sr_product]")) {{
                    document.querySelector("button[data-testid=receive-inventory_button_add]")?.click();
                    await delay(300);
                }}

                const prod = document.querySelector("input[data-testid=receive-inventory-details_sr_product]");
                if (prod) {{
                    prod.focus(); prod.click();
                    await delay(150);
                    const search = document.querySelector("input[data-testid=receive-package-modal-products-dropdown-search-input]");
                    if (search) {{
                        const ns = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                        ns.call(search, "{ndc}");
                        search.dispatchEvent(new Event("input", {{bubbles: true}}));
                        await delay(300);
                        const opt = document.querySelector("li[data-option-index='0']");
                        if (opt) opt.click();
                    }}
                }}
                await delay(300);

                await setReactValue("input[data-testid=receive-inventory-details_sr_quantity]", "{qty}");
                await setReactValue("input-input_Package ID", "{ndc}");
                await setReactValue("input[data-testid=receive-inventory-details_sr_external-package-id]", "{metrc}");
                await setReactValue("input-input_Lot name/batch ID", "{lot}");
                await setReactValue("input-input_Expiration date", "{exp_date}");
                await setReactValue("input-input_Packaging date", "{pack_date}");

                await delay(300);

                const saveBtn = document.querySelector("button[data-testid=receive-inventory-details_button_save]");
                if (saveBtn && !saveBtn.disabled) {{
                    saveBtn.click();
                }}
            }})();
        "#);

        dutchie_window.eval(&js_payload).map_err(|e| e.to_string())?;
        
        sleep(Duration::from_millis(2500)).await; 
    }

    emit_progress(&app, total_rows, total_rows, "Import Complete!");
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![start_import])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}