import AppKit
import ApplicationServices
import Carbon.HIToolbox

struct Failure: Error {
    let message: String
    let status: Int32
}

let namedKeys: [String: Int] = [
    "return": kVK_Return, "escape": kVK_Escape, "tab": kVK_Tab, "space": kVK_Space, "delete": kVK_Delete,
    "left": kVK_LeftArrow, "right": kVK_RightArrow, "up": kVK_UpArrow, "down": kVK_DownArrow,
    "pageup": kVK_PageUp, "pagedown": kVK_PageDown, "home": kVK_Home, "end": kVK_End, "us-quote": kVK_ANSI_Quote,
]

let letters = "abcdefghijklmnopqrstuvwxyz".map { String($0) }
let digits = "0123456789".map { String($0) }
let symbolNames = ["comma", "slash", "question", "minus", "equal", "plus", "leftbracket", "rightbracket"]
let positionNames = [
    "return", "escape", "tab", "space", "delete", "left", "right", "up", "down", "pageup", "pagedown", "home", "end", "us-quote",
]
let keyGrammar = "a…z 0…9 " + (symbolNames + positionNames).joined(separator: " ")

let usage =
    "usage: manual-check window|covers|front|size|key|hold|text|paste|press|drag|scroll PID … | self-test "
    + "(key: KEY [cmd] [shift] [alt] [ctrl]; hold: KEY [MS]; KEY: \(keyGrammar); paste: TEXT; drag: x0 y0 x1 y1 [right] [hold MS])"

let characterNames: [String: Character] = {
    var names: [String: Character] = [
        "comma": ",", "slash": "/", "question": "?", "minus": "-", "equal": "=", "plus": "+", "leftbracket": "[",
        "rightbracket": "]",
    ]
    for name in letters + digits { names[name] = Character(name) }
    return names
}()

let mainBlock: [Int] = Array(0...35) + Array(37...47) + [50, 93, 94]

let commandState = UInt32(cmdKey >> 8) & 0xFF
let shiftState = UInt32(shiftKey >> 8) & 0xFF
let optionState = UInt32(optionKey >> 8) & 0xFF

let modifiers: [String: CGEventFlags] = [
    "cmd": .maskCommand, "shift": .maskShift, "alt": .maskAlternate, "ctrl": .maskControl,
]

struct Window {
    let id: Int
    let frame: CGRect
}

func windowList() -> [[String: Any]] {
    let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
    return CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
}

func bounds(_ info: [String: Any]) -> CGRect? {
    guard let dict = info[kCGWindowBounds as String] as? NSDictionary else { return nil }
    return CGRect(dictionaryRepresentation: dict as CFDictionary)
}

struct Shown {
    let id: Int
    let pid: pid_t
    let owner: String
    let layer: Int
    let alpha: Double
    let frame: CGRect
}

struct Screen {
    let displays: [CGRect]
    let visible: [CGRect]
}

func shown(_ info: [String: Any]) -> Shown? {
    guard let pid = info[kCGWindowOwnerPID as String] as? pid_t, let frame = bounds(info) else { return nil }
    return Shown(
        id: info[kCGWindowNumber as String] as? Int ?? 0, pid: pid,
        owner: info[kCGWindowOwnerName as String] as? String ?? "",
        layer: info[kCGWindowLayer as String] as? Int ?? 0,
        alpha: info[kCGWindowAlpha as String] as? Double ?? 1, frame: frame)
}

func onScreen() -> [Shown] {
    windowList().compactMap(shown)
}

func window(of pid: pid_t) throws -> Window {
    guard let found = onScreen().first(where: { $0.pid == pid && $0.layer == 0 }) else {
        throw Failure(message: "process \(pid) shows no window", status: 1)
    }
    return Window(id: found.id, frame: found.frame)
}

func displayFrames() -> [CGRect] {
    var ids = [CGDirectDisplayID](repeating: 0, count: 16)
    var count: UInt32 = 0
    CGGetActiveDisplayList(16, &ids, &count)
    return ids.prefix(Int(count)).map { CGDisplayBounds($0) }
}

func visibleAreas() -> [CGRect] {
    guard let main = NSScreen.screens.first else { return [] }
    return NSScreen.screens.map { screen in
        let visible = screen.visibleFrame
        let top = main.frame.maxY - visible.maxY
        return CGRect(x: visible.minX, y: top, width: visible.width, height: visible.height)
    }
}

func screen() -> Screen {
    Screen(displays: displayFrames(), visible: visibleAreas())
}

func isPassedOver(_ window: Shown, on screen: Screen) -> Bool {
    window.alpha == 0 || (window.owner == "Dock" && screen.displays.contains(window.frame))
}

func inTheWay(at point: CGPoint, of pid: pid_t, in list: [Shown], on screen: Screen) -> [Shown]? {
    var found: [Shown] = []
    for window in list where window.frame.contains(point) && !isPassedOver(window, on: screen) {
        if window.pid == pid { return found }
        found.append(window)
    }
    return nil
}

func area(_ rect: CGRect) -> CGRect? {
    rect.isNull || rect.width <= 0 || rect.height <= 0 ? nil : rect
}

func visiblePart(of window: CGRect, over frame: CGRect, on screen: Screen) -> CGRect? {
    let overlap = window.intersection(frame)
    let parts = screen.visible.compactMap { area(overlap.intersection($0)) }
    guard let first = parts.first else { return nil }
    let joined = parts.dropFirst().reduce(first) { $0.union($1) }
    return joined.offsetBy(dx: -frame.minX, dy: -frame.minY)
}

func coversBeforeFront(of pid: pid_t, frame: CGRect, in list: [Shown], on screen: Screen) -> [(window: Shown, part: CGRect)] {
    list.compactMap { window in
        guard window.layer > 0, window.pid != pid, !isPassedOver(window, on: screen),
            let part = visiblePart(of: window.frame, over: frame, on: screen)
        else { return nil }
        return (window, part)
    }
}

func processName(_ pid: pid_t) -> String? {
    var buffer = [CChar](repeating: 0, count: 4096)
    guard proc_pidpath(pid, &buffer, UInt32(buffer.count)) > 0 else { return nil }
    return String(cString: buffer).split(separator: "/").last.map(String.init)
}

func named(_ window: Shown, _ process: String?) -> String {
    let middle = process.map { ", \($0)" } ?? ""
    return "\(window.owner) (pid \(window.pid)\(middle), layer \(window.layer))"
}

func coverRefusal(_ x: String, _ y: String, _ names: [String]) -> String {
    guard names.count > 1 else { return "another window covers \(x),\(y): \(names.joined()): nothing was posted" }
    return "other windows cover \(x),\(y), front to back: \(names.joined(separator: "; ")): nothing was posted"
}

