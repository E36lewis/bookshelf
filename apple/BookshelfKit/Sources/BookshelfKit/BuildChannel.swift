import Foundation
import BookshelfFFI

/// Which build this is. Only a release build is Stable: it's "Bookshelf"
/// and opens the real journal. Every other build, dev and CI included, is
/// "Bookshelf Preview", with its own bundle ID and journal, so trying one
/// out never touches real books. apple/scripts/build-dmg.sh sets
/// `BOOKSHELF_CHANNEL=Stable` for a release, which project.yml puts in
/// Info.plist as `BookshelfChannel`.
public enum BuildChannel {
    /// The channel an Info.plist value names: anything but "Stable" is a preview.
    public static func channel(infoValue: Any?) -> Channel {
        (infoValue as? String) == "Stable" ? .stable : .preview
    }

    /// This app's channel, from its Info.plist.
    public static let current = channel(infoValue: Bundle.main.object(forInfoDictionaryKey: "BookshelfChannel"))

    /// "Bookshelf" or "Bookshelf Preview": the main window's name, and About's.
    public static func appName(_ channel: Channel) -> String {
        channel == .stable ? "Bookshelf" : "Bookshelf Preview"
    }
}
