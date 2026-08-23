//! Platform-specific protection for the memory data directory.

use std::fs;
use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Make `path` private to the current user and reject a symlink at the final
/// path component.  Callers validate the path before this operation and map
/// this I/O error to their public error vocabulary.
#[cfg(unix)]
pub(crate) fn tighten_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "memory data root must not be a symlink",
        ));
    }
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "memory data root must be a directory",
        ));
    }

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

/// Apply a protected, current-user-only inheritable DACL to the directory.
///
/// The `OICI` ACE flags make access flow to files and subdirectories created
/// below the root.  `P` protects the DACL from inheriting broader access from
/// its parent.  Both the write and read-back verification use safe wrappers
/// supplied by `windows-permissions`; this crate contains no Windows `unsafe`.
#[cfg(windows)]
pub(crate) fn tighten_directory(path: &Path) -> io::Result<()> {
    use std::str::FromStr;

    use win_security_identifier::{GetCurrentSid, SecurityIdentifier};
    use windows_permissions::constants::{
        AccessRights, AceFlags, AceType, SeObjectType, SecurityInformation,
    };
    use windows_permissions::wrappers::{GetNamedSecurityInfo, SetNamedSecurityInfo};
    use windows_permissions::{LocalBox, SecurityDescriptor, Sid};

    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "memory data root must not be a symlink",
        ));
    }
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "memory data root must be a directory",
        ));
    }

    let current_sid = SecurityIdentifier::get_current_user_sid()
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
    let sid = current_sid.to_string();
    let descriptor_text = format!("D:P(A;OICI;GA;;;{sid})");
    let descriptor = LocalBox::<SecurityDescriptor>::from_str(&descriptor_text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let expected_sid = LocalBox::<Sid>::from_str(&sid)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let dacl = descriptor.dacl().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "protected current-user DACL did not contain an ACL",
        )
    })?;

    SetNamedSecurityInfo(
        path,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Dacl | SecurityInformation::ProtectedDacl,
        None,
        None,
        Some(dacl),
        None,
    )?;

    let applied = GetNamedSecurityInfo(
        path,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Dacl,
    )?;
    let applied_sddl_value = applied.as_sddl()?;
    let applied_sddl = applied_sddl_value.to_string_lossy();
    let applied_dacl = applied.dacl().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "memory data root has no DACL after hardening",
        )
    })?;
    let mut direct_aces = 0_u32;
    let mut inheritable_aces = 0_u32;
    let mut current_user_only = applied_dacl.len() == 2;
    for index in 0..applied_dacl.len() {
        let Some(ace) = applied_dacl.get_ace(index) else {
            current_user_only = false;
            break;
        };
        let flags = ace.flags();
        let rights = ace.mask();
        let common = ace.ace_type() == AceType::ACCESS_ALLOWED_ACE_TYPE
            && ace.sid() == Some(expected_sid.as_ref())
            && !flags.contains(AceFlags::Inherited)
            && (rights.contains(AccessRights::GenericAll)
                || rights.contains(AccessRights::FileAllAccess));
        if !common {
            current_user_only = false;
            break;
        }
        if flags
            .contains(AceFlags::ObjectInherit | AceFlags::ContainerInherit | AceFlags::InheritOnly)
        {
            inheritable_aces += 1;
        } else if !flags.intersects(
            AceFlags::ObjectInherit | AceFlags::ContainerInherit | AceFlags::InheritOnly,
        ) {
            direct_aces += 1;
        } else {
            current_user_only = false;
            break;
        }
    }
    current_user_only &= direct_aces == 1 && inheritable_aces == 1;
    if !applied_sddl.contains("D:P") || !current_user_only {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "memory data root DACL verification failed",
        ));
    }

    Ok(())
}

/// Fail explicitly on platforms whose directory permission model is not
/// implemented rather than silently weakening the data-root guarantee.
#[cfg(not(any(unix, windows)))]
pub(crate) fn tighten_directory(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "private memory-directory permissions are unsupported on this platform",
    ))
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::tighten_directory;
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn tightens_directory_to_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("tempdir");
        let target = root.path().join("memory");
        fs::create_dir(&target).expect("directory");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).expect("perms");

        tighten_directory(&target).expect("tighten");

        let mode = fs::metadata(&target)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink() {
        let root = tempfile::tempdir().expect("tempdir");
        let target = root.path().join("target");
        let link = root.path().join("link");
        fs::create_dir(&target).expect("directory");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        let error = tighten_directory(&link).expect_err("symlink must be rejected");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