func coverLines(_ found: [(name: String, part: CGRect)]) -> String {
    found.map { name, part in
        let size = "\(Int(part.minX)),\(Int(part.minY)) \(Int(part.width))×\(Int(part.height))"
        return "\(name) covers \(size) of the window: a press or a scroll there is refused"
    }.joined(separator: "\n")
}

func scale(of frame: CGRect) -> CGFloat {
    var id: CGDirectDisplayID = 0
    var count: UInt32 = 0
    CGGetDisplaysWithRect(frame, 1, &id, &count)
    let screen = NSScreen.screens.first {
        ($0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value == id
    }
    return (count > 0 ? screen : NSScreen.main)?.backingScaleFactor ?? 1
}

func isInsideVisibleArea(_ point: CGPoint) -> Bool {
    let areas = visibleAreas()
    return areas.isEmpty || areas.contains { $0.contains(point) }
}

func isDebugBuild(_ pid: pid_t) -> Bool {
    var buffer = [CChar](repeating: 0, count: 4096)
    guard proc_pidpath(pid, &buffer, UInt32(buffer.count)) > 0 else { return false }
    return String(cString: buffer).hasSuffix("/target/debug/cornercase-gpui")
}

func checkedPid(_ pid: pid_t) throws {
    guard isDebugBuild(pid) else {
        throw Failure(message: "process \(pid) is not a target/debug/cornercase-gpui: nothing was done", status: 2)
    }
}

func allowed() throws {
    guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else {
        throw Failure(
            message: "this process may not post events: allow it in System Settings › Privacy & Security › Accessibility",
            status: 1)
    }
}

func isFront(_ pid: pid_t) -> Bool {
    RunLoop.main.run(until: Date())
    return NSWorkspace.shared.frontmostApplication?.processIdentifier == pid
}

func front(_ pid: pid_t) throws {
    try allowed()
    let app = AXUIElementCreateApplication(pid)
    let result = AXUIElementSetAttributeValue(app, kAXFrontmostAttribute as CFString, kCFBooleanTrue)
    guard result == .success else {
        throw Failure(message: "process \(pid) refused the front (\(result.rawValue))", status: 1)
    }
    for _ in 0..<20 where !isFront(pid) {
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
    }
    guard isFront(pid) else { throw Failure(message: "process \(pid) did not come to the front", status: 3) }
}

func number(_ text: String, _ what: String) throws -> Double {
    guard let value = Double(text), value.isFinite else {
        throw Failure(message: "\(what) must be a number, not '\(text)'", status: 2)
    }
    return value
}

func wholeNumber(_ text: String, _ what: String) throws -> Int32 {
    guard let value = Int32(text) else {
        throw Failure(message: "\(what) must be a whole number, not '\(text)'", status: 2)
    }
    return value
}

func clear(_ pid: pid_t, at point: CGPoint, _ x: String, _ y: String) throws {
    guard isFront(pid) else { throw Failure(message: "process \(pid) is not in front: nothing was posted", status: 3) }
    guard let found = inTheWay(at: point, of: pid, in: onScreen(), on: screen()) else {
        throw Failure(message: "no window of process \(pid) is at \(x),\(y): nothing was posted", status: 3)
    }
    guard found.isEmpty else {
        let names = found.map { named($0, processName($0.pid)) }
        throw Failure(message: coverRefusal(x, y, names), status: 3)
    }
}

func target(_ pid: pid_t, _ x: String, _ y: String) throws -> CGPoint {
    try allowed()
    let frame = try window(of: pid).frame
    let dx = try number(x, "x")
    let dy = try number(y, "y")
    guard dx >= 0, dy >= 0, dx < frame.width, dy < frame.height else {
        throw Failure(message: "\(x),\(y) is outside the window (\(Int(frame.width))×\(Int(frame.height)))", status: 2)
    }
    let point = CGPoint(x: frame.minX + dx, y: frame.minY + dy)
    guard isInsideVisibleArea(point) else {
        throw Failure(message: "\(x),\(y) is under the Dock or the menu bar: nothing was posted", status: 3)
    }
    try clear(pid, at: point, x, y)
    return point
}

func post(_ type: CGEventType, at point: CGPoint, button: CGMouseButton = .left) {
    CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: button)?
        .post(tap: .cghidEventTap)
}

func hover(_ pid: pid_t, at point: CGPoint, _ x: String, _ y: String) throws {
    post(.mouseMoved, at: point)
    usleep(150_000)
    try clear(pid, at: point, x, y)
}

func keepingPointer(_ body: () throws -> Void) throws {
    let saved = CGEvent(source: nil)?.location
    defer { if let saved { CGWarpMouseCursorPosition(saved) } }
    try body()
}

func press(_ pid: pid_t, _ x: String, _ y: String, right: Bool) throws {
    let point = try target(pid, x, y)
    let button: CGMouseButton = right ? .right : .left
    try keepingPointer {
        try hover(pid, at: point, x, y)
        post(right ? .rightMouseDown : .leftMouseDown, at: point, button: button)
        usleep(50_000)
        post(right ? .rightMouseUp : .leftMouseUp, at: point, button: button)
        usleep(100_000)
    }
}

func drag(_ pid: pid_t, from start: (String, String), to end: (String, String), right: Bool, hold: Int) throws {
    let origin = try target(pid, start.0, start.1)
    let goal = try target(pid, end.0, end.1)
    let button: CGMouseButton = right ? .right : .left
    let (down, moved, up): (CGEventType, CGEventType, CGEventType) =
        right ? (.rightMouseDown, .rightMouseDragged, .rightMouseUp) : (.leftMouseDown, .leftMouseDragged, .leftMouseUp)
    try keepingPointer {
        try hover(pid, at: origin, start.0, start.1)
        try clear(pid, at: goal, end.0, end.1)
        post(down, at: origin, button: button)
        let steps = 12
        for step in 1...steps {
            let fraction = CGFloat(step) / CGFloat(steps)
            let point = CGPoint(
                x: origin.x + (goal.x - origin.x) * fraction, y: origin.y + (goal.y - origin.y) * fraction)
            post(moved, at: point, button: button)
            usleep(21_000)
        }
        usleep(250_000 + UInt32(hold) * 1_000)
        post(up, at: goal, button: button)
        print(String(format: "%.3f", Date().timeIntervalSince1970))
        usleep(100_000)
    }
}

func scroll(_ pid: pid_t, _ x: String, _ y: String, dy: Int32, dx: Int32) throws {
    let point = try target(pid, x, y)
    try keepingPointer {
        try hover(pid, at: point, x, y)
        let event = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0)
        event?.location = point
        event?.post(tap: .cghidEventTap)
        usleep(100_000)
    }
}

struct Typed: Equatable {
    let text: String
    let dead: Bool
}

