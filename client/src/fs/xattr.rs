use super::prelude::*;

/// Handles the `getxattr` request (Read extended attribute).
///
/// macOS often requests attributes like `com.apple.quarantine` or `com.apple.FinderInfo`.
/// We always reply that the attribute does not exist (`ENOATTR` on macOS, `ENODATA` on Linux).
/// This is safe behavior that tells the OS "this file has no special metadata".
pub fn getxattr(_fs: &mut RemoteFS, _req: &Request, _ino: u64, _name: &OsStr, _size: u32, reply: ReplyXattr) {
    #[cfg(target_os = "macos")]
    reply.error(ENOATTR);

    #[cfg(not(target_os = "macos"))]
    reply.error(ENODATA);
}

/// Handles the `setxattr` request (Write extended attribute).
///
/// If Finder tries to set an icon, a tag, or quarantine info, we pretend the operation
/// succeeded (`reply.ok()`) but we do not actually store the data on the server.
///
/// This "fake success" avoids user-visible errors (e.g., "Cannot copy file", "Error -36")
/// when interacting with the filesystem via Finder.
pub fn setxattr(_fs: &mut RemoteFS, _req: &Request, _ino: u64, _name: &OsStr, _value: &[u8], _flags: i32, _position: u32, reply: ReplyEmpty) {
    reply.ok();
}

/// Handles the `listxattr` request (List extended attributes).
///
/// We always reply with an empty list, indicating the file has no special extended attributes.
pub fn listxattr(_fs: &mut RemoteFS, _req: &Request, _ino: u64, size: u32, reply: ReplyXattr) {
    if size == 0 {
        // If size is 0, the kernel is asking "how many bytes do you need for the list?".
        // We reply 0 bytes (empty list).
        reply.size(0);
    } else {
        // If size > 0, the kernel wants the actual list data.
        // We send an empty array.
        reply.data(&[]);
    }
}

/// Handles the `removexattr` request (Remove extended attribute).
///
/// We pretend success (`reply.ok()`) even if there was nothing to remove.
pub fn removexattr(_fs: &mut RemoteFS, _req: &Request, _ino: u64, _name: &OsStr, reply: ReplyEmpty) {
    reply.ok();
}