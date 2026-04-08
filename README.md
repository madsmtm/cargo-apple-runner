# Cargo [runner](https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner) for Apple targets

[![Latest version](https://badgen.net/crates/v/cargo-apple-runner)](https://crates.io/crates/cargo-apple-runner)
[![License](https://badgen.net/badge/license/Zlib%20OR%20Apache-2.0%20OR%20MIT/blue)](./README.md#license)
[![Documentation](https://docs.rs/cargo-apple-runner/badge.svg)](https://docs.rs/cargo-apple-runner/)
[![CI](https://github.com/madsmtm/cargo-apple-runner/actions/workflows/ci.yml/badge.svg)](https://github.com/madsmtm/cargo-apple-runner/actions/workflows/ci.yml)

Easily bundle, sign and launch binaries on Apple targets.


## Usage

Install with:
```sh
cargo install cargo-apple-runner
```

And add the following to your project's `.cargo/config.toml`:

```toml
[target.'cfg(target_vendor = "apple")']
runner = "cargo-apple-runner"
```

Now you can test and run your programs on the simulator with:
```sh
cargo test --target aarch64-apple-ios-sim --target aarch64-apple-visionos-sim
cargo run --target aarch64-apple-ios-sim
# etc.
```


## Bundling

`cargo-apple-runner` will inspect your binary, and determine if it needs to bundle it based on a few factors:
- TODO.




## Usage in CI

Example GitHub Actions workflow that runs tests on macOS, Mac Catalyst, iOS Simulator and tvOS Simulator.

```yaml
# ...

jobs:
  test:
    runs-on: macos-latest

    # Configure the job to use `cargo-apple-runner` when launching our binaries.
    # (Alternatively, you could commit the `.cargo/config.toml` above).
    env:
      CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER: cargo-apple-runner
      CARGO_TARGET_AARCH64_APPLE_IOS_MACABI_RUNNER: cargo-apple-runner
      CARGO_TARGET_AARCH64_APPLE_IOS_SIM_RUNNER: cargo-apple-runner
      CARGO_TARGET_AARCH64_APPLE_TVOS_SIM_RUNNER: cargo-apple-runner
      CARGO_TARGET_AARCH64_APPLE_VISIONOS_SIM_RUNNER: cargo-apple-runner

    steps:
    - uses: taiki-e/checkout-action@v1

    - name: Install Rustup targets
      run: rustup target add aarch64-apple-ios-macabi aarch64-apple-ios-sim aarch64-apple-tvos-sim

    - name: Install `cargo-apple-runner`
      uses: taiki-e/install-action@cargo-apple-runner

    - uses: Swatinem/rust-cache@v2

    # You can find names of existing simulator devices at:
    # https://github.com/actions/runner-images/blob/main/images/macos/macos-26-arm64-Readme.md#installed-simulators
    - name: Start iOS simulator
      run: xcrun simctl boot "iPhone 17"
    - name: Start tvOS simulator
      run: xcrun simctl boot "Apple TV"
    - name: Start visionOS simulator
      run: xcrun simctl boot "Apple Vision Pro"

    - name: Run tests on host macOS
      run: cargo test
    - name: Run Mac Catalyst tests
      run: cargo test --target aarch64-apple-ios-macabi
    - name: Run iOS simulator tests
      run: cargo test --target aarch64-apple-ios-sim
    - name: Run tvOS simulator tests
      run: cargo test --target aarch64-apple-tvos-sim
    - name: Run visionOS simulator tests
      run: cargo test --target aarch64-apple-visionos-sim
```


## What do I want?

```toml
[target.'cfg(target_vendor = "apple")']
runner = "cargo-apple-runner"
# Optional optimization when using entitlements, there signing is done by the runner.
rustflags = ["-Clink-arg=-Wl,-no_adhoc_codesign"]

# TODO: Running on device requires doing the steps in
# https://github.com/sonos/dinghy/blob/main/docs/ios.md#additional-requirements
```


## Supported platforms

Host: macOS 10.12, [same as `rustc`](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html#os-version).
OS: macOS, Mac Catalyst, iOS, tvOS, watchOS and visionOS.
Simulators: Xcode 9.2 and above.
Devices: Yet unsupported.


## Env vars

Easily pass onwards to runner? Maybe extract from Cargo's `[env]` table, along with a few standard `CARGO_*` env vars?

Host `DYLD_FALLBACK_LIBRARY_PATH` env var needs to copy the directory probably? To make dynamic loading work properly.
- Probably also needs to be patched (`install_name_tool`) and codesigned.

https://doc.rust-lang.org/cargo/reference/environment-variables.html#dynamic-library-paths


## Implementation

Init:
- Commandline args?
- Configuration file?
- Inspect binary
  - Supported platform (`otool -l`) (actually platformS, see `lipo`)

Create bundle:
- Create `.app` folder structure
- Add `Info.plist` and assets from above
- Create entitlements and DER entitlements
<!-- - Patch and copy the binary:
  - Remove codesigning (it's gonna break with the below)
  - Insert desired `__entitlements`/`__ents_der` sections. -->
- Sign `.app`.

Prepare for run:
- Register execution policy exception (for anything that's gonna run with the host kernel at least)
- Touch
- `lsregister` (for macOS apps)

Run:
```sh
xcrun simctl install booted ./target/aarch64-apple-ios-sim/debug/examples/bundle/ios/softbuffer.app
xcrun simctl launch --console booted raytracing.example.softbuffer
# OR
xcrun simctl spawn booted ./target/aarch64-apple-ios-sim/examples/raytracing
```
- Select specific device with `DEVICE="..."`?
- Use some sort of device-global file lock to sequentially launch tests?
  - We need some way to say "this test needs to be bundled and launched" and "this test can be run in parallel / just spawned".
    - Maybe `embed_plist`? Or some other data in the binary - would make it workable with doc tests too.
      - Maybe whether the binary links `UIKit`? Or calls `UIApplicationMain`?
      - `standalone_crate` attribute probably useful here?


## Codesign


`derq` (Generate DER entitlements)
`codesign`
`-Wl,-no_adhoc_codesign`?




## How do we nicely handle embedded / associated binaries?

UI tests.
Extensions.
PlugIns.
Frameworks and dynamic libraries.

Will probably need a `cargo-ios` subcommand for that.


## Working with hot reloading?

`subsecond` todo


## Credits

https://simlay.net/posts/rust-target-runner-for-ios/
cargo-dinghy







## Why

The user-experience of `cargo run --target aarch64-apple-ios-sim`.

This is a development tool only; when deploying on real devices, use something else (such as `cargo-xcode`).

- Current options are too generic, `cargo-mobile2`, `cargo-dinghy` and `crossbow` all , which makes them harder to debug and understand.
- None of the current options make it easy to use Cargo target runners.
- Correctness.
- Support more Apple platforms (such as tvOS, watchOS and visionOS).


## Design choices

Don't parse any `.toml` files; everything is embedded in the binary instead. This makes it much easier to support test binaries.

Don't automatically create and boot simulator devices: we'll have a hard time doing this correctly when running under `cargo test` (we'd need to do a bit of IPC between `cargo-apple-runner` processes), and it's unclear what we should do afterwards (should we shut down the device if we booted it?)


## Limitations

Only supports bundled assets; reading from other directories may fail (we don't copy the entire workspace to the device).

Environment variables.

Use `SIMCTL_CHILD_*` to pass env vars to simctl instances.


## Planned

Support for more advanced features that apps may end up needing, such as:
- Entitlements.
- Application Extensions and Plug-Ins.
- UI testing.

Unsure yet _how_ we're going to support it though.


# TODO


- https://github.com/cargo-bins/cargo-binstall + https://github.com/taiki-e/install-action/blob/main/DEVELOPMENT.md + https://github.com/taiki-e/upload-rust-binary-action

- A test runner where running `cargo test --target aarch64-apple-ios-sim` runs UI tests (including screenshot-based testing probably?)
    - Will run on host, but send the binary to the device.
    - Needs metadata? Otherwise, how can the runner differentiate between needing to package the app, and not needing to?
    - Maybe we just always package, that'll work better for running things on a real device.
    - Custom test runner must run tests on main thread.
- Better error messages using `annotate-snippets`? Or at least something similar, I would like to emit `help` notes.

Needs some sort of integration with `cargo-apple-runner`, because we need to include `XCTest.framework` and `XCUIAutomation.framework` in the bundle.

```rust
// TODO: How do we specify the binary that we want these tests to run against?
// `tags`/`tagsToRun`/`tagsToSkip` in the configuration?
use xctest::prelude::*;

#[ui_test]
fn test(mtm: MainThreadMarker) {
    let app = XCUIApplication::new(mtm);
    let device = XCUIDevice::sharedDevice(mtm);
    let screen = XCUIScreen::mainScreen(mtm);
    // ...
}

#[ui_test]
fn test2(app: &XCUIApplication) {
    // ...
}
```

https://github.com/sonos/dinghy/blob/main/docs/ios.md#additional-requirements


## License

This project is trio-licensed under the [Zlib], [Apache-2.0] or [MIT] license,
at your option.

[MIT]: ./LICENSE-MIT.txt
[Zlib]: ./LICENSE-ZLIB.txt
[Apache-2.0]: ./LICENSE-APACHE.txt
