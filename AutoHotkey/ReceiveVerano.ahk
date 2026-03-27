; CHANGELOG
;
; Version 1.2.1 - Gui QoL additions (view path from end)
; Version 1.2.0 - Add progress bar, total rows and time estimates to GUI
; Version 1.1.1 - fix crash on run when excel takes too long to respond
; Version 1.1.0 - Readme, gui, & statusText updates - performance improvements
; Version 1.0.0 - Initial Release
;
#Requires AutoHotkey v2.0
#Include Chrome.ahk

ProcessSetPriority "High"

global xlApp := ""
global backofficePage := ""
global browser := ""
global profileDir := A_AppData "\Packagie\DutchieProfile"

if !DirExist(profileDir) {
    DirCreate(profileDir)
}

KillInvisibleExcel()
OnExit(CleanupOnExit)

MonitorGui := Gui("+Resize +MinSize -MaximizeBox", "Packagie 1.2.1 - Dutchie Package Importer")
TitleText := MonitorGui.Add("Text", "x50 y15 w350 h15", "Select Excel File (.xlsx):")
TitleText.SetFont("s10", "Verdana")

global FilePathEdit := MonitorGui.Add("Edit", "x15 y+10 w280 ReadOnly", "")
FileSelectBtn := MonitorGui.Add("Button", "x+10 yp-1 w80 h24", "Browse")
FileSelectBtn.SetFont("bold", "Verdana")

StartBtn := MonitorGui.Add("Button", "x40 y+20 w150 h35", "START IMPORT")
StartBtn.SetFont("bold", "Verdana")
ReadMeBtn := MonitorGui.Add("Button", "x+15 yp-1 w150 h35", "README")
ReadMeBtn.SetFont(,"Verdana")
global StatusText := MonitorGui.Add("Text", "x15 y+15 w350 Center", "Initializing...")

global ProgressBar := MonitorGui.Add("Progress", "x15 y+5 w350 h15 -Smooth", 0)
global TimeText := MonitorGui.Add("Text", "x15 y+5 w350 Center", "Elapsed: 00:00 | ETA: 00:00")

FileSelectBtn.OnEvent("Click", SelectFile)
StartBtn.OnEvent("Click", StartImport)
ReadMeBtn.OnEvent("Click", ShowReadme)
MonitorGui.OnEvent("Close", (*) => ExitApp())


MonitorGui.Show("w400 h200")
Sleep(1000)
InitBrowserState()

ShowReadme(*) {
    helpText := "
(
========= EXCEL FILE PREP =========

1. Use Amy's Metrc script to create partially filled prep sheet

2. Cross-Reference with CPG's Sharepoint for product labels

3. Fill in Prep Sheet (Qty, NDC, Lot, ExpDate, PackDate, metrc)


===== Running the Importer =====

1. Log into backoffice from the newly opened chrome window
(First run asks you to log into a chrome account, choose no)

2. Select Prepped order excel file

3. Click 'Start Import'


============= Notes =============

* Processing Time: ~6 seconds per row

* You can do other tasks while the program runs, as long as the window remains visible
)"

MsgBox(helpText, "Packagie ReadMe", )
}

KillInvisibleExcel() {
    try {
        wmi := ComObjGet("winmgmts:")
        query := wmi.ExecQuery("Select * from Win32_Process where Name='excel.exe'")
        
        for proc in query {
            pid := proc.ProcessId
            ; By default, AHK ignores hidden windows. 
            ; If we can't find a visible main Excel window (XLMAIN) for this PID, it's an invisible orphan.
            if !WinExist("ahk_class XLMAIN ahk_pid " pid) {
                try ProcessClose(pid)
            }
        }
    }
}

CleanupOnExit(ExitReason, ExitCode) {
    global xlApp
    if (xlApp != "") {
        try {
            xlApp.DisplayAlerts := false
            xlApp.Quit()
        }
        xlApp := ""
    }
}

SelectFile(*) {
    path := FileSelect("3", "", "Select Excel File", "Excel Documents (*.xlsx; *.xls)")
    if (path) {
        FilePathEdit.Value := path
        ControlSend("{End}", FilePathEdit)
    }
}

InitBrowserState() {
    global browser, profileDir, backofficePage
    StatusText.Value := "Establishing connection to Chrome..."
    myPort := 9901

    try {
        browser := Chrome.FindInstance("chrome.exe", myPort)
        if (!browser) {
            targetURL := "https://verano.backoffice.dutchie.com/products/inventory/receive-inventory"
            browser := Chrome([targetURL], "", "", myPort, profileDir)
        }
        Sleep(2000)
        StatusText.Value := "Chrome Ready: Select File and Click 'Start Import'..."
    } catch as err {
        StatusText.Value := "Error: Could not link to Chrome."
        MsgBox("Chrome Connection Error: " err.Message, "Error", "Iconx")
        return
    }
}

