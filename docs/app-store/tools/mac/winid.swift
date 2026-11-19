// winid <pid>: the id of that process's largest on-screen normal window.
import CoreGraphics
let pid = Int32(CommandLine.arguments[1])!
var best: (id: Int, area: Double)? = nil
for w in (CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]) {
    guard (w[kCGWindowOwnerPID as String] as? Int32) == pid,
          (w[kCGWindowLayer as String] as? Int) == 0,
          let b = w[kCGWindowBounds as String] as? [String: Any],
          let wd = b["Width"] as? Double, let h = b["Height"] as? Double,
          let id = w[kCGWindowNumber as String] as? Int
    else { continue }
    if best == nil || wd * h > best!.area { best = (id, wd * h) }
}
if let best { print(best.id) }
