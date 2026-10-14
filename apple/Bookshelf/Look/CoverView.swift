import AppKit
import ImageIO
import SwiftUI

/// Book covers, decoded off the main thread at the size they're shown and
/// kept in memory. Covers never change once saved, so the file path and
/// size are the whole key.
final class CoverCache: @unchecked Sendable {
    static let shared = CoverCache()

    // NSCache is thread-safe; it lets go of covers under memory pressure.
    private let cache = NSCache<NSString, CGImage>()

    private init() {
        cache.countLimit = 400
    }

    /// The cover at `path`, at most `pixels` on its longer side.
    func thumbnail(at path: String, pixels: Int) async -> CGImage? {
        let key = "\(pixels)|\(path)" as NSString
        if let hit = cache.object(forKey: key) { return hit }
        let image = await Task.detached(priority: .utility) {
            Self.decode(path, pixels: pixels)
        }.value
        if let image { cache.setObject(image, forKey: key) }
        return image
    }

    /// A search result's cover from its image file's bytes, at most
    /// `pixels` on its longer side, decoded off the main thread. Not kept:
    /// the core keeps the bytes.
    func thumbnail(of data: Data, pixels: Int) async -> CGImage? {
        await Task.detached(priority: .utility) { () -> CGImage? in
            guard let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary)
            else { return nil }
            return Self.thumbnail(of: source, pixels: pixels)
        }.value
    }

    /// ImageIO makes a thumbnail straight from the file, without decoding
    /// the full-size image first.
    private static func decode(_ path: String, pixels: Int) -> CGImage? {
        let url = URL(fileURLWithPath: path)
        guard let source = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary)
        else { return nil }
        return thumbnail(of: source, pixels: pixels)
    }

    private static func thumbnail(of source: CGImageSource, pixels: Int) -> CGImage? {
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: max(pixels, 16),
        ]
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }
}

/// A book's cover, or a quiet made-up one when there's no image. Covers
/// are decoration: VoiceOver skips them (the title is beside them).
struct CoverView: View {
    let path: String?
    let title: String
    var author: String?
    let width: CGFloat

    @Environment(\.displayScale) private var scale
    @State private var image: CGImage?

    private var height: CGFloat { (width * 1.5).rounded() }

    var body: some View {
        CoverFrame(image: image, title: title, author: author, width: width)
            .task(id: path) {
                guard let path else {
                    image = nil
                    return
                }
                image = await CoverCache.shared.thumbnail(at: path, pixels: Int(height * scale))
            }
    }
}

/// A search result's cover, fetched through the core as the row appears,
/// a few at a time; the made-up cover until then, and for good when Open
/// Library has none. Decoration, like `CoverView`.
struct SearchCoverView: View {
    let coverId: Int64?
    let title: String
    let width: CGFloat

    @Environment(AppModel.self) private var model
    @Environment(\.displayScale) private var scale
    @State private var image: CGImage?

    var body: some View {
        CoverFrame(image: image, title: title, author: nil, width: width)
            .task(id: coverId) {
                image = nil
                guard let coverId else { return }
                let pixels = Int((width * 1.5 * scale).rounded())
                let model = model
                guard let data = await SearchCoverLoads.shared.run({ await model.searchCover(coverId) }),
                    !Task.isCancelled
                else { return }
                image = await CoverCache.shared.thumbnail(of: data, pixels: pixels)
            }
    }
}

/// Keeps search-result covers to a few loading at a time. A row that's
/// gone by its turn (a newer search replaced it) loads nothing.
actor SearchCoverLoads {
    static let shared = SearchCoverLoads()
    private let limit = 3
    private var running = 0
    private var waiting: [CheckedContinuation<Void, Never>] = []

    func run<T: Sendable>(_ load: @Sendable () async -> T?) async -> T? {
        if running < limit {
            running += 1
        } else {
            await withCheckedContinuation { waiting.append($0) }  // handed a turn
        }
        defer {
            if waiting.isEmpty { running -= 1 } else { waiting.removeFirst().resume() }
        }
        guard !Task.isCancelled else { return nil }
        return await load()
    }
}

/// A cover's look: the image (or a made-up cover), a hint of a spine,
/// rounded corners and a shadow. Hidden from VoiceOver.
private struct CoverFrame: View {
    let image: CGImage?
    let title: String
    let author: String?
    let width: CGFloat

    private var height: CGFloat { (width * 1.5).rounded() }

    var body: some View {
        ZStack(alignment: .leading) {
            if let image {
                Image(decorative: image, scale: 1)
                    .resizable()
                    .aspectRatio(contentMode: .fill)
                    .frame(width: width, height: height)
            } else {
                CoverPlaceholder(title: title, author: author, width: width)
            }
            // A hint of a spine on the left.
            Rectangle().fill(.white.opacity(0.18)).frame(width: max(1, width * 0.025))
        }
        .frame(width: width, height: height)
        .clipShape(UnevenRoundedRectangle(
            topLeadingRadius: width * 0.03, bottomLeadingRadius: width * 0.03,
            bottomTrailingRadius: width * 0.08, topTrailingRadius: width * 0.08))
        .shadow(color: .black.opacity(0.22), radius: width * 0.06, y: width * 0.03)
        .accessibilityHidden(true)
    }
}

/// A plain cover for books without one: a muted color picked from the
/// title, with the title set in serif.
struct CoverPlaceholder: View {
    let title: String
    var author: String?
    let width: CGFloat

    private static let colors: [(Color, Color)] = [
        (Color(red: 0.42, green: 0.51, blue: 0.45), Color(red: 0.31, green: 0.40, blue: 0.35)),  // sage
        (Color(red: 0.62, green: 0.42, blue: 0.35), Color(red: 0.50, green: 0.32, blue: 0.27)),  // clay
        (Color(red: 0.36, green: 0.43, blue: 0.56), Color(red: 0.26, green: 0.32, blue: 0.45)),  // slate
        (Color(red: 0.53, green: 0.40, blue: 0.53), Color(red: 0.41, green: 0.30, blue: 0.42)),  // plum
        (Color(red: 0.66, green: 0.55, blue: 0.33), Color(red: 0.53, green: 0.43, blue: 0.24)),  // ochre
        (Color(red: 0.30, green: 0.50, blue: 0.53), Color(red: 0.21, green: 0.39, blue: 0.42)),  // teal
    ]

    /// The same color for the same title, every launch (`hashValue` isn't).
    private var palette: (Color, Color) {
        let sum = title.unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) & 0xFFFF }
        return Self.colors[sum % Self.colors.count]
    }

    var body: some View {
        let (top, bottom) = palette
        ZStack {
            LinearGradient(colors: [top, bottom], startPoint: .top, endPoint: .bottom)
            if width >= 80 {
                VStack(spacing: width * 0.06) {
                    Text(title)
                        .font(.custom(Typeface.serifFamily, size: width * 0.11).weight(.bold))
                        .multilineTextAlignment(.center)
                        .lineLimit(4)
                        .minimumScaleFactor(0.7)
                    Rectangle().frame(width: width * 0.2, height: 1).opacity(0.6)
                    if let author {
                        Text(author)
                            .font(.custom(Typeface.serifFamily, size: width * 0.075))
                            .lineLimit(2)
                            .multilineTextAlignment(.center)
                            .opacity(0.85)
                    }
                }
                .foregroundStyle(.white)
                .padding(width * 0.1)
            } else {
                Text(String(title.prefix(1)))
                    .font(.custom(Typeface.serifFamily, size: width * 0.45).weight(.bold))
                    .foregroundStyle(.white.opacity(0.9))
            }
        }
    }
}
