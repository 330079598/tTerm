# Decode only top-level session_id and cwd, after validating the entire JSON.
# LC_ALL=C makes percent encoding use UTF-8 bytes on every host.
function fail() { bad = 1; return "" }
function ws() { while (substr(text, pos, 1) ~ /[ \t\r\n]/ && pos <= length(text)) pos++ }
function hex4(    i,c,n) {
    n = 0
    for (i = 0; i < 4; i++) {
        c = index("0123456789abcdef", tolower(substr(text, pos++, 1))) - 1
        if (c < 0) { fail(); return 0 }
        n = n * 16 + c
    }
    return n
}
function utf8(n) {
    if (n < 128) return sprintf("%c", n)
    if (n < 2048) return sprintf("%c%c", 192 + int(n/64), 128 + n%64)
    if (n < 65536) return sprintf("%c%c%c", 224 + int(n/4096), 128 + int(n/64)%64, 128 + n%64)
    return sprintf("%c%c%c%c", 240 + int(n/262144), 128 + int(n/4096)%64, 128 + int(n/64)%64, 128 + n%64)
}
function string(    out,c,n,low) {
    if (substr(text, pos++, 1) != "\"") return fail()
    out = ""
    while (pos <= length(text) && !bad) {
        c = substr(text, pos++, 1)
        if (c == "\"") return out
        if (c ~ /[\001-\037]/) return fail()
        if (c == "\\") {
            c = substr(text, pos++, 1)
            if (c == "u") {
                n = hex4()
                if (n >= 55296 && n <= 56319) {
                    if (substr(text, pos, 1) != sprintf("%c", 92) || substr(text, pos+1, 1) != "u") return fail()
                    pos += 2; low = hex4()
                    if (low < 56320 || low > 57343) return fail()
                    n = 65536 + (n-55296)*1024 + low-56320
                } else if (n >= 56320 && n <= 57343) return fail()
                # Reject NUL before awk or a shell variable can truncate it.
                if (n == 0) return fail()
                c = utf8(n)
            } else if (c == "n") c = "\n"
            else if (c == "r") c = "\r"
            else if (c == "t") c = "\t"
            else if (c == "b") c = sprintf("%c", 8)
            else if (c == "f") c = sprintf("%c", 12)
            else if (c != "\"" && c != "\\" && c != "/") return fail()
        }
        out = out c
    }
    return fail()
}
function value(depth,    c,key,v,start,kind) {
    if (depth > 64) return fail()
    ws(); c = substr(text, pos, 1)
    if (c == "\"") return string()
    if (c == "{" || c == "[") {
        kind = c; pos++; ws()
        if (substr(text, pos, 1) == (kind == "{" ? "}" : "]")) { pos++; return "" }
        while (!bad) {
            key = ""
            if (kind == "{") {
                ws(); key = string(); ws()
                if (substr(text, pos++, 1) != ":") return fail()
            }
            ws(); start = substr(text, pos, 1); v = value(depth+1)
            if (depth == 0 && kind == "{") {
                if (key == "session_id") { if (seen_sid++) return fail(); if (start == "\"") sid = v }
                if (key == "cwd") { if (seen_cwd++) return fail(); if (start == "\"") cwd = v }
            }
            ws(); c = substr(text, pos++, 1)
            if (c == (kind == "{" ? "}" : "]")) return ""
            if (c != ",") return fail()
        }
        return ""
    }
    start = pos
    while (pos <= length(text) && substr(text, pos, 1) !~ /[ \t\r\n,}\]]/) pos++
    v = substr(text, start, pos-start)
    if (v !~ /^(null|true|false|-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?)$/) return fail()
    return ""
}
{ text = text $0 "\n" }
END {
    pos = 1; ws()
    if (substr(text, pos, 1) != "{") bad = 1
    value(0); ws()
    if (bad || pos <= length(text)) { print ";"; exit }
    if (sid !~ /^[A-Za-z0-9-]+$/ || length(sid) > 64) sid = ""
    if (length(cwd) > 16384 || cwd ~ /[\001-\037\177]/ || cwd ~ /\302[\200-\237]/) cwd = ""
    for (i = 1; i < 256; i++) bytes[sprintf("%c", i)] = i
    encoded = ""
    for (i = 1; i <= length(cwd); i++) encoded = encoded sprintf("%%%02X", bytes[substr(cwd, i, 1)])
    print sid ";" encoded
}
