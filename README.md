# Cargo [runner](https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner) for Apple targets

[![Latest version](https://badgen.net/crates/v/cargo-apple-runner)](https://crates.io/crates/cargo-apple-runner)
[![License](https://badgen.net/badge/license/Zlib%20OR%20Apache-2.0%20OR%20MIT/blue)](./README.md#license)
[![Documentation](https://docs.rs/cargo-apple-runner/badge.svg)](https://docs.rs/cargo-apple-runner/)
[![CI](https://github.com/madsmtm/cargo-apple-runner/actions/workflows/ci.yml/badge.svg)](https://github.com/madsmtm/cargo-apple-runner/actions/workflows/ci.yml)

Easily bundle, sign and launch binaries on Apple targets, including on the simulator and on real devices.


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

Now you can test and run your (GUI) applications on the iOS simulator with:

```sh
cargo test --target aarch64-apple-ios-sim --target aarch64-apple-visionos-sim
cargo run --target aarch64-apple-ios-sim
# etc.
```

Or on Mac Catalyst with:

```sh
cargo run --example my_example --target aarch64-apple-ios-macabi
```


## Supported platforms and requirements

Host: Requires at least macOS 10.12, [same as `rustc`](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html#os-version).
Targets: macOS, Mac Catalyst, iOS, tvOS, watchOS and visionOS.
Simulators: Uses `xcrun simctl`, only tested on Xcode 9.2 and above.
Devices: Yet unsupported, will use `devicectl` (see [#1](https://github.com/madsmtm/cargo-apple-runner/issues/1)) and fall back to `ios-deploy` (see [#2](https://github.com/madsmtm/cargo-apple-runner/issues/2)) on older Xcode.


## Bundling

`cargo-apple-runner` will inspect your binary, and guess whether it needs to bundle it based on a few factors:
- Whether your binary links AppKit, UIKit, WatchKit and similar system GUI frameworks.
- TODO: Maybe something more?
- TODO: Allow overriding this somehow?

Note that this might mean that for documentation tests to be runnable in parallel with `cargo nextest`, you might need to use the [`standalone_crate` attribute](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html#:~:text=standalone_crate) on GUI tests to avoid these making your other doc tests need to be launched as well.


### Custom `Info.plist`

Most real-world applications will want to modify the data in the application's `Info.plist`, you can use the [`embed_plist`](https://docs.rs/embed_plist/) crate to do so:

```rust,ignore
embed_plist::embed_plist!("Info.plist");
```

If this is not done, `cargo-apple-runner` will generate a reasonable `Info.plist` for you.


## Signing

`cargo-apple-runner` will sign your application with whatever signing identity is passed in the `CODE_SIGN_IDENTITY` environment variable. If not set, it will default to "ad-hoc" signing.


### Custom entitlements

In some cases, you might need to request different entitlements for your application.

You can use the [`embed_entitlements`](https://docs.rs/embed_entitlements/) crate to do so:

```rust,ignore
embed_entitlements::embed_entitlements!("my_app.entitlements");
```

Note that when building for a real (non-simulator) device, you will need to configure a provisioning profile with those entitlements allowed. On macOS, certain entitlements are allowed by default, see [this tech note](https://developer.apple.com/documentation/technotes/tn3125-inside-code-signing-provisioning-profiles#Entitlements-on-macOS).

As a small optimization when using entitlements, you can consider adding the following to `.cargo/config.toml` to reduce link-time (since signing will be done by the runner):

```toml
[target.'cfg(all(target_vendor = "apple", not(target_env = "sim")))']
# Signing is done by `cargo-apple-runner`.
rustflags = ["-Clink-arg=-Wl,-no_adhoc_codesign"]
```


## Launching

Similar to when bundling, `cargo-apple-runner` will also guess whether it needs to launch your binary, or whether it can simply spawn it.

Spawning is generally more efficient, since it can be done in parallel, while launching must be serialized (as only a single application can be the frontmost application).


## Usage in CI

Example GitHub Actions workflow that runs tests on macOS, Mac Catalyst, the iOS simulator, the tvOS simulator and the visionOS simulator.

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


## Limitations

This is intended as a development tool only; when deploying on real devices, consider using something else. I can recommend [`cargo-xcode`](https://lib.rs/crates/cargo-xcode), this gives the most control and helps with the complex process of notarizing and submitting to the App Store.

Will only supports bundled assets (at least after [#5](https://github.com/madsmtm/cargo-apple-runner/issues/5)), reading from other directories may fail (we don't copy the entire workspace to the device).

Only a few Cargo environment variables are automatically passed onwards to the simulator, use `SIMCTL_CHILD_*` to explicitly pass the environment variables you want to pass to the program being run.


## License

This project is trio-licensed under the [Zlib], [Apache-2.0] or [MIT] license,
at your option.

[MIT]: ./LICENSE-MIT.txt
[Zlib]: ./LICENSE-ZLIB.txt
[Apache-2.0]: ./LICENSE-APACHE.txt


## Credits

- Original idea for this by [@simlay](https://github.com/simlay): https://simlay.net/posts/rust-target-runner-for-ios/
- [`cargo-bundle`](https://github.com/burtonageo/cargo-bundle)
- [`cargo-dinghy`](https://github.com/sonos/dinghy)
- [`cargo-mobile2`](https://github.com/tauri-apps/cargo-mobile2)
- [`tauri-bundler`](https://crates.io/crates/tauri-bundler)
- [`apple-platform-rs`](https://github.com/indygreg/apple-platform-rs)
- [`fruitbasket`](https://github.com/mrmekon/fruitbasket)
- [`fbidb`](https://fbidb.io)