struct Layout {
    let name: String?
    let types: ((Int, UInt32) -> Typed)?
}

struct Reading: Equatable {
    let key: String
    let shift: Bool
    let dead: Bool
}

struct Posting: Equatable {
    let code: Int
    let flags: CGEventFlags
}

func isASCII(_ text: String) -> Bool {
    text.unicodeScalars.allSatisfy { $0.isASCII }
}

func asciiUppercased(_ text: String) -> String {
    String(String.UnicodeScalarView(text.unicodeScalars.map { ($0.value >= 97 && $0.value <= 122) ? Unicode.Scalar($0.value - 32)! : $0 }))
}

func isASCIILowercase(_ text: String) -> Bool {
    text.unicodeScalars.allSatisfy { $0.value >= 97 && $0.value <= 122 }
}

func windowKey(_ types: (Int, UInt32) -> Typed, code: Int, command: Bool, shift: Bool) -> Reading {
    var plain = types(code, 0)
    var shifted = types(code, shiftState)
    let alwaysCommand = !isASCII(types(0, 0).text) && isASCII(types(0, commandState).text)
    if command || alwaysCommand {
        let withCommand = types(code, commandState)
        let withBoth = types(code, commandState | shiftState)
        if withBoth.text != withCommand.text {
            shifted = withBoth
        } else if asciiUppercased(withCommand.text) != withCommand.text {
            shifted = Typed(text: asciiUppercased(withCommand.text), dead: withCommand.dead)
        }
        plain = withCommand
    }
    if shift && isASCIILowercase(plain.text) { return Reading(key: plain.text, shift: true, dead: plain.dead) }
    if shift { return Reading(key: shifted.text, shift: false, dead: shifted.dead) }
    return Reading(key: plain.text, shift: false, dead: plain.dead)
}

func shortcutKey(_ types: (Int, UInt32) -> Typed, _ wanted: String, command: Bool, shift: Bool) -> (code: Int, shift: Bool)? {
    for used in [false, true] {
        for code in mainBlock {
            let read = windowKey(types, code: code, command: command, shift: used)
            if read.key == wanted && read.shift == shift && !read.dead { return (code, used) }
        }
    }
    return nil
}

func typingKey(
    _ types: (Int, UInt32) -> Typed, _ wanted: String, dead: Bool = false, states: [UInt32] = [0, shiftState, optionState, shiftState | optionState]
) -> (code: Int, state: UInt32)? {
    for state in states {
        for code in mainBlock {
            let typed = types(code, state)
            if typed.text == wanted && typed.dead == dead { return (code, state) }
        }
    }
    return nil
}

func stateFlags(_ state: UInt32) -> CGEventFlags {
    var flags = CGEventFlags()
    if state & shiftState != 0 { flags.insert(.maskShift) }
    if state & optionState != 0 { flags.insert(.maskAlternate) }
    return flags
}

func unknownKey(_ name: String) -> Failure {
    var hint = ""
    if let stands = characterNames.first(where: { name.count == 1 && $0.value == Character(name) })?.key {
        hint = "its name is \(stands); "
    } else if name == "quote" {
        hint = "quote is now us-quote, the key where a US keyboard has ' (a dead key on Spanish ISO); "
    }
    return Failure(message: "unknown key \(name): \(hint)the keys are \(keyGrammar)", status: 2)
}

func unknownModifier(_ word: String) -> Failure {
    Failure(message: "unknown modifier \(word): the modifiers are cmd, shift, alt and ctrl", status: 2)
}

func nameOf(_ reading: String) -> String? {
    guard reading.count == 1 else { return nil }
    return characterNames.first(where: { $0.value == Character(reading) })?.key
}

func shiftRefusal(_ name: String, _ character: String, _ reading: String?) -> Failure {
    let head = "shift with \(name) is another character: the window reads shift and \(character)"
    guard let reading else {
        return Failure(message: head + " as the character they type: nothing was posted", status: 2)
    }
    let tail = nameOf(reading).map { "so name it: \($0)" } ?? "which no name posts and no shortcut takes"
    return Failure(message: "\(head) as \(reading), \(tail): nothing was posted", status: 2)
}

func noLayoutRefusal(_ name: String) -> Failure {
    Failure(message: "no keyboard layout is in use, so no key can be found for \(name): named keys still work, nothing was posted", status: 1)
}

func noKeyDataRefusal(_ name: String, _ layout: String) -> Failure {
    Failure(
        message: "the keyboard layout in use (\(layout)) has no key data, as an input method's may not, so no key can be found for \(name): named keys still work, nothing was posted",
        status: 1)
}

func altOnlyRefusal(_ name: String, _ layout: String, _ reading: String) -> Failure {
    Failure(
        message: "\(name) is typed only with alt on \(layout), and with cmd or ctrl held the window reads that key without alt, as \(reading): this shortcut cannot reach the window on this layout, nothing was posted",
        status: 1)
}

func advice(_ shortcut: Bool) -> String {
    shortcut ? "this shortcut cannot reach the window on this layout" : "`text` types it"
}

func deadRefusal(_ name: String, _ layout: String, shortcut: Bool = false) -> Failure {
    Failure(message: "\(name) is behind a dead key on \(layout): \(advice(shortcut)), nothing was posted", status: 1)
}

func missingRefusal(_ name: String, _ layout: String, shortcut: Bool = false) -> Failure {
    Failure(message: "no key of \(layout) types \(name): \(advice(shortcut)), nothing was posted", status: 1)
}

func noShortcutRefusal(_ name: String, _ layout: String) -> Failure {
    Failure(message: "no key of \(layout) makes the window read \(name) with these modifiers: nothing was posted", status: 1)
}

func whyNoShortcut(_ types: (Int, UInt32) -> Typed, _ wanted: String, _ name: String, _ layout: String, command: Bool) -> Failure {
    if let typed = typingKey(types, wanted) {
        guard typed.state & optionState != 0 else { return noShortcutRefusal(name, layout) }
        let read = windowKey(types, code: typed.code, command: command, shift: typed.state & shiftState != 0)
        return altOnlyRefusal(name, layout, "\(command ? "cmd" : "ctrl")-\(read.key)")
    }
    return typingKey(types, wanted, dead: true) == nil
        ? missingRefusal(name, layout, shortcut: true) : deadRefusal(name, layout, shortcut: true)
}

