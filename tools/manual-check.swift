import AppKit
import ApplicationServices
import Carbon.HIToolbox

struct Failure: Error {
    let message: String
    let status: Int32
}

let usage = "usage: manual-check window|covers|front|size|key|hold|text|paste|press|drag|scroll PID … | self-test (hold: KEY [MS]; paste: TEXT; drag: x0 y0 x1 y1 [right] [hold MS])"

let keys: [String: Int] = [
    "a": kVK_ANSI_A, "b": kVK_ANSI_B, "c": kVK_ANSI_C, "d": kVK_ANSI_D, "e": kVK_ANSI_E, "f": kVK_ANSI_F,
    "g": kVK_ANSI_G, "h": kVK_ANSI_H, "i": kVK_ANSI_I, "j": kVK_ANSI_J, "k": kVK_ANSI_K, "l": kVK_ANSI_L,
    "m": kVK_ANSI_M, "n": kVK_ANSI_N, "o": kVK_ANSI_O, "p": kVK_ANSI_P, "q": kVK_ANSI_Q, "r": kVK_ANSI_R,
    "s": kVK_ANSI_S, "t": kVK_ANSI_T, "u": kVK_ANSI_U, "v": kVK_ANSI_V, "w": kVK_ANSI_W, "x": kVK_ANSI_X,
    "y": kVK_ANSI_Y, "z": kVK_ANSI_Z, "return": kVK_Return, "escape": kVK_Escape, "tab": kVK_Tab,
    "space": kVK_Space, "delete": kVK_Delete, "left": kVK_LeftArrow, "right": kVK_RightArrow,
    "up": kVK_UpArrow, "down": kVK_DownArrow, "quote": kVK_ANSI_Quote,
]

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

func flags(_ held: [String]) throws -> CGEventFlags {
    var flags = CGEventFlags()
    for word in held {
        guard let flag = modifiers[word] else { throw Failure(message: "unknown modifier \(word)", status: 2) }
        flags.insert(flag)
    }
    return flags
}

func key(_ pid: pid_t, _ name: String, _ held: [String]) throws {
    guard let code = keys[name] else { throw Failure(message: "unknown key \(name)", status: 2) }
    let flags = try flags(held)
    try allowed()
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(code), keyDown: down)
        event?.flags = flags
        event?.postToPid(pid)
        usleep(30_000)
    }
}

func hold(_ pid: pid_t, _ name: String, _ duration: String?) throws {
    guard let code = keys[name] else { throw Failure(message: "unknown key \(name)", status: 2) }
    let ms = try duration.map { Int(try wholeNumber($0, "ms")) } ?? 800
    guard (0...10_000).contains(ms) else { throw Failure(message: "ms must be 0 to 10000", status: 2) }
    try allowed()
    func post(_ down: Bool, repeating: Bool) {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(code), keyDown: down)
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
    let board = NSPasteboard.general
    let kept = saved(board)
    let held = (board.pasteboardItems ?? []).map { $0.types.count }
    guard kept.map({ $0.count }) == held else {
        throw Failure(message: "the clipboard holds data that cannot be read back: nothing was done", status: 1)
    }
    board.clearContents()
    board.setString(typed, forType: .string)
    board.setData(Data(), forType: NSPasteboard.PasteboardType("org.nspasteboard.TransientType"))
    try key(pid, "v", ["cmd"])
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

    static func run() throws {
        let cases = coverTests() + dictionaryTests() + beforeTests() + wordTests()
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
    case ("hold", 3...4): try hold(pid, args[2], args.count == 4 ? args[3] : nil)
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
