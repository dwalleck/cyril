use std::io;

use interprocess::os::windows::named_pipe::tokio::{PipeListener, PipeStream};
use interprocess::os::windows::named_pipe::{PipeListenerOptions, pipe_mode};
use interprocess::os::windows::security_descriptor::SecurityDescriptor;
use widestring::U16CString;
use win_security_identifier::{GetCurrentSid, SecurityIdentifier};

use super::{IpcError, MemoryEndpoint};
use crate::wire::BoxedStream;

pub(super) struct Listener {
    inner: PipeListener<pipe_mode::Bytes, pipe_mode::Bytes>,
}

impl Listener {
    pub(super) async fn accept(&self) -> Result<BoxedStream, IpcError> {
        let stream = self
            .inner
            .accept()
            .await
            .map_err(|source| IpcError::Accept { source })?;
        Ok(Box::new(stream))
    }
}

pub(super) async fn bind(endpoint: &MemoryEndpoint) -> Result<Listener, IpcError> {
    let current_user =
        SecurityIdentifier::get_current_user_sid().map_err(|error| IpcError::Protect {
            source: io::Error::new(io::ErrorKind::PermissionDenied, error),
        })?;
    let sddl = U16CString::from_str(format!("D:P(A;;GA;;;{current_user})")).map_err(|error| {
        IpcError::Protect {
            source: io::Error::new(io::ErrorKind::InvalidData, error),
        }
    })?;
    let descriptor = SecurityDescriptor::deserialize(sddl.as_ucstr())
        .map_err(|source| IpcError::Protect { source })?;
    let inner = PipeListenerOptions::new()
        .path(endpoint.as_path())
        .accept_remote(false)
        .security_descriptor(Some(descriptor))
        .create_tokio_duplex::<pipe_mode::Bytes>()
        .map_err(|source| IpcError::Bind { source })?;
    Ok(Listener { inner })
}

pub(super) async fn connect(endpoint: &MemoryEndpoint) -> Result<BoxedStream, IpcError> {
    let stream =
        PipeStream::<pipe_mode::Bytes, pipe_mode::Bytes>::connect_by_path(endpoint.as_path())
            .await
            .map_err(|source| IpcError::Connect { source })?;
    Ok(Box::new(stream))
}

#[cfg(test)]
mod tests {
    use std::os::windows::io::{AsHandle, AsRawHandle, RawHandle};
    use std::path::Path;
    use std::str::FromStr;

    use windows_permissions::constants::{
        AccessRights, AceFlags, AceType, SeObjectType, SecurityInformation,
    };
    use windows_permissions::wrappers::GetSecurityInfo;
    use windows_permissions::{LocalBox, Sid};

    use super::*;

    struct RawHandleView(RawHandle);

    impl AsRawHandle for RawHandleView {
        fn as_raw_handle(&self) -> RawHandle {
            self.0
        }
    }

    #[tokio::test]
    async fn live_pipe_dacl_is_protected_and_current_user_only()
    -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = MemoryEndpoint::from_path(Path::new(r"C:\cyril-memory-ipc-test"))?;
        let listener = bind(&endpoint).await?;
        let (server, client) = tokio::try_join!(
            listener.inner.accept(),
            PipeStream::<pipe_mode::Bytes, pipe_mode::Bytes>::connect_by_path(endpoint.as_path())
        )?;

        let handle = RawHandleView(server.as_handle().as_raw_handle());
        let descriptor = GetSecurityInfo(
            &handle,
            SeObjectType::SE_KERNEL_OBJECT,
            SecurityInformation::Dacl,
        )?;
        let sddl = descriptor.as_sddl()?;
        assert!(sddl.to_string_lossy().contains("D:P"));
        let dacl = descriptor.dacl().ok_or("pipe DACL missing")?;
        assert_eq!(dacl.len(), 1);
        let ace = dacl.get_ace(0).ok_or("pipe allow ACE missing")?;
        let current_user = SecurityIdentifier::get_current_user_sid()?.to_string();
        let expected_sid = LocalBox::<Sid>::from_str(&current_user)?;
        assert_eq!(ace.ace_type(), AceType::ACCESS_ALLOWED_ACE_TYPE);
        assert_eq!(ace.sid(), Some(expected_sid.as_ref()));
        assert!(!ace.flags().contains(AceFlags::Inherited));
        assert!(
            ace.mask().contains(AccessRights::GenericAll)
                || ace.mask().contains(AccessRights::FileAllAccess)
        );

        drop(client);
        drop(server);
        Ok(())
    }
}
