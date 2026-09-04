// Pont « bureau distant » entre le Mac et Nothing OS.
//
// Capture la fenêtre de Discord, la réduit, la quantifie vers le même
// cube de 180 couleurs que le noyau (indices 76..255), et écrit les
// tuiles modifiées dans <partage>/.nothingos-bridge/frame.bin.
// Lit <partage>/.nothingos-bridge/input.bin et rejoue souris/clavier
// vers Discord.
//
// Compil :  swiftc -O bridge/discord-bridge.swift -o bridge/discord-bridge
// Usage  :  bridge/discord-bridge ~/Documents        (le dossier partagé par QEMU)
//
// Permissions macOS : Enregistrement de l'écran (capture) + Accessibilité
// (injection souris/clavier). Réglages > Confidentialité et sécurité.

import Cocoa
import CoreGraphics
import ScreenCaptureKit

setvbuf(stdout, nil, _IOLBF, 0)
func log(_ s: String) { print(s); fflush(stdout) }

let FW = 960, FH = 576          // taille envoyée (multiples de 32)
let TILE = 32
let TX = FW / TILE, TY = FH / TILE
let FPS = 10.0

// --- cube de couleurs (indices 76..255, comme src/image.rs) ----------
// Pas de tramage : une UI est aplatie, et sans tramage le RLE écrase.
@inline(__always) func quant(_ r: Int, _ g: Int, _ b: Int, _ x: Int, _ y: Int) -> UInt8 {
    let ri = min(5, r * 6 / 256)
    let gi = min(5, g * 6 / 256)
    let bi = min(4, b * 5 / 256)
    return UInt8(76 + ri*30 + gi*5 + bi)
}

// --- fenêtre Discord (ScreenCaptureKit) -----------------------------
struct Win { var scw: SCWindow; var rect: CGRect }

func runSync<T>(_ op: @escaping () async throws -> T) -> T? {
    let sem = DispatchSemaphore(value: 0)
    var res: T? = nil
    Task { res = try? await op(); sem.signal() }
    sem.wait()
    return res
}

func findDiscord() -> Win? {
    guard let content = runSync({ try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true) }) else { return nil }
    var best: Win? = nil
    var bestArea: CGFloat = 0
    for w in content.windows {
        let owner = w.owningApplication?.applicationName ?? ""
        guard owner.contains("Discord"), w.windowLayer == 0 else { continue }
        let area = w.frame.width * w.frame.height
        if area > bestArea && area > 40_000 {
            bestArea = area
            best = Win(scw: w, rect: w.frame)
        }
    }
    return best
}

func discordPID() -> pid_t {
    for app in NSWorkspace.shared.runningApplications where (app.localizedName ?? "").contains("Discord") {
        if app.activationPolicy == .regular { return app.processIdentifier }
    }
    return 0
}

// --- capture + réduction -> tampon indexé --------------------------
func grabIndexed(_ win: Win) -> [UInt8]? {
    let cfg = SCStreamConfiguration()
    cfg.width = FW
    cfg.height = FH
    cfg.pixelFormat = kCVPixelFormatType_32BGRA
    cfg.showsCursor = false
    cfg.scalesToFit = true
    let filter = SCContentFilter(desktopIndependentWindow: win.scw)
    guard let img = runSync({ try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: cfg) }),
          let provider = img.dataProvider, let cfdata = provider.data else { return nil }

    let w = img.width, h = img.height
    let bpr = img.bytesPerRow
    let bpp = img.bitsPerPixel / 8
    // BGRA (kCVPixelFormatType_32BGRA) → octets B,G,R,A
    let n = CFDataGetLength(cfdata)
    guard let base = CFDataGetBytePtr(cfdata), bpp >= 3 else { return nil }

    var out = [UInt8](repeating: 0, count: FW * FH)
    for y in 0..<FH {
        let sy = min(h - 1, y * h / FH)
        let row = sy * bpr
        for x in 0..<FW {
            let sx = min(w - 1, x * w / FW)
            let o = row + sx * bpp
            if o + 2 >= n { continue }
            out[y * FW + x] = quant(Int(base[o + 2]), Int(base[o + 1]), Int(base[o]), x, y)
        }
    }
    return out
}

// --- protocole trame ---------------------------------------------
func writeFrame(_ dir: String, seq: UInt32, cur: [UInt8], prev: [UInt8]?, full: Bool) {
    var data = Data()
    func u16(_ v: Int) { var x = UInt16(v).littleEndian; withUnsafeBytes(of: &x) { data.append(contentsOf: $0) } }
    func u32(_ v: UInt32) { var x = v.littleEndian; withUnsafeBytes(of: &x) { data.append(contentsOf: $0) } }
    data.append(contentsOf: Array("NOSF".utf8))
    u32(seq); u16(FW); u16(FH); data.append(full ? 1 : 0)

    var tiles = Data()
    var n = 0
    for ty in 0..<TY {
        for tx in 0..<TX {
            var changed = full || prev == nil
            if !changed, let p = prev {
                outer: for yy in 0..<TILE {
                    let base = (ty*TILE + yy) * FW + tx*TILE
                    for xx in 0..<TILE where cur[base+xx] != p[base+xx] { changed = true; break outer }
                }
            }
            if changed {
                n += 1
                var raw = [UInt8](); raw.reserveCapacity(TILE*TILE)
                for yy in 0..<TILE {
                    let base = (ty*TILE + yy) * FW + tx*TILE
                    raw.append(contentsOf: cur[base..<base+TILE])
                }
                // RLE (count, value) — l'UI de Discord est très aplatie
                var rle = [UInt8](); var i = 0
                while i < raw.count {
                    let v = raw[i]; var c = 1
                    while i + c < raw.count && raw[i+c] == v && c < 255 { c += 1 }
                    rle.append(UInt8(c)); rle.append(v); i += c
                }
                var t = Data()
                for val in [tx, ty, rle.count] {
                    var x = UInt16(val).littleEndian; withUnsafeBytes(of: &x) { t.append(contentsOf: $0) }
                }
                t.append(contentsOf: rle)
                tiles.append(t)
            }
        }
    }
    u16(n); data.append(tiles)

    let tmp = dir + "/frame.tmp", fin = dir + "/frame.bin"
    try? data.write(to: URL(fileURLWithPath: tmp))
    try? FileManager.default.removeItem(atPath: fin)
    try? FileManager.default.moveItem(atPath: tmp, toPath: fin)
}

// --- entrées : rejoue input.bin vers Discord --------------------
var lastISeq: UInt32 = 0
var forceFull = false
func pumpInput(_ dir: String, _ win: Win, _ pid: pid_t) {
    let path = dir + "/input.bin"
    guard let d = try? Data(contentsOf: URL(fileURLWithPath: path)), d.count >= 8,
          d[0] == 0x4e, d[1] == 0x4f, d[2] == 0x53, d[3] == 0x49 else { return }
    func rd16(_ i: Int) -> Int { Int(d[i]) | (Int(d[i+1]) << 8) }
    func rd32(_ i: Int) -> UInt32 { UInt32(d[i]) | (UInt32(d[i+1])<<8) | (UInt32(d[i+2])<<16) | (UInt32(d[i+3])<<24) }
    let iseq = rd32(4)
    if iseq == lastISeq { return }
    lastISeq = iseq
    let count = rd16(8)
    var p = 10
    let sx = win.rect.width / CGFloat(FW), sy = win.rect.height / CGFloat(FH)
    func scr(_ fx: Int, _ fy: Int) -> CGPoint {
        CGPoint(x: win.rect.origin.x + CGFloat(fx) * sx, y: win.rect.origin.y + CGFloat(fy) * sy)
    }
    let src = CGEventSource(stateID: .hidSystemState)
    for _ in 0..<count {
        guard p < d.count else { break }
        let t = d[p]; p += 1
        switch t {
        case 0x4d: // M x y
            let pt = scr(rd16(p), rd16(p+2)); p += 4
            CGEvent(mouseEventSource: src, mouseType: .mouseMoved, mouseCursorPosition: pt, mouseButton: .left)?.post(tap: .cghidEventTap)
        case 0x44, 0x55: // D/U button x y
            let btn = d[p]; let pt = scr(rd16(p+1), rd16(p+3)); p += 5
            let down = t == 0x44
            let mt: CGEventType = btn == 1 ? (down ? .rightMouseDown : .rightMouseUp) : (down ? .leftMouseDown : .leftMouseUp)
            let mb: CGMouseButton = btn == 1 ? .right : .left
            CGEvent(mouseEventSource: src, mouseType: mt, mouseCursorPosition: pt, mouseButton: mb)?.post(tap: .cghidEventTap)
        case 0x57: // W dy x y
            let dy = Int8(bitPattern: d[p]); let pt = scr(rd16(p+1), rd16(p+3)); p += 5
            let e = CGEvent(scrollWheelEvent2Source: src, units: .line, wheelCount: 1, wheel1: Int32(dy), wheel2: 0, wheel3: 0)
            e?.location = pt; e?.post(tap: .cghidEventTap)
        case 0x46: // F : le noyau demande une trame complete
            forceFull = true
        case 0x4b: // K down ascii
            let down = d[p] == 1; let ch = d[p+1]; p += 2
            if let e = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: down) {
                if ch == 0x0a || ch == 0x0d { e.setIntegerValueField(.keyboardEventKeycode, value: 36) }
                else if ch == 0x08 { e.setIntegerValueField(.keyboardEventKeycode, value: 51) }
                else if ch == 0x1b { e.setIntegerValueField(.keyboardEventKeycode, value: 53) }
                else {
                    var u: [UniChar] = [UniChar(ch)]
                    e.keyboardSetUnicodeString(stringLength: 1, unicodeString: &u)
                }
                if pid != 0 { e.postToPid(pid) } else { e.post(tap: .cghidEventTap) }
            }
        default:
            break
        }
    }
}

// --- boucle -----------------------------------------------------
let args = CommandLine.arguments
guard args.count >= 2 else { print("usage: discord-bridge <dossier-partage>"); exit(2) }
let dir = (args[1] as NSString).expandingTildeInPath + "/.nothingos-bridge"
try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
log("[bridge] partage : \(dir)")

// contexte GUI minimal (SCK + CGEvent en ont besoin)
let app = NSApplication.shared
app.setActivationPolicy(.accessory)

var prev: [UInt8]? = nil
var seq: UInt32 = 0
var missing = 0

var diag = 0
func tick() {
    if let win = findDiscord() {
        let pid = discordPID()
        pumpInput(dir, win, pid)   // avant la capture : prend en compte 'F'
        if let cur = grabIndexed(win) {
            seq &+= 1
            let full = prev == nil || forceFull || seq % 40 == 0
            writeFrame(dir, seq: seq, cur: cur, prev: prev, full: full)
            prev = cur
            if missing != 0 { log("[bridge] capture OK (\(FW)x\(FH))") }
            missing = 0
            diag += 1
            let distinct = Set(cur).count
            if diag == 1 || (full && diag < 6) {
                if distinct <= 3 {
                    log("[bridge] ⚠️  \(distinct) couleurs — accorde 'Enregistrement de l'ecran' au Terminal puis relance")
                } else {
                    log("[bridge] keyframe \(seq) envoyee (\(distinct) couleurs)")
                }
            }
            if forceFull { log("[bridge] keyframe sur demande du noyau (seq \(seq))") }
            forceFull = false
        } else if missing == 0 {
            log("[bridge] capture nil — accorde 'Enregistrement de l'ecran' au terminal, puis relance")
            missing += 1
        }
    } else {
        if missing == 0 { log("[bridge] fenetre Discord introuvable — ouvre Discord") }
        missing += 1
    }
}

Timer.scheduledTimer(withTimeInterval: 1.0 / FPS, repeats: true) { _ in tick() }
app.run()
