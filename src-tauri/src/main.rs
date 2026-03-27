// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use calamine::{open_workbook, DataType, Reader, Xlsx};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::sleep;

#[derive(Clone, Serialize)]
struct ProgressStatus {
    current: usize,
    total: usize,
    message: String,
}

// Emits progress to the TypeScript frontend
fn emit_progress(app: &AppHandle, current: usize, total: usize, message: &str) {
    let _ = app.emit("import-progress", ProgressStatus {
        current,
        total,
        message: message.to_string(),
    });
}

#[tauri::command]
async fn start_import(app: AppHandle, file_path: String) -> Result<(), String> {
    emit_progress(&app, 0, 0, "Initializing...");

    // 1. Ensure the Dutchie Automation Window exists
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

    // Bring it to focus so the user can ensure they are logged in
    dutchie_window.set_focus().unwrap();

    emit_progress(&app, 0, 0, "Reading Excel File...");

    // 2. Read Excel File using Calamine (No Excel COM required)
    let mut excel: Xlsx<_> = open_workbook(&file_path).map_err(|e| format!("Excel Error: {}", e))?;
    let sheet_names = excel.sheet_names().to_owned();
    let sheet = sheet_names.first().ok_or("No sheets found in workbook")?;
    
    let range = excel.worksheet_range(sheet).ok_or("Empty sheet")?.map_err(|e| e.to_string())?;
    
    let total_rows = range.get_size().0.saturating_sub(1); // Subtract header row
    if total_rows == 0 {
        return Err("No data found to process.".to_string());
    }

    // 3. Process Rows (Skipping Header)
    for (i, row) in range.rows().skip(1).enumerate() {
        let current_row = i + 1;
        
        // AHK Column Mapping (0-indexed in Rust): metrc(0), qty(3), ndc(4), lot(5), expDate(6), packDate(7)
        let metrc = row.get(0).unwrap_or(&DataType::Empty).to_string();
        if metrc.is_empty() { break; } // Stop at first empty row

        let qty = row.get(3).unwrap_or(&DataType::Empty).to_string();
        let ndc = row.get(4).unwrap_or(&DataType::Empty).to_string();
        let lot = row.get(5).unwrap_or(&DataType::Empty).to_string();
        let exp_date = row.get(6).unwrap_or(&DataType::Empty).to_string();
        let pack_date = row.get(7).unwrap_or(&DataType::Empty).to_string();

        emit_progress(&app, current_row, total_rows, &format!("Processing Row {} of {}...", current_row, total_rows));

        // 4. Build and Execute JS Payload
        // Note: We escape `{` and `}` as `{{` and `}}` in Rust's format! macro.
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
                    el.dispatchEvent(new Event("blur", {{ bubbles: true }})); // Added blur to trigger React state
                    await delay(150);
                }};

                // Open modal if closed
                if (!document.querySelector("div[data-testid=receive-inventory-details_sr_product]")) {{
                    document.querySelector("button[data-testid=receive-inventory_button_add]")?.click();
                    await delay(300);
                }}

                // Search Product
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

                // Set Standard Inputs
                await setReactValue("input[data-testid=receive-inventory-details_sr_quantity]", "{qty}");
                await setReactValue("input-input_Package ID", "{ndc}");
                await setReactValue("input[data-testid=receive-inventory-details_sr_external-package-id]", "{metrc}");
                await setReactValue("input-input_Lot name/batch ID", "{lot}");

                // Date Inputs (Replacing CDP Coord Clicks with React setters)
                await setReactValue("input-input_Expiration date", "{exp_date}");
                await setReactValue("input-input_Packaging date", "{pack_date}");

                await delay(300);

                // Save
                const saveBtn = document.querySelector("button[data-testid=receive-inventory-details_button_save]");
                if (saveBtn && !saveBtn.disabled) {{
                    saveBtn.click();
                }}
            }})();
        "#);

        // Evaluate JS in the Dutchie window
        dutchie_window.eval(&js_payload).map_err(|e| e.to_string())?;
        
        // Wait for the JS to finish and the UI to settle (~2.5 seconds total per row)
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
