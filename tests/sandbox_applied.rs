use std::ptr;

use objc2_core_foundation::{CFBoolean, CFString};
use objc2_security::SecTask;

embed_entitlements::embed_entitlements!("sandbox.entitlements");

#[test]
fn applied() {
    let task = unsafe { SecTask::from_self(None) }.unwrap();
    let value = unsafe {
        task.value_for_entitlement(
            &CFString::from_static_str("com.apple.security.app-sandbox"),
            ptr::null_mut(),
        )
    };
    assert!(
        value
            .expect("must have app sandbox entitlement")
            .downcast::<CFBoolean>()
            .unwrap()
            .as_bool()
    );

    if cfg!(target_os = "macos") {
        assert!(
            std::env::var_os("APP_SANDBOX_CONTAINER_ID").is_some(),
            "must have sandbox container ID"
        );
    }
}
