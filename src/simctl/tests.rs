
use super::*;

/// Test a configuration for Xcode 9.2 with the 8.1 iOS runtime added.
///
/// (Some items filtered for brevity).
#[test]
fn xcode_9() {
    let info: SimulatorInfo = serde_json::from_str(include_str!("xcode_9.json")).unwrap();
    let expected = [
        (Availability::Available, "iOS"),
        (Availability::Available, "iOS"),
        (Availability::Available, "tvOS"),
        (Availability::Available, "watchOS"),
    ];
    assert_eq!(info.runtimes.len(), expected.len());

    for (runtime, expected) in info.runtimes.into_iter().zip(expected) {
        let (availability, platform_name) = expected;
        assert_eq!(runtime.availability, availability);
        assert!(runtime.is_platform(platform_name));
        assert!(!runtime.is_platform("visionOS"));
    }
}

/// Test a configuration for Xcode 26.3 with the 15.0 iOS runtime added.
///
/// (Some items filtered for brevity).
#[test]
fn xcode_26() {
    let info: SimulatorInfo = serde_json::from_str(include_str!("xcode_26.json")).unwrap();
    let expected = [
        (
            Availability::Unavailable(
                "The iOS 15.0 simulator runtime is not supported on hosts after macOS 14.99.0."
                    .into(),
            ),
            "iOS",
        ),
        (Availability::Available, "iOS"),
        (Availability::Available, "tvOS"),
        (Availability::Available, "watchOS"),
        (Availability::Available, "xrOS"),
    ];
    assert_eq!(info.runtimes.len(), expected.len());

    for (runtime, expected) in info.runtimes.into_iter().zip(expected) {
        let (availability, platform_name) = expected;
        assert_eq!(runtime.availability, availability);
        assert!(runtime.is_platform(platform_name));
        assert!(!runtime.is_platform("visionOS"));
    }
}