StartImport(*) {
    global browser, backofficePage
    if (FilePathEdit.Value = "") {
        MsgBox("Please select an Excel file first.", "Missing File", "Icon! 0x30")
        return
    }

    try {
        pageList := browser.GetPageList()
        found := false
        for pageInfo in pageList {
            if (pageInfo.Has("url") && InStr(pageInfo["url"], "receive-inventory")) {
                backofficePage := Chrome.Page(pageInfo["webSocketDebuggerUrl"])
                found := true
                break
            }
        }
        
        if (!found) {
            MsgBox("Could not find the Receive Inventory page.", "Missing Page", "Iconx")
            return
        }
    } catch as err {
        MsgBox("Lost connection to Chrome: " err.Message)
        return
    }

    StatusText.Value := "Waking up Excel.."
    try {
        global xlApp

        xlApp := ComObject("Excel.Application")
        xlApp.Visible := false
        xlApp.DisplayAlerts := false

        wb := ""
        Loop 10 {
            try {
                wb := xlApp.Workbooks.Open(FilePathEdit.Value)
                break
            } catch as err {
                if (A_Index == 10) {
                    MsgBox("Excel is stuck or unresponsive after 5 seconds. Restart the app and try again. Details: " err.Message)
                }
                Sleep(500)
            }
        }

        StatusText.Value := "Reading Excel File..."
        Sleep(500)
        ws := ""
        Loop 10 {
            try {
                ws := wb.Sheets(1)
                break
            } catch as err {
                if (A_Index == 10) {   
                    MsgBox("Excel started but can't open the workbook. Restart the app and try again. Details: " err.Message)
                }
            }
        }
        
        lastRow := ws.Cells(ws.Rows.Count, 1).End(-4162).Row 
        totalRows := lastRow - 1 
        
        if (totalRows < 1) {
            MsgBox("No data found to process.", "Empty Sheet")
            wb.Close(0)
            xlApp.Quit()
            return
        }

        row := 2 
        currentRow := 1
        startTime := A_TickCount 
        rowStartTime := A_TickCount 
        
        formatSecs := (sec) => Format("{:02}:{:02}", Floor(sec/60), Mod(sec, 60))

        UpdateGUI() {
            elapsedSec := Round((A_TickCount - startTime) / 1000)
            remainingRows := totalRows - currentRow + 1
            expectedRowTime := 6500 ; 6000 ms (6 seconds)

            etaSec := Round(remainingRows * expectedRowTime / 1000)
            TimeText.Value := "Elapsed: " formatSecs(elapsedSec) " | ETA: " formatSecs(etaSec)
            
            chunkSize := 100 / totalRows
            baseProgress := ((currentRow - 1) / totalRows) * 100
            
            rowElapsed := A_TickCount - rowStartTime
            
            
            smoothFraction := Min(rowElapsed / expectedRowTime, 1.0)
            smoothProgress := chunkSize * 0.9 * smoothFraction
            
            ProgressBar.Value := Round(baseProgress + smoothProgress)
        }

        SetTimer(UpdateGUI, 100) ; run UpdateGUI every 100ms

        while (row <= lastRow && ws.Cells(row, 1).Text != "") {
            currentRow := row - 1
            rowStartTime := A_TickCount
            
            StatusText.Value := "Processing Row " currentRow " of " totalRows "..."
            
            metrc    := ws.Cells(row, 1).Text  
            qty      := ws.Cells(row, 4).Text  
            ndc      := ws.Cells(row, 5).Text  
            lot      := ws.Cells(row, 6).Text  
            expDate  := ws.Cells(row, 7).Text  
            packDate := ws.Cells(row, 8).Text  
            
            InjectRowData(metrc, qty, ndc, lot, expDate, packDate)
            
            Sleep(250) 
            row++
        }
        
        SetTimer(UpdateGUI, 0) ; Stop the timer
        
        wb.Close(0)
        xlApp.Quit()
        
        ProgressBar.Value := 100
        finalTime := formatSecs(Round((A_TickCount - startTime) / 1000))
        TimeText.Value := "Elapsed: " finalTime " | ETA: 00:00"
        StatusText.Value := "Import Complete! Processed " totalRows " rows."
        
        MsgBox("Finished importing items.", "Success")
    } catch as err {
        try SetTimer(UpdateGUI, 0) ; kill timer on error
        StatusText.Value := "Error reading Excel."
        MsgBox("Error: " err.Message)
    }
}

