UrlEncode(str, sExcepts := "-_.", enc := "UTF-8") {
    buff := Buffer(StrPut(str, enc))
    StrPut(str, buff, enc)
    encoded := ""
    Loop {
        if (!b := NumGet(buff, A_Index - 1, "UChar"))
            break
        
        ; Keep alphanumeric characters and exceptions unencoded
        if (b >= 0x41 && b <= 0x5A || b >= 0x61 && b <= 0x7A || b >= 0x30 && b <= 0x39 || InStr(sExcepts, Chr(b), true)) {
            encoded .= Chr(b)
        } else {
            ; Convert special characters to standard %HEX format
            encoded .= Format("%{:02X}", b)
        }
    }
    return encoded
}