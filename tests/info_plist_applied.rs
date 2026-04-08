use objc2_core_foundation::CFBundle;

embed_plist::embed_info_plist_bytes!(br#"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>English</string>
  <key>CFBundleDisplayName</key>
  <string>cargo-simctl-runner</string>
  <key>CFBundleExecutable</key>
  <string>cargo-simctl-runner</string>
  <key>CFBundleIdentifier</key>
  <string>cargo-simctl-runner.cargo-simctl-runner</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>cargo-simctl-runner</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>20260325.080054</string>
  <key>CSResourcesFileMapped</key>
  <true/>
  <key>LSRequiresCarbon</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
"#);

#[test]
fn contains_plist() {
    let bundle = CFBundle::main_bundle().unwrap();
    panic!("{:?}", bundle.info_dictionary())
}