InjectRowData(metrc, qty, ndc, lot, expDate, packDate) {
    global backofficePage

    jsPart1 := '
    (
        (async function(){
            const delay = ms => new Promise(r => setTimeout(r, ms));
            const sV = async (sel, val) => {
                const el = document.querySelector(sel) || document.getElementById(sel);
                if (!el) return;
                el.focus();
                const ns = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                ns.call(el, val);
                el.dispatchEvent(new Event("input", {bubbles: true}));
                el.dispatchEvent(new Event("change", {bubbles: true}));
                await delay(150);
            };

            if (!document.querySelector("div[data-testid=receive-inventory-details_sr_product]")) {
                document.querySelector("button[data-testid=receive-inventory_button_add]")?.click();
                await delay(150);
            }

            const prod = document.querySelector("input[data-testid=receive-inventory-details_sr_product]");
            if (prod) {
                prod.focus(); prod.click(); await delay(150);
                const search = document.querySelector("input[data-testid=receive-package-modal-products-dropdown-search-input]");
                if (search) {
                    const ns = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                    ns.call(search, "%%NDC%%");
                    search.dispatchEvent(new Event("input", {bubbles: true}));
                    await delay(150);
                    const opt = document.querySelector("li[data-option-index='0']");
                    if (opt) opt.click();
                }
            }
            await delay(150);

            await sV("input[data-testid=receive-inventory-details_sr_quantity]", "%%QTY%%");
            await sV("input-input_Package ID", "%%NDC%%");
            await sV("input[data-testid=receive-inventory-details_sr_external-package-id]", "%%METRC%%");
        })();
    )'
    
    rowJs := StrReplace(jsPart1, "%%NDC%%", ndc)
    rowJs := StrReplace(rowJs, "%%QTY%%", qty)
    rowJs := StrReplace(rowJs, "%%METRC%%", metrc)
    
    backofficePage.Evaluate(rowJs)
    Sleep(1750) ; Wait for JS (750ms sleep inside rowjs) min: 1300

    HandleDateInput("input-input_Expiration date", expDate)
    HandleDateInput("input-input_Packaging date", packDate)

    jsPart2 := '
    (
        (async function(){
            const delay = ms => new Promise(r => setTimeout(r, ms));
            const el = document.getElementById("input-input_Lot name/batch ID");
            if (el) {
                el.focus();
                const ns = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                ns.call(el, "%%LOT%%");
                el.dispatchEvent(new Event("input", {bubbles: true}));
                el.dispatchEvent(new Event("change", {bubbles: true}));
            }
            await delay(150);
            
            const btn = document.querySelector("button[data-testid=receive-inventory-details_button_save]");
            if (btn && !btn.disabled) {
                btn.click();
            }
        })();
    )'
    rowJs2 := StrReplace(jsPart2, "%%LOT%%", lot)
    backofficePage.Evaluate(rowJs2)
    Sleep(500) ; Wait for JS (150ms sleep inside rowjs2)
}

HandleDateInput(id, dateValue) {
    global backofficePage
    
    jsGet := "(() => { let e = document.getElementById('" . id . "'); if(!e) return 'null'; let r = e.parentElement.getBoundingClientRect(); return r.left + ',' + r.top + ',' + r.width + ',' + r.height; })()"
    res := backofficePage.Evaluate(jsGet)
    
    if (!res.Has("value") || res["value"] == "null")
        return
        
    c := StrSplit(res["value"], ",")
    targetX := Float(c[1]) + (Float(c[3]) / 2)
    targetY := Float(c[2]) + (Float(c[4]) / 2)
    
    CDPClickCoords(backofficePage, targetX, targetY)
    Sleep(400) ; min:300
    
    ; Hardware Clear (Ctrl+A then Backspace)
    backofficePage.Call("Input.dispatchKeyEvent", Map("type", "rawKeyDown", "windowsVirtualKeyCode", 65, "modifiers", 2))
    backofficePage.Call("Input.dispatchKeyEvent", Map("type", "keyUp", "windowsVirtualKeyCode", 65, "modifiers", 2))
    backofficePage.Call("Input.dispatchKeyEvent", Map("type", "rawKeyDown", "windowsVirtualKeyCode", 8))
    backofficePage.Call("Input.dispatchKeyEvent", Map("type", "keyUp", "windowsVirtualKeyCode", 8))
    Sleep(150)

    CDPType(backofficePage, dateValue)

    backofficePage.Call("Input.dispatchKeyEvent", Map("type", "rawKeyDown", "windowsVirtualKeyCode", 13))
    backofficePage.Call("Input.dispatchKeyEvent", Map("type", "keyUp", "windowsVirtualKeyCode", 13))
    Sleep(150)

    titleRes := backofficePage.Evaluate("(() => { let e = document.getElementById('input-label_" . id . "') || document.querySelector('h2'); if(!e) return 'null'; let r = e.getBoundingClientRect(); return r.left + ',' + r.top; })()")
    if (titleRes.Has("value") && titleRes["value"] != "null") {
        tc := StrSplit(titleRes["value"], ",")
        CDPClickCoords(backofficePage, Float(tc[1])+5, Float(tc[2])+5)
    }
    Sleep(500) ; min:250
}

CDPType(page, text) {
    Loop Parse, text {
        page.Call("Input.dispatchKeyEvent", Map("type", "char", "text", A_LoopField))
        Sleep(15)
    }
}

CDPClickCoords(page, x, y) {
    page.Call("Input.dispatchMouseEvent", Map("type", "mousePressed", "x", x, "y", y, "button", "left", "clickCount", 1))
    Sleep(75)
    page.Call("Input.dispatchMouseEvent", Map("type", "mouseReleased", "x", x, "y", y, "button", "left", "clickCount", 1))
}