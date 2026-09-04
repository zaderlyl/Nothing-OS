// Pont « bureau distant » — Discord du Mac dans Nothing OS.
//
// Capture la fenêtre Discord (ScreenCaptureKit), la réduit en 960×576, la
// quantifie vers le cube 180 couleurs du noyau (indices 76..255) et écrit
// les tuiles 32×32 modifiées (RLE) dans <partage>/.nothingos-bridge/
// frame.bin. Lit input.bin et rejoue souris/clavier vers Discord.
//
// À lancer via le bundle (build.sh) pour que la capture d'écran marche :
//     bridge/build.sh && open bridge/NothingBridge.app --args ~/Documents
// Log : ~/Library/Logs/nothing-bridge.log

import Cocoa
import CoreGraphics
import CoreMedia
import CoreVideo
import ScreenCaptureKit

let FW = 960, FH = 576
let TILE = 32
let TX = FW / TILE, TY = FH / TILE
let FPS = 12.0

// journal fichier (le bundle n'a pas de stdout visible)
let logURL = FileManager.default.homeDirectoryForCurrentUser
    .appendingPathComponent("Library/Logs/nothing-bridge.log")
func log(_ s: String) {
    let line = "\(Date()) \(s)\n"
    if let h = try? FileHandle(forWritingTo: logURL) { h.seekToEndOfFile(); h.write(line.data(using: .utf8)!); try? h.close() }
    else { try? line.data(using: .utf8)?.write(to: logURL) }
    FileHandle.standardError.write(line.data(using: .utf8)!)
}

@inline(__always) func quant(_ r: Int, _ g: Int, _ b: Int) -> UInt8 {
    UInt8(76 + min(5, r*6/256)*30 + min(5, g*6/256)*5 + min(4, b*5/256))
}

// --- protocole ----------------------------------------------------
func writeFrame(_ dir: String, seq: UInt32, cur: [UInt8], prev: [UInt8]?, full: Bool) {
    var d = Data()
    func u16(_ v: Int) { var x = UInt16(truncatingIfNeeded: v).littleEndian; withUnsafeBytes(of: &x) { d.append(contentsOf: $0) } }
    func u32(_ v: UInt32) { var x = v.littleEndian; withUnsafeBytes(of: &x) { d.append(contentsOf: $0) } }
    d.append(contentsOf: Array("NOSF".utf8))
    u32(seq); u16(FW); u16(FH); d.append(full ? 1 : 0)
    var tiles = Data(); var n = 0
    for ty in 0..<TY { for tx in 0..<TX {
        var changed = full
        if !changed, let p = prev {
            scan: for yy in 0..<TILE {
                let b = (ty*TILE+yy)*FW + tx*TILE
                for xx in 0..<TILE where cur[b+xx] != p[b+xx] { changed = true; break scan }
            }
        }
        guard changed else { continue }
        n += 1
        var raw = [UInt8](); raw.reserveCapacity(TILE*TILE)
        for yy in 0..<TILE { let b = (ty*TILE+yy)*FW + tx*TILE; raw.append(contentsOf: cur[b..<b+TILE]) }
        var rle = [UInt8](); var i = 0
        while i < raw.count {
            let v = raw[i]; var c = 1
            while i+c < raw.count && raw[i+c] == v && c < 255 { c += 1 }
            rle.append(UInt8(c)); rle.append(v); i += c
        }
        for val in [tx, ty, rle.count] { var x = UInt16(val).littleEndian; withUnsafeBytes(of: &x) { tiles.append(contentsOf: $0) } }
        tiles.append(contentsOf: rle)
    }}
    u16(n); d.append(tiles)
    let tmp = dir + "/frame.tmp", fin = dir + "/frame.bin"
    try? d.write(to: URL(fileURLWithPath: tmp))
    try? FileManager.default.removeItem(atPath: fin)
    try? FileManager.default.moveItem(atPath: tmp, toPath: fin)
}

var lastISeq: UInt32 = 0
var forceFull = false
func pumpInput(_ dir: String, _ winRect: CGRect, _ pid: pid_t) {
    guard let d = try? Data(contentsOf: URL(fileURLWithPath: dir + "/input.bin")),
          d.count >= 10, d[0]==0x4e, d[1]==0x4f, d[2]==0x53, d[3]==0x49 else { return }
    func r16(_ i: Int) -> Int { Int(d[i]) | (Int(d[i+1])<<8) }
    func r32(_ i: Int) -> UInt32 { UInt32(d[i]) | (UInt32(d[i+1])<<8) | (UInt32(d[i+2])<<16) | (UInt32(d[i+3])<<24) }
    let iseq = r32(4); if iseq == lastISeq { return }; lastISeq = iseq
    let count = r16(8); var p = 10
    let sx = winRect.width / CGFloat(FW), sy = winRect.height / CGFloat(FH)
    func scr(_ fx: Int, _ fy: Int) -> CGPoint { CGPoint(x: winRect.origin.x + CGFloat(fx)*sx, y: winRect.origin.y + CGFloat(fy)*sy) }
    let src = CGEventSource(stateID: .hidSystemState)
    for _ in 0..<count {
        guard p < d.count else { break }
        let t = d[p]; p += 1
        switch t {
        case 0x4d:
            let pt = scr(r16(p), r16(p+2)); p += 4
            CGEvent(mouseEventSource: src, mouseType: .mouseMoved, mouseCursorPosition: pt, mouseButton: .left)?.post(tap: .cghidEventTap)
        case 0x44, 0x55:
            let btn = d[p]; let pt = scr(r16(p+1), r16(p+3)); p += 5
            let down = t == 0x44
            let mt: CGEventType = btn == 1 ? (down ? .rightMouseDown : .rightMouseUp) : (down ? .leftMouseDown : .leftMouseUp)
            CGEvent(mouseEventSource: src, mouseType: mt, mouseCursorPosition: pt, mouseButton: btn == 1 ? .right : .left)?.post(tap: .cghidEventTap)
        case 0x57:
            let dy = Int8(bitPattern: d[p]); let pt = scr(r16(p+1), r16(p+3)); p += 5
            let e = CGEvent(scrollWheelEvent2Source: src, units: .line, wheelCount: 1, wheel1: Int32(dy), wheel2: 0, wheel3: 0)
            e?.location = pt; e?.post(tap: .cghidEventTap)
        case 0x46:
            forceFull = true
        case 0x4b:
            let down = d[p] == 1; let ch = d[p+1]; p += 2
            if let e = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: down) {
                if ch == 0x0a || ch == 0x0d { e.setIntegerValueField(.keyboardEventKeycode, value: 36) }
                else if ch == 0x08 { e.setIntegerValueField(.keyboardEventKeycode, value: 51) }
                else if ch == 0x1b { e.setIntegerValueField(.keyboardEventKeycode, value: 53) }
                else { var u: [UniChar] = [UniChar(ch)]; e.keyboardSetUnicodeString(stringLength: 1, unicodeString: &u) }
                if pid != 0 { e.postToPid(pid) } else { e.post(tap: .cghidEventTap) }
            }
        default: break
        }
    }
}

// --- capture continue via SCStream (delegate, pas de polling) -------
final class Cap: NSObject, SCStreamOutput, SCStreamDelegate {
    let dir: String
    let q = DispatchQueue(label: "nothing.bridge.capture")
    var stream: SCStream?
    var prev: [UInt8]?
    var seq: UInt32 = 0
    var frames = 0
    var winRect = CGRect.zero
    var askedOnce = false, waitLogged = false, restarting = false

    init(_ dir: String) { self.dir = dir; super.init() }

    func begin() {
        Task { @MainActor in
            while !CGPreflightScreenCaptureAccess() {
                if !askedOnce { askedOnce = true
                    log("[bridge] demande de l'autorisation « Enregistrement de l'ecran »…")
                    _ = CGRequestScreenCaptureAccess()
                } else if !waitLogged { waitLogged = true
                    log("[bridge] en attente — coche « NothingBridge » dans Reglages ▸ Enregistrement de l'ecran, puis QUITTE et relance")
                }
                try? await Task.sleep(nanoseconds: 1_500_000_000)
            }
            log("[bridge] autorisation OK")
            await self.startStream()
        }
    }