func resolveCharacter(_ name: String, _ wanted: Character, _ asked: CGEventFlags, layout: Layout) throws -> Posting {
    let shortcut = asked.contains(.maskCommand) || asked.contains(.maskControl)
    let shift = asked.contains(.maskShift)
    guard let types = layout.types, let id = layout.name else {
        throw layout.name.map { noKeyDataRefusal(name, $0) } ?? noLayoutRefusal(name)
    }
    if shortcut {
        let command = asked.contains(.maskCommand)
        guard let found = shortcutKey(types, String(wanted), command: command, shift: shift) else {
            throw whyNoShortcut(types, String(wanted), name, id, command: command)
        }
        return Posting(code: found.code, flags: asked.union(found.shift ? .maskShift : []))
    }
    let typedText = shift ? String(wanted).uppercased() : String(wanted)
    let states = asked.contains(.maskAlternate) ? [0, shiftState] : [0, shiftState, optionState, shiftState | optionState]
    guard let found = typingKey(types, typedText, states: states) else {
        throw typingKey(types, typedText, dead: true) == nil ? missingRefusal(name, id) : deadRefusal(name, id)
    }
    return Posting(code: found.code, flags: asked.union(stateFlags(found.state)))
}

func shiftReading(_ layout: Layout, _ wanted: Character, command: Bool) -> String? {
    guard let types = layout.types else { return nil }
    let text = String(wanted)
    guard let code = shortcutKey(types, text, command: command, shift: false)?.code ?? typingKey(types, text)?.code else { return nil }
    return windowKey(types, code: code, command: command, shift: true).key
}

func resolve(_ name: String, _ held: [String], layout: () -> Layout) throws -> Posting {
    var asked = CGEventFlags()
    for word in held {
        guard let flag = modifiers[word] else { throw unknownModifier(word) }
        asked.insert(flag)
    }
    if let code = namedKeys[name] { return Posting(code: code, flags: asked) }
    guard let wanted = characterNames[name] else { throw unknownKey(name) }
    let shortcut = asked.contains(.maskCommand) || asked.contains(.maskControl)
    let found = layout()
    if asked.contains(.maskShift) && !wanted.isLetter {
        throw shiftRefusal(name, String(wanted), shortcut ? shiftReading(found, wanted, command: asked.contains(.maskCommand)) : nil)
    }
    return try resolveCharacter(name, wanted, asked, layout: found)
}

func translate(_ data: Data, _ code: Int, _ state: UInt32) -> Typed {
    var dead: UInt32 = 0
    var length = 0
    var buffer = [UniChar](repeating: 0, count: 4)
    let status = data.withUnsafeBytes { raw -> OSStatus in
        guard let base = raw.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else { return -1 }
        return UCKeyTranslate(
            base, UInt16(code), UInt16(kUCKeyActionDown), state, UInt32(LMGetKbdType()), 0, &dead, buffer.count, &length, &buffer)
    }
    guard status == noErr else { return Typed(text: "", dead: false) }
    guard dead != 0 else { return Typed(text: String(utf16CodeUnits: buffer, count: length), dead: false) }
    let spacing = data.withUnsafeBytes { raw -> OSStatus in
        guard let base = raw.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else { return -1 }
        return UCKeyTranslate(
            base, UInt16(kVK_Space), UInt16(kUCKeyActionDown), 0, UInt32(LMGetKbdType()), 0, &dead, buffer.count, &length, &buffer)
    }
    return Typed(text: spacing == noErr ? String(utf16CodeUnits: buffer, count: length) : "", dead: true)
}

func layoutInUse() -> Layout {
    guard let source = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue() else {
        return Layout(name: nil, types: nil)
    }
    return withExtendedLifetime(source) {
        let id = TISGetInputSourceProperty(source, kTISPropertyInputSourceID).map {
            Unmanaged<CFString>.fromOpaque($0).takeUnretainedValue() as String
        } ?? "unnamed"
        guard let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else {
            return Layout(name: id, types: nil)
        }
        let cf = Unmanaged<CFData>.fromOpaque(raw).takeUnretainedValue()
        let data = Data(bytes: CFDataGetBytePtr(cf), count: CFDataGetLength(cf))
        return Layout(name: id, types: { code, state in translate(data, code, state) })
    }
}

func postKey(_ pid: pid_t, _ posting: Posting) throws {
    try allowed()
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(posting.code), keyDown: down)
        event?.flags = posting.flags
        event?.postToPid(pid)
        usleep(30_000)
    }
}

func key(_ pid: pid_t, _ name: String, _ held: [String]) throws {
    try postKey(pid, try resolve(name, held, layout: layoutInUse))
}

func holdRefusal(_ word: String) -> Failure {
    Failure(message: "hold takes no modifiers (\(word)): hold PID KEY [MS]; key posts a key with modifiers", status: 2)
}

func hold(_ pid: pid_t, _ name: String, _ rest: [String]) throws {
    if let word = rest.first(where: { modifiers[$0] != nil }) { throw holdRefusal(word) }
    guard rest.count <= 1 else { throw Failure(message: usage, status: 2) }
    let posting = try resolve(name, [], layout: layoutInUse)
    let ms = try rest.first.map { Int(try wholeNumber($0, "ms")) } ?? 800
    guard (0...10_000).contains(ms) else { throw Failure(message: "ms must be 0 to 10000", status: 2) }
    try allowed()
    func post(_ down: Bool, repeating: Bool) {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(posting.code), keyDown: down)
        if !posting.flags.isEmpty { event?.flags = posting.flags }
        if repeating { event?.setIntegerValueField(.keyboardEventAutorepeat, value: 1) }
        event?.postToPid(pid)
    }
    post(true, repeating: false)
    for _ in 0..<(ms / 33) {
        usleep(33_000)
        post(true, repeating: true)
    }
    post(false, repeating: false)
}

func saved(_ board: NSPasteboard) -> [[(NSPasteboard.PasteboardType, Data)]] {
    (board.pasteboardItems ?? []).map { item in item.types.compactMap { type in item.data(forType: type).map { (type, $0) } } }
}

func restore(_ items: [[(NSPasteboard.PasteboardType, Data)]], to board: NSPasteboard) throws {
    board.clearContents()
    let rebuilt = items.map { entries -> NSPasteboardItem in
        let item = NSPasteboardItem()
        for (type, data) in entries { item.setData(data, forType: type) }
        return item
    }
    guard rebuilt.isEmpty || board.writeObjects(rebuilt) else {
        throw Failure(message: "the clipboard could not be given back: tell the person who runs this check at once", status: 1)
    }
}

func paste(_ pid: pid_t, _ typed: String) throws {
    try allowed()
    let posting = try resolve("v", ["cmd"], layout: layoutInUse)
    let board = NSPasteboard.general
    let kept = saved(board)
    let held = (board.pasteboardItems ?? []).map { $0.types.count }
    guard kept.map({ $0.count }) == held else {
        throw Failure(message: "the clipboard holds data that cannot be read back: nothing was done", status: 1)
    }
    board.clearContents()
    board.setString(typed, forType: .string)
    board.setData(Data(), forType: NSPasteboard.PasteboardType("org.nspasteboard.TransientType"))
    try postKey(pid, posting)
    usleep(300_000)
    try restore(kept, to: board)
    let back = saved(board)
    guard back.count == kept.count, zip(back, kept).allSatisfy({ $0.count == $1.count }) else {
        throw Failure(message: "the clipboard differs after it was given back: tell the person who runs this check at once", status: 1)
    }
    print("clipboard: \(kept.count) items, \(kept.reduce(0) { $0 + $1.count }) types, given back")
}

