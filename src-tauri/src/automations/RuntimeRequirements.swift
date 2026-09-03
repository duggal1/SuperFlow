import Foundation

public enum RuntimeRequirements {
    public static let infoPlistKeys = [
        "NSRemindersFullAccessUsageDescription",
        "NSCalendarsFullAccessUsageDescription",
        "NSAppleEventsUsageDescription"
    ]

    public static func validate(bundle: Bundle = .main) throws {
        for key in infoPlistKeys {
            guard bundle.object(forInfoDictionaryKey: key) != nil else {
                throw AutomationError.missingRuntimeRequirement(key)
            }
        }
    }
}
