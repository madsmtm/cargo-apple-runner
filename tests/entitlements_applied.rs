use std::ptr;

use objc2_core_foundation::{CFData, CFDictionary, CFString, CFType};
use objc2_security::{
    SecItemAdd, SecItemDelete, errSecMissingEntitlement, errSecSuccess, kSecAttrAccessGroup,
    kSecAttrAccount, kSecClass, kSecClassGenericPassword, kSecValueData,
};

// Keychain group entitlements require a provisioning profile, except on the
// simulator (there we can test this without).
#[cfg(target_env = "sim")]
embed_entitlements::embed_entitlements!("keychain-group.entitlements");

/// Test adding a keychain item in a group that we have entitlements for.
#[test]
fn applied() {
    let query = CFDictionary::<CFString, CFType>::from_slices(
        &[
            unsafe { kSecClass },
            unsafe { kSecAttrAccount },
            unsafe { kSecValueData },
            unsafe { kSecAttrAccessGroup },
        ],
        &[
            unsafe { kSecClassGenericPassword },
            &CFString::from_static_str("test-account"),
            &CFData::from_bytes(b"secret"),
            &CFString::from_static_str("com.somecompany.testgroup"),
        ],
    );

    let _delete = DeleteOnDrop(query.as_opaque());
    let status = unsafe { SecItemAdd(query.as_opaque(), ptr::null_mut()) };
    assert_eq!(status, errSecSuccess);
}

/// Test adding a keychain item in a group that we don't have entitlements for.
#[test]
fn not_applied() {
    let query = CFDictionary::<CFString, CFType>::from_slices(
        &[
            unsafe { kSecClass },
            unsafe { kSecAttrAccount },
            unsafe { kSecValueData },
            unsafe { kSecAttrAccessGroup },
        ],
        &[
            unsafe { kSecClassGenericPassword },
            &CFString::from_static_str("test-account2"),
            &CFData::from_bytes(b"secret2"),
            &CFString::from_static_str("com.unknown.group"),
        ],
    );

    let _delete = DeleteOnDrop(query.as_opaque());
    let status = unsafe { SecItemAdd(query.as_opaque(), ptr::null_mut()) };
    if cfg!(target_env = "sim") {
        assert_eq!(status, errSecMissingEntitlement);
    } else if cfg!(target_os = "macos") {
        assert_eq!(status, errSecSuccess);
    } else {
        todo!("unsure?")
    }
}

/// Clean up the item on drop.
struct DeleteOnDrop<'a>(&'a CFDictionary);

impl Drop for DeleteOnDrop<'_> {
    fn drop(&mut self) {
        unsafe { SecItemDelete(self.0) };
    }
}