    func restart(_ why: String) {
        if restarting { return }
        restarting = true
        log("[bridge] \(why) — relance dans 2 s")
        try? stream?.stopCapture { _ in }
        stream = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            self?.restarting = false
            Task { @MainActor in await self?.startStream() }
        }
    }

    func startStream() async {
        guard let content = try? await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true) else {
            restart("contenu partageable indisponible"); return
        }
        var win: SCWindow? = nil; var area: CGFloat = 0
        for w in content.windows {
            guard (w.owningApplication?.applicationName ?? "").contains("Discord"), w.windowLayer == 0 else { continue }
            let a = w.frame.width * w.frame.height
            if a > area && a > 40_000 { area = a; win = w }
        }
        guard let w = win else { restart("fenetre Discord introuvable (ouvre Discord)"); return }
        winRect = w.frame

        let cfg = SCStreamConfiguration()
        cfg.width = FW; cfg.height = FH
        cfg.pixelFormat = kCVPixelFormatType_32BGRA
        cfg.showsCursor = false
        cfg.scalesToFit = true
        cfg.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(FPS))
        cfg.queueDepth = 5
        let s = SCStream(filter: SCContentFilter(desktopIndependentWindow: w), configuration: cfg, delegate: self)
        do {
            try s.addStreamOutput(self, type: .screen, sampleHandlerQueue: q)
            try await s.startCapture()
            stream = s
            log("[bridge] flux demarre — fenetre \(w.frame.size)")
        } catch {
            restart("startCapture: \(error)")
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        restart("flux arrete: \(error)")
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, CMSampleBufferIsValid(sb) else { return }

        // statut de la trame
        var complete = true
        if let arr = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
           let raw = arr.first?[.status] as? Int, let st = SCFrameStatus(rawValue: raw) {
            complete = (st == .complete)
        }

        if let px = complete ? CMSampleBufferGetImageBuffer(sb) : nil {
            CVPixelBufferLockBaseAddress(px, .readOnly)
            defer { CVPixelBufferUnlockBaseAddress(px, .readOnly) }
            guard let base = CVPixelBufferGetBaseAddress(px)?.assumingMemoryBound(to: UInt8.self) else { return }
            let bpr = CVPixelBufferGetBytesPerRow(px)
            let iw = CVPixelBufferGetWidth(px), ih = CVPixelBufferGetHeight(px)
            var cur = [UInt8](repeating: 0, count: FW * FH)
            for y in 0..<FH {
                let sy = min(ih - 1, y * ih / FH) * bpr
                for x in 0..<FW {
                    let o = sy + min(iw - 1, x * iw / FW) * 4
                    cur[y * FW + x] = quant(Int(base[o + 2]), Int(base[o + 1]), Int(base[o]))
                }
            }
            frames += 1
            if frames == 1 { log("[bridge] 1re image OK — \(Set(cur).count) couleurs") }
            if frames % 300 == 0 { log("[bridge] \(frames) images") }
            emit(cur: cur)
        } else {
            // trame « idle » : rien n'a changé → on ré-émet le dernier
            // état (garde le noyau « live » et honore les keyframes)
            if let p = prev { frames += 1; emit(cur: p) }
        }

        pumpInput(dir, winRect, discordPid())
    }

    func emit(cur: [UInt8]) {
        let full = prev == nil || forceFull || seq % 30 == 0
        forceFull = false
        seq &+= 1
        writeFrame(dir, seq: seq, cur: cur, prev: prev, full: full)
        prev = cur
    }
}

func discordPid() -> pid_t {
    for a in NSWorkspace.shared.runningApplications
    where (a.localizedName ?? "").contains("Discord") && a.activationPolicy == .regular {
        return a.processIdentifier
    }
    return 0
}

// --- démarrage --------------------------------------------------
var sharePath = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Documents").path
let cliArgs = CommandLine.arguments
if cliArgs.count >= 2 && !cliArgs[1].hasPrefix("-") { sharePath = (cliArgs[1] as NSString).expandingTildeInPath }
let dir = sharePath + "/.nothingos-bridge"
try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
log("[bridge] demarrage — partage \(dir)")

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let cap = Cap(dir)
cap.begin()
app.run()
