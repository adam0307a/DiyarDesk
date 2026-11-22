import Cocoa
import CoreGraphics

final class DesktopIconsReader {
    struct ScreenObservation {
        let iconRects: [[Int]]
        let windowRects: [[Int]]
        let occluded: Bool
    }

    func observe(screen: NSScreen, windowAware: Bool) -> ScreenObservation {
        let scale = Double(screen.backingScaleFactor)
        let frame = screen.frame
        let visible = screen.visibleFrame
        let screenW = Int(round(Double(frame.width) * scale))
        let screenH = Int(round(Double(frame.height) * scale))

        // Read ~/Desktop visible items and place them along macOS Finder's default top-right columns
        let desktopURL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Desktop")
        let items = (try? FileManager.default.contentsOfDirectory(atPath: desktopURL.path))?
            .filter { !$0.hasPrefix(".") }
            .sorted() ?? []

        var iconRects: [[Int]] = []
        let cellW = Int(round(92.0 * scale))
        let cellH = Int(round(98.0 * scale))
        let marginRight = Int(round(24.0 * scale))
        let topMenuBar = Int(round(Double(frame.maxY - visible.maxY) * scale)) + Int(round(22.0 * scale))
        let dockBottom = Int(round(Double(visible.minY - frame.minY) * scale)) + Int(round(20.0 * scale))
        let usableH = max(cellH, screenH - topMenuBar - dockBottom)
        let maxRows = max(1, usableH / cellH)

        for (idx, _) in items.prefix(64).enumerated() {
            let col = idx / maxRows
            let row = idx % maxRows
            let right = screenW - marginRight - col * cellW
            let left = right - Int(round(76.0 * scale))
            let top = topMenuBar + row * cellH
            let bottom = top + Int(round(84.0 * scale))
            if left >= 0 && bottom <= screenH {
                iconRects.append([left, top, right, bottom])
            }
        }

        // Inspect visible on-screen normal application windows via CoreGraphics
        var winRects: [[Int]] = []
        var occluded = false
        let mainMaxY = NSScreen.screens.first?.frame.maxY ?? frame.maxY
        // Convert Cocoa screen frame to CoreGraphics top-left global coordinates
        let screenCgX = Double(frame.minX)
        let screenCgY = Double(mainMaxY - frame.maxY)
        let screenArea = max(1.0, Double(frame.width * frame.height))
        let myPid = ProcessInfo.processInfo.processIdentifier

        if let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] {
            for info in list {
                let layer = (info[kCGWindowLayer as String] as? Int) ?? 0
                if layer != 0 { continue }
                let pid = (info[kCGWindowOwnerPID as String] as? Int32) ?? 0
                if pid == myPid { continue }
                let alpha = (info[kCGWindowAlpha as String] as? Double) ?? 1.0
                if alpha < 0.05 { continue }
                guard let boundsDict = info[kCGWindowBounds as String] as? [String: Any],
                      let bx = boundsDict["X"] as? Double,
                      let by = boundsDict["Y"] as? Double,
                      let bw = boundsDict["Width"] as? Double,
                      let bh = boundsDict["Height"] as? Double else { continue }
                if bw < 64 || bh < 64 { continue }

                // Intersect with this monitor
                let ix1 = max(bx, screenCgX)
                let iy1 = max(by, screenCgY)
                let ix2 = min(bx + bw, screenCgX + Double(frame.width))
                let iy2 = min(by + bh, screenCgY + Double(frame.height))
                if ix2 <= ix1 || iy2 <= iy1 { continue }

                let coverRatio = ((ix2 - ix1) * (iy2 - iy1)) / screenArea
                if coverRatio >= 0.88 {
                    occluded = true
                }
                if windowAware {
                    let wid = (info[kCGWindowNumber as String] as? Int) ?? winRects.count + 1
                    let l = Int(round((ix1 - screenCgX) * scale))
                    let t = Int(round((iy1 - screenCgY) * scale))
                    let r = Int(round((ix2 - screenCgX) * scale))
                    let b = Int(round((iy2 - screenCgY) * scale))
                    winRects.append([wid, l, t, r, b])
                }
            }
        }

        if windowAware && iconRects.isEmpty && !winRects.isEmpty {
            iconRects = winRects.map { [$0[1], $0[2], $0[3], $0[4]] }
        }

        return ScreenObservation(iconRects: iconRects, windowRects: winRects, occluded: occluded)
    }
}