func text(_ pid: pid_t, _ typed: String) throws {
    try allowed()
    for grapheme in typed {
        let units = Array(String(grapheme).utf16)
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)
            event?.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
            event?.postToPid(pid)
            usleep(30_000)
        }
    }
}

func size(_ pid: pid_t, _ width: String, _ height: String) throws {
    var wanted = CGSize(width: try number(width, "width"), height: try number(height, "height"))
    try allowed()
    var value: CFTypeRef?
    let app = AXUIElementCreateApplication(pid)
    guard AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value) == .success,
        let windows = value as? [AXUIElement], let first = windows.first,
        let size = AXValueCreate(.cgSize, &wanted),
        AXUIElementSetAttributeValue(first, kAXSizeAttribute as CFString, size) == .success
    else { throw Failure(message: "process \(pid) refused the size", status: 1) }
}

func describe(_ pid: pid_t) throws {
    let found = try window(of: pid)
    let frame = found.frame
    print(found.id, Int(frame.minX), Int(frame.minY), Int(frame.width), Int(frame.height), Int(scale(of: frame)))
}

func covers(_ pid: pid_t) throws {
    let frame = try window(of: pid).frame
    let found = coversBeforeFront(of: pid, frame: frame, in: onScreen(), on: screen())
    guard found.isEmpty else {
        let lines = found.map { (name: named($0.window, processName($0.window.pid)), part: $0.part) }
        throw Failure(message: coverLines(lines), status: 3)
    }
}

func button(_ args: [String], after count: Int) throws -> Bool {
    guard args.count == count + 1 else { return false }
    guard args[count] == "right" else {
        throw Failure(message: "\(args[0]) takes only 'right' after the point", status: 2)
    }
    return true
}

func dragOptions(_ words: [String]) throws -> (right: Bool, hold: Int) {
    var rest = words[...]
    let right = rest.first == "right"
    if right { rest = rest.dropFirst() }
    guard !rest.isEmpty else { return (right, 0) }
    guard rest.count == 2, rest.first == "hold", let last = rest.last else {
        throw Failure(message: "drag takes only 'right' and 'hold MS' after the points", status: 2)
    }
    let ms = Int(try wholeNumber(last, "hold"))
    guard (0...10_000).contains(ms) else { throw Failure(message: "hold must be 0 to 10000 ms", status: 2) }
    return (right, ms)
}

enum SelfTest {
    static let testDebug: pid_t = 4242
    static let testDisplay = CGRect(x: 0, y: 0, width: 1512, height: 982)
    static let testVisible = CGRect(x: 0, y: 34, width: 1512, height: 912)
    static let testScreen = Screen(displays: [testDisplay], visible: [testVisible])

    static func made(_ id: Int, _ pid: pid_t, _ owner: String, _ layer: Int, _ frame: CGRect, alpha: Double = 1) -> Shown {
        Shown(id: id, pid: pid, owner: owner, layer: layer, alpha: alpha, frame: frame)
    }

    static func debugWindow(at y: CGFloat = 34) -> Shown {
        made(100, testDebug, "cornercase-gpui", 0, CGRect(x: 0, y: y, width: 1512, height: 912))
    }

    static func ids(_ found: [Shown]?) -> String {
        found.map { "\($0.map { String($0.id) }.joined(separator: ","))" } ?? "nil"
    }

    static func expect<T: Equatable>(_ got: T, _ want: T) -> String? {
        got == want ? nil : "got \(got), wanted \(want)"
    }

    static func atPoint(_ list: [Shown]) -> String {
        ids(inTheWay(at: CGPoint(x: 200, y: 200), of: testDebug, in: list, on: testScreen))
    }

    static func before(_ list: [Shown], at y: CGFloat = 34) -> [String] {
        coversBeforeFront(of: testDebug, frame: debugWindow(at: y).frame, in: list, on: testScreen)
            .map { "\($0.window.id) \($0.part)" }
    }

    static let wide = CGRect(x: 0, y: 0, width: 1512, height: 982)
    static let bar = CGRect(x: 0, y: 0, width: 1512, height: 33)
    static let small = CGRect(x: 100, y: 134, width: 300, height: 200)
    static let near = CGRect(x: 100, y: 100, width: 300, height: 200)

    static func dictionaryTests() -> [(String, () -> String?)] {
        let info: [String: Any] = [
            kCGWindowOwnerPID as String: Int32(7), kCGWindowOwnerName as String: "Dock", kCGWindowLayer as String: 20,
            kCGWindowAlpha as String: 0.5, kCGWindowNumber as String: 9,
            kCGWindowBounds as String: wide.dictionaryRepresentation as NSDictionary,
        ]
        return [
            ("a_window_dictionary_is_read", {
                guard let read = shown(info) else { return "not read" }
                return expect("\(read.id) \(read.pid) \(read.owner) \(read.layer) \(read.alpha) \(read.frame)", "9 7 Dock 20 0.5 \(wide)")
            }),
            ("a_window_dictionary_without_bounds_is_not_read", { expect(shown([kCGWindowOwnerPID as String: Int32(7)]) == nil, true) }),
        ]
    }

    static func coverTests() -> [(String, () -> String?)] {
        let a = made(1, 10, "A", 25, wide)
        let b = made(2, 11, "B", 0, wide)
        let d = debugWindow()
        return [
            ("a_window_in_front_at_the_point_covers_it", { expect(atPoint([b, d]), "2") }),
            ("two_windows_in_front_cover_it_front_to_back", { expect(atPoint([a, b, d]), "1,2") }),
            ("the_docks_full_screen_backdrop_is_passed_over", { expect(atPoint([made(3, 12, "Dock", 20, testDisplay), d]), "") }),
            ("a_dock_window_smaller_than_the_display_covers_it", {
                expect(atPoint([made(3, 12, "Dock", 20, CGRect(x: 0, y: 150, width: 1512, height: 80)), d]), "3")
            }),
            ("a_window_of_alpha_0_is_passed_over", { expect(atPoint([made(3, 12, "X", 24, wide, alpha: 0), d]), "") }),
            ("nothing_covers_the_debug_window_on_top", { expect(atPoint([d, b]), "") }),
            ("a_window_beside_the_point_is_no_cover", { expect(atPoint([made(3, 12, "X", 3, CGRect(x: 600, y: 600, width: 50, height: 50)), d]), "") }),
            ("a_window_of_the_debug_process_is_no_cover", { expect(atPoint([made(3, testDebug, "cornercase-gpui", 101, wide), d]), "") }),
            ("no_window_of_the_process_at_the_point_gives_nil", { expect(atPoint([b]), "nil") }),
        ]
    }

