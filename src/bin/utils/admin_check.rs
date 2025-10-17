// Windows admin check utility
// Returns true if running as administrator, false otherwise
#[cfg(windows)]
pub fn is_running_as_admin() -> bool {
    use windows::Win32::Security::{CheckTokenMembership, AllocateAndInitializeSid, FreeSid, SID_IDENTIFIER_AUTHORITY, PSID};
    use windows_core::BOOL;
    unsafe {
        let nt_authority = SID_IDENTIFIER_AUTHORITY { Value: [0, 0, 0, 0, 0, 5] };
        let mut admin_group: PSID = PSID::default();
        let alloc_ok = AllocateAndInitializeSid(
            &nt_authority,
            2,
            32, 544, 0, 0, 0, 0, 0, 0,
            &mut admin_group
        ).is_ok();
        if !alloc_ok {
            return false;
        }
        let mut is_admin = BOOL(0);
        let check = CheckTokenMembership(None, admin_group, &mut is_admin);
        let _ = FreeSid(admin_group);
        check.is_ok() && is_admin.as_bool()
    }
}
