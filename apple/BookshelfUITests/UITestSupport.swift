import AppKit
import XCTest

/// Light or dark, whatever the profile's settings say.
enum Look: String {
    case light, dark
}

/// Which journal the app opens: an empty one, or the demo journal. Both
/// live in a temporary folder inside the app's sandbox.
enum Journal {
    case fresh, demo
}

extension XCTestCase {
    /// Launches the app on a journal of its own, in a fixed-size window.
    @MainActor
    func launch(_ journal: Journal, _ look: Look, _ extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = [
            journal == .demo ? "-BookshelfDemoJournal" : "-BookshelfFreshJournal",
            "-BookshelfAppearance", look.rawValue,
            "-BookshelfWindowSize", "1000x700",
        ] + extra
        app.launch()
        if !app.windows.firstMatch.waitForExistence(timeout: 8) {
            // Shouldn't happen (the app opens its own window); noted so it's seen.
            let note = XCTAttachment(string: "No window after launch: opened it from the Window menu.")
            note.name = "no-window-at-launch"
            note.lifetime = .keepAlways
            add(note)
            app.menuBars.menuBarItems["Window"].click()
            app.menuItems["Bookshelf Preview"].firstMatch.click() // tests run preview builds
        }
        // If a test can't find what it waits for, this shows what was there.
        sleep(1)
        let shot = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        shot.name = "00-launched-\(name)-\(look.rawValue)"
        shot.lifetime = .deleteOnSuccess
        add(shot)
        return app
    }

    /// A book in the list, by title.
    @MainActor
    func entry(_ app: XCUIApplication, _ title: String) -> XCUIElement {
        app.descendants(matching: .any)["entry.\(title)"].firstMatch
    }

    /// Keeps a screenshot of the app's front window.
    @MainActor
    func keep(_ app: XCUIApplication, _ name: String, _ look: Look) {
        keep(app.windows.firstMatch, name, look)
    }

    @MainActor
    func keep(_ element: XCUIElement, _ name: String, _ look: Look) {
        let shot = XCTAttachment(screenshot: element.screenshot())
        shot.name = "\(name)-\(look.rawValue)"
        shot.lifetime = .keepAlways
        add(shot)
    }

    /// Keeps the whole screen: for menus, which aren't in a window, and
    /// full screen.
    @MainActor
    func keepScreen(_ name: String, _ look: Look) {
        let shot = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        shot.name = "\(name)-\(look.rawValue)"
        shot.lifetime = .keepAlways
        add(shot)
    }

    /// Keeps a note with the test's results.
    func note(_ text: String, named name: String) {
        let attachment = XCTAttachment(string: text)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// The contrast of `element`'s text on its background, measured from a
    /// screenshot of it: the background is its commonest color, the text
    /// the pixel that differs most from that. Anti-aliasing only ever
    /// lightens strokes, so this is the least the text has. `nil` if there
    /// was nothing to measure.
    @MainActor
    func measuredContrast(_ element: XCUIElement) -> Double? {
        let image = element.screenshot().image
        guard let picture = image.cgImage(forProposedRect: nil, context: nil, hints: nil),
              let sRGB = CGColorSpace(name: CGColorSpace.sRGB)
        else { return nil }
        let width = picture.width, height = picture.height
        guard width > 0, height > 0 else { return nil }
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        let drawn = pixels.withUnsafeMutableBytes { buffer -> Bool in
            guard let context = CGContext(
                data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8,
                bytesPerRow: width * 4, space: sRGB, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
            else { return false }
            context.draw(picture, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard drawn else { return nil }
        var counts: [UInt32: Int] = [:]
        for i in stride(from: 0, to: pixels.count, by: 4) {
            let rgb = UInt32(pixels[i]) << 16 | UInt32(pixels[i + 1]) << 8 | UInt32(pixels[i + 2])
            counts[rgb, default: 0] += 1
        }
        guard let background = counts.max(by: { $0.value < $1.value })?.key else { return nil }
        // WCAG 2 relative luminance and contrast ratio.
        func luminance(_ rgb: UInt32) -> Double {
            func channel(_ shift: UInt32) -> Double {
                let c = Double((rgb >> shift) & 0xFF) / 255
                return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4)
            }
            return 0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
        }
        let back = luminance(background)
        return counts.keys.map { rgb in
            let l = luminance(rgb)
            return (max(l, back) + 0.05) / (min(l, back) + 0.05)
        }.max()
    }

    /// Waits until `element`'s value or label satisfies `test`.
    @MainActor
    @discardableResult
    func waitFor(_ element: XCUIElement, timeout: TimeInterval = 5, _ test: (String) -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            if element.exists, test(element.value as? String ?? "") || test(element.label) { return true }
            usleep(200_000)
        } while Date() < deadline
        return false
    }
}