    static func beforeTests() -> [(String, () -> String?)] {
        let d = debugWindow()
        let mac = (1...10).map { made($0, 20, "Control Center", 25, CGRect(x: 1200, y: 0, width: 40, height: 33)) }
            + [made(11, 69680, "Screenshot", 24, wide), made(12, 21, "Window Server", 24, bar),
                made(13, 22, "Dock", 20, testDisplay), d, made(15, 23, "Google Chrome", 0, CGRect(x: 244, y: 57, width: 1024, height: 822))]
        return [
            ("before_the_front_a_higher_layer_over_the_window_covers_it", { expect(before([made(1, 10, "A", 3, small), d]).count, 1) }),
            ("before_the_front_a_window_of_the_debug_process_is_no_cover", {
                expect(before([made(1, testDebug, "cornercase-gpui", 101, wide), d]), [])
            }),
            ("before_the_front_a_bar_touching_the_visible_area_is_no_cover", {
                expect(before([made(1, 10, "A", 24, CGRect(x: 0, y: 0, width: 1512, height: 34)), d], at: 25), [])
            }),
            ("before_the_front_a_window_of_layer_0_is_no_cover_yet", { expect(before([made(1, 10, "A", 0, wide), d]), []) }),
            ("before_the_front_the_menu_bar_and_its_items_are_no_cover", {
                expect(before([made(1, 10, "A", 25, bar), made(2, 21, "Window Server", 24, bar)], at: 25), [])
            }),
            ("before_the_front_the_docks_backdrop_and_alpha_0_are_no_cover", {
                expect(before([made(1, 22, "Dock", 20, testDisplay), made(2, 10, "A", 24, wide, alpha: 0)]), [])
            }),
            ("before_the_front_a_higher_layer_beside_the_window_is_no_cover", {
                expect(before([made(1, 10, "A", 3, CGRect(x: 2000, y: 100, width: 50, height: 50))]), [])
            }),
            ("before_the_front_the_covered_part_is_in_the_windows_points", { expect(before([made(1, 10, "A", 3, small)]), ["1 \(near)"]) }),
            ("this_macs_windows_report_only_the_screenshot_window", { expect(before(mac), ["11 \(CGRect(x: 0, y: 0, width: 1512, height: 912))"]) }),
            ("this_macs_windows_without_the_screenshot_window_report_nothing", { expect(before(mac.filter { $0.id != 11 }), []) }),
        ]
    }

    static func wordTests() -> [(String, () -> String?)] {
        let w = made(11, 69680, "Screenshot", 24, wide)
        let name = "Screenshot (pid 69680, screencaptureui, layer 24)"
        let second = "Notes (pid 5, layer 3)"
        return [
            ("the_name_has_owner_pid_process_and_layer", { expect(named(w, "screencaptureui"), name) }),
            ("the_name_leaves_out_a_process_it_could_not_read", { expect(named(made(1, 5, "Notes", 3, wide), nil), second) }),
            ("the_refusal_names_one_cover", { expect(coverRefusal("200", "100", [name]), "another window covers 200,100: \(name): nothing was posted") }),
            ("the_refusal_names_two_covers_front_to_back", {
                expect(coverRefusal("200", "100", [name, second]), "other windows cover 200,100, front to back: \(name); \(second): nothing was posted")
            }),
            ("the_lines_say_where_each_window_covers_the_window", {
                expect(coverLines([(name, wide), (second, near)]),
                    "\(name) covers 0,0 1512×982 of the window: a press or a scroll there is refused\n"
                    + "\(second) covers 100,100 300×200 of the window: a press or a scroll there is refused")
            }),
        ]
    }

    static func madeLayout(_ name: String, _ rows: [Int: [UInt32: String]]) -> Layout {
        Layout(name: name, types: { code, state in
            let row = rows[code] ?? [:]
            let base = row[state] ?? (state & commandState != 0 ? row[0] : nil) ?? ""
            return Typed(text: base.hasPrefix("~") ? String(base.dropFirst()) : base, dead: base.hasPrefix("~"))
        })
    }

    static let qwerty: [String: Int] = [
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11, "q": 12, "w": 13, "e": 14,
        "r": 15, "y": 16, "t": 17, "o": 31, "u": 32, "i": 34, "p": 35, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46,
    ]

    static let letterRows: [Int: [UInt32: String]] = Dictionary(
        uniqueKeysWithValues: qwerty.map { ($0.value, [0: $0.key, 2: $0.key.uppercased()]) })

    static let digitRows: [Int: [UInt32: String]] = [
        18: [0: "1", 2: "!"], 19: [0: "2", 2: "@"], 20: [0: "3", 2: "#"], 21: [0: "4", 2: "$"], 23: [0: "5", 2: "%"],
        22: [0: "6", 2: "^"], 26: [0: "7", 2: "&"], 28: [0: "8", 2: "*"], 25: [0: "9", 2: "("], 29: [0: "0", 2: ")"],
    ]

    static let us = madeLayout("test.US", letterRows.merging(digitRows) { $1 }.merging([
        27: [0: "-", 2: "_"], 24: [0: "=", 2: "+"], 33: [0: "[", 2: "{"], 30: [0: "]", 2: "}"],
        43: [0: ",", 2: "<"], 44: [0: "/", 2: "?"],
    ]) { $1 })

    static let spanish = madeLayout("test.Spanish-ISO", letterRows.merging(digitRows) { $1 }.merging([
        26: [0: "7", 2: "/", 8: "÷"], 29: [0: "0", 2: "=", 8: "≠"], 27: [0: "'", 2: "?", 8: "´"], 24: [0: "¡", 2: "¿"],
        30: [0: "+", 2: "*", 8: "]", 10: "±"], 33: [0: "~`", 2: "~^", 8: "["], 39: [0: "~´", 2: "~¨", 8: "{"],
        43: [0: ",", 2: ";"], 44: [0: "-", 2: "_"], 75: [0: "/"],
    ]) { $1 })

    static let azerty = madeLayout("test.French", [
        0: [0: "q", 2: "Q"], 12: [0: "a", 2: "A"], 18: [0: "&", 2: "1", 1: "&", 3: "1"],
    ])

    static let dvorakCommand = madeLayout("test.Dvorak-QWERTY", [
        40: [0: "t", 2: "T", 1: "k", 3: "K"], 9: [0: "k", 2: "K", 1: "v", 3: "V"],
    ])

