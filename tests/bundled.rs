use objc2_core_foundation::{CFBundle, CFString};

/// Test that the main bundle of the binary has an Info.plist.
#[test]
fn has_plist() {
    let bundle = CFBundle::main_bundle().unwrap();
    if let Some(identifier) = bundle.identifier() {
        assert!(identifier.has_prefix(Some(&CFString::from_static_str(
            "cargo-apple-runner.bundled-"
        ))));
    } else {
        panic!("must have a bundle identifier")
    }
}

/// Test that AppKit thinks we're bundled.
#[test]
#[cfg(target_os = "macos")]
fn appkit_bundled() {
    assert!(
        objc2_app_kit::NSRunningApplication::currentApplication()
            .bundleIdentifier()
            .is_some()
    );
}
