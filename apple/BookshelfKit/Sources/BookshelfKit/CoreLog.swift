import Foundation
import os
import BookshelfFFI

/// Sends the core's log messages to the unified log (Console.app), under
/// the app's subsystem and the "core" category. The core never logs what
/// anyone wrote, so its messages are logged as public.
public final class CoreLog: BookshelfFFI.Logger {
    private let osLog: os.Logger

    /// Logs under `subsystem`, usually the app's bundle id.
    public init(subsystem: String) {
        osLog = os.Logger(subsystem: subsystem, category: "core")
    }

    /// One message from the core.
    public func log(level: LogLevel, target: String, message: String) {
        switch level {
        case .error:
            osLog.error("\(target, privacy: .public): \(message, privacy: .public)")
        case .warn:
            osLog.warning("\(target, privacy: .public): \(message, privacy: .public)")
        case .info:
            osLog.info("\(target, privacy: .public): \(message, privacy: .public)")
        }
    }

    /// Routes the core's messages to the unified log from now on.
    public static func start(subsystem: String) {
        setLogger(logger: CoreLog(subsystem: subsystem))
    }
}