    static let cyrillic = madeLayout("test.Russian", [0: [0: "ф", 2: "Ф", 1: "a", 3: "A"], 8: [0: "с", 2: "С", 1: "c", 3: "C"]])

    static let ruLike = madeLayout("test.Ru-Like", [44: [0: ".", 2: ",", 1: "/", 3: "?"]])

    static let orderLike = madeLayout("test.Order", [20: [0: "a", 2: "x"], 5: [0: "b", 8: "x"]])

    static let plusTwice = madeLayout("test.Plus", [30: [0: "+"], 24: [0: "=", 2: "+"]])

    static let noData = Layout(name: "test.Input-Method", types: nil)
    static let nothing = Layout(name: nil, types: nil)

    static func posted(_ name: String, _ held: [String], _ layout: Layout) -> String {
        do {
            let found = try resolve(name, held, layout: { layout })
            return "\(found.code) \(found.flags.contains(.maskShift) ? "S" : "")\(found.flags.contains(.maskCommand) ? "C" : "")\(found.flags.contains(.maskAlternate) ? "A" : "")\(found.flags.contains(.maskControl) ? "K" : "")"
        } catch let failure as Failure {
            return "\(failure.status): \(failure.message)"
        } catch { return "\(error)" }
    }

    static func refused(_ name: String, _ held: [String], _ layout: Layout) -> String {
        let got = posted(name, held, layout)
        return got.first?.isNumber == true && got.contains(":") ? got : "posted \(got)"
    }

    static func keyTests() -> [(String, () -> String?)] {
        [
            ("every_name_posts_its_key_on_a_us_layout", {
                let want = [("comma", 43, "C"), ("slash", 44, "C"), ("question", 44, "SC"), ("minus", 27, "C"), ("equal", 24, "C"),
                    ("plus", 24, "SC"), ("leftbracket", 33, "C"), ("rightbracket", 30, "C"), ("1", 18, "C"), ("9", 25, "C"), ("0", 29, "C")]
                return expect(want.map { posted($0.0, ["cmd"], us) }, want.map { "\($0.1) \($0.2)" })
            }),
            ("every_name_posts_its_key_or_says_why_on_spanish_iso", {
                let want = [("comma", "43 C"), ("slash", "26 SC"), ("question", "27 SC"), ("minus", "44 C"), ("equal", "29 SC"),
                    ("plus", "30 C"), ("1", "18 C"), ("0", "29 C")]
                let seen = want.map { posted($0.0, ["cmd"], spanish) }
                return expect(seen, want.map { $0.1 })
                    ?? expect(refused("leftbracket", ["cmd"], spanish).prefix(2), "1:")
                    ?? expect(refused("rightbracket", ["cmd"], spanish).prefix(2), "1:")
            }),
            ("a_named_key_is_its_position_on_every_layout", {
                let all = [us, spanish, noData, nothing].map { [posted("pageup", [], $0), posted("home", [], $0), posted("us-quote", [], $0)] }
                return expect(all, Array(repeating: ["116 ", "115 ", "39 "], count: 4))
            }),
            ("a_symbol_needing_shift_reaches_the_window_without_it", {
                guard let types = spanish.types else { return "no data" }
                return expect(windowKey(types, code: 26, command: true, shift: true), Reading(key: "/", shift: false, dead: false))
            }),
            ("a_character_needing_alt_is_refused_with_cmd", {
                expect(refused("leftbracket", ["cmd"], spanish),
                    "1: leftbracket is typed only with alt on test.Spanish-ISO, and with cmd or ctrl held the window reads that key without alt, as cmd-`: this shortcut cannot reach the window on this layout, nothing was posted")
            }),
            ("a_character_needing_alt_is_typed_with_it", { expect(posted("leftbracket", [], spanish), "33 A") }),
            ("a_character_behind_a_dead_key_is_refused", {
                expect(refused("leftbracket", [], madeLayout("test.Dead", [33: [0: "~["]])).contains("dead key"), true)
            }),
            ("a_dead_symbol_under_cmd_is_refused_as_dead", {
                let layout = madeLayout("test.Dead", [33: [0: "~[", 1: "~["]])
                return expect(refused("leftbracket", ["cmd"], layout).contains("dead key"), true)
            }),
            ("a_character_the_layout_lacks_is_refused", { expect(refused("slash", ["cmd"], madeLayout("test.Bare", [0: [0: "a"]])).contains("no key of test.Bare types slash"), true) }),
            ("a_keypad_key_is_never_chosen", { expect(posted("slash", ["cmd"], spanish), "26 SC") }),
            ("fewer_modifiers_win_over_a_lower_code", { expect(posted("plus", ["cmd"], plusTwice), "30 C") }),
            ("a_letter_is_posted_where_the_layout_has_it", { expect(posted("a", ["cmd"], azerty), "12 C") }),
            ("a_digit_that_needs_shift_gets_it_with_cmd", { expect(posted("1", ["cmd"], azerty), "18 SC") }),
            ("with_cmd_the_command_table_decides", { expect([posted("k", ["cmd"], dvorakCommand), posted("k", [], dvorakCommand)], ["40 C", "9 "]) }),
            ("a_layout_of_other_letters_reads_its_command_table", {
                expect(posted("c", ["ctrl"], cyrillic), "8 K") ?? expect(refused("a", [], cyrillic).hasPrefix("1: "), true)
            }),
            ("shift_stays_with_a_letter", { expect(posted("k", ["cmd", "shift"], us), "40 SC") }),
            ("shift_with_a_symbol_without_a_name_is_refused_with_its_reading", {
                expect(refused("leftbracket", ["cmd", "shift"], us),
                    "2: shift with leftbracket is another character: the window reads shift and [ as {, which no name posts and no shortcut takes: nothing was posted")
            }),
            ("shift_with_a_symbol_that_has_a_name_says_it", {
                expect(refused("slash", ["cmd", "shift"], us),
                    "2: shift with slash is another character: the window reads shift and / as ?, so name it: question: nothing was posted")
            }),
            ("shift_with_a_symbol_and_no_cmd_is_refused_without_a_reading", {
                expect(refused("slash", ["shift"], us),
                    "2: shift with slash is another character: the window reads shift and / as the character they type: nothing was posted")
            }),
            ("alt_with_a_character_is_posted_on_the_key_that_types_it_plain", {
                expect([posted("2", ["alt"], spanish), refused("leftbracket", ["alt"], spanish).prefix(2).description], ["19 A", "1:"])
            }),
            ("the_shifted_command_table_decides_when_it_differs", {
                expect([posted("question", ["cmd"], ruLike), posted("slash", ["cmd"], ruLike)], ["44 SC", "44 C"])
            }),
            ("the_typing_search_tries_no_modifier_then_shift_before_alt_and_states_before_codes", {
                expect([posted("plus", [], plusTwice), posted("x", [], orderLike)], ["30 ", "20 S"])
            }),
            ("every_character_name_posts_its_key_on_a_us_layout", {
                var want: [String] = []
                var got: [String] = []
                for (name, code) in qwerty { want.append("\(code) C"); got.append(posted(name, ["cmd"], us)) }
                for (index, name) in digits.enumerated() {
                    want.append("\([29, 18, 19, 20, 21, 23, 22, 26, 28, 25][index]) C")
                    got.append(posted(name, ["cmd"], us))
                }
                return expect(got, want)
            }),
            ("every_named_key_posts_its_position", {
                let want = ["return": 36, "escape": 53, "tab": 48, "space": 49, "delete": 51, "left": 123, "right": 124, "up": 126,
                    "down": 125, "pageup": 116, "pagedown": 121, "home": 115, "end": 119, "us-quote": 39]
                return expect(Dictionary(uniqueKeysWithValues: positionNames.map { ($0, posted($0, [], us)) }),
                    Dictionary(uniqueKeysWithValues: want.map { ($0.key, "\($0.value) ") }))
            }),
            ("an_empty_name_is_unknown_and_posts_nothing", { expect(refused("", ["cmd"], us), "2: unknown key : the keys are \(keyGrammar)") }),
            ("every_refusal_says_its_words", {
                let all = [
                    refused("a", [], nothing), refused("a", [], noData), refused("t", ["cmd"], dvorakCommand),
                    refused("slash", ["cmd"], madeLayout("test.Bare", [0: [0: "a"]])),
                    refused("slash", [], madeLayout("test.Bare", [0: [0: "a"]])),
                    refused("leftbracket", ["cmd"], madeLayout("test.Dead", [33: [0: "~["]])),
                    refused("leftbracket", [], madeLayout("test.Dead", [33: [0: "~["]])),
                ]
                return expect(all, [
                    "1: no keyboard layout is in use, so no key can be found for a: named keys still work, nothing was posted",
                    "1: the keyboard layout in use (test.Input-Method) has no key data, as an input method's may not, so no key can be found for a: named keys still work, nothing was posted",
                    "1: no key of test.Dvorak-QWERTY makes the window read t with these modifiers: nothing was posted",
                    "1: no key of test.Bare types slash: this shortcut cannot reach the window on this layout, nothing was posted",
                    "1: no key of test.Bare types slash: `text` types it, nothing was posted",
                    "1: leftbracket is behind a dead key on test.Dead: this shortcut cannot reach the window on this layout, nothing was posted",
                    "1: leftbracket is behind a dead key on test.Dead: `text` types it, nothing was posted",
                ])
            }),
            ("hold_says_it_takes_no_modifiers", {
                expect(holdRefusal("cmd").message, "hold takes no modifiers (cmd): hold PID KEY [MS]; key posts a key with modifiers")
            }),
            ("alt_with_cmd_is_passed_on", { expect(posted("plus", ["cmd", "alt"], spanish), "30 CA") }),
            ("a_layout_without_key_data_refuses_a_character", {
                let got = refused("slash", ["cmd"], noData)
                return expect(got.hasPrefix("1: ") && got.contains("has no key data"), true) ?? expect(posted("pagedown", [], noData), "121 ")
            }),
            ("no_layout_in_use_is_refused_with_a_sentence", { expect(refused("a", [], nothing).hasPrefix("1: no keyboard layout is in use"), true) }),
            ("an_unknown_modifier_lists_the_modifiers", {
                expect(posted("a", ["command"], us), "2: unknown modifier command: the modifiers are cmd, shift, alt and ctrl")
            }),
            ("an_unknown_key_posts_nothing_and_lists_every_key", {
                expect(refused("quote", [], us), "2: unknown key quote: quote is now us-quote, the key where a US keyboard has ' (a dead key on Spanish ISO); the keys are \(keyGrammar)")
            }),
            ("a_single_character_is_told_its_name", {
                expect(refused("/", ["cmd"], us), "2: unknown key /: its name is slash; the keys are \(keyGrammar)")
            }),
            ("the_order_lists_name_every_key_once", {
                let listed = letters + digits + symbolNames + positionNames
                return expect(Set(listed).count, listed.count) ?? expect(Set(listed), Set(namedKeys.keys).union(characterNames.keys))
            }),
            ("the_usage_line_holds_every_key", { expect(usage.contains(keyGrammar) && usage.contains("hold: KEY [MS]"), true) }),
            ("the_layout_in_use_gives_a_key_or_says_why", {
                let got = posted("slash", ["cmd"], layoutInUse())
                return expect(got.hasPrefix("1: ") || !got.contains(":"), true)
            }),
        ]
    }

    static func run() throws {
        let cases = coverTests() + dictionaryTests() + beforeTests() + wordTests() + keyTests()
        var failed = 0
        for (name, body) in cases {
            if let why = body() {
                failed += 1
                print("not ok - \(name): \(why)")
            } else {
                print("ok - \(name)")
            }
        }
        print("\(cases.count) tests, \(failed) failed")
        guard failed == 0 else { throw Failure(message: "self-test failed", status: 1) }
    }
}

func run(_ args: [String]) throws {
    if args == ["self-test"] { return try SelfTest.run() }
    guard args.count >= 2, let pid = pid_t(args[1]) else { throw Failure(message: usage, status: 2) }
    if ["front", "size", "key", "hold", "text", "paste", "press", "drag", "scroll"].contains(args[0]) { try checkedPid(pid) }
    switch (args[0], args.count) {
    case ("window", 2): try describe(pid)
    case ("covers", 2): try covers(pid)
    case ("front", 2): try front(pid)
    case ("size", 4): try size(pid, args[2], args[3])
    case ("key", 3...): try key(pid, args[2], Array(args.dropFirst(3)))
    case ("hold", 3...): try hold(pid, args[2], Array(args.dropFirst(3)))
    case ("text", 3): try text(pid, args[2])
    case ("paste", 3): try paste(pid, args[2])
    case ("press", 4...5): try press(pid, args[2], args[3], right: try button(args, after: 4))
    case ("drag", 6...9):
        let (right, hold) = try dragOptions(Array(args.dropFirst(6)))
        try drag(pid, from: (args[2], args[3]), to: (args[4], args[5]), right: right, hold: hold)
    case ("scroll", 5...6):
        let sideways = args.count == 6 ? try wholeNumber(args[5], "dx") : 0
        try scroll(pid, args[2], args[3], dy: try wholeNumber(args[4], "dy"), dx: sideways)
    default: throw Failure(message: usage, status: 2)
    }
}

do {
    try run(Array(CommandLine.arguments.dropFirst()))
} catch let failure as Failure {
    FileHandle.standardError.write(Data((failure.message + "\n").utf8))
    exit(failure.status)
}
