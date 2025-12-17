//! This prelude module re-exports all common types, traits, and functions
//! used across the `fs` module's sub-files (e.g., `read.rs`, `write.rs`).
//!
//! This avoids repetitive imports in every file and provides a single
//! location to manage shared dependencies for the FUSE implementation.

// --- FUSE Types ---
/// Re-exports all common FUSE types for filesystem operations and replies.
pub use fuser::{
    FileAttr, FileType, ReplyAttr, ReplyCreate, ReplyData,
    ReplyDirectory, ReplyEntry, ReplyOpen, ReplyWrite, Request, ReplyEmpty,
    TimeOrNow,
    // --- MACOS ---
    ReplyXattr
};

// --- LibC Error Codes ---
/// Re-exports standard `libc` error codes used to reply to FUSE.
pub use libc::{
    EIO,     // Errore I/O
    ENOENT,  // File/Dir non trovata
    EBADF,   // Bad file descriptor
    ENOTEMPTY, // Directory non vuota
};
#[cfg(not(target_os = "macos"))]
pub use libc::ENODATA;
#[cfg(target_os = "macos")]
pub use libc::ENOATTR;

// --- Standard Library Types ---
/// Re-exports common types from the Rust standard library.
pub use std::collections::HashMap;
pub use std::ffi::OsStr;
pub use std::time::{Duration, SystemTime, UNIX_EPOCH};

// --- External Crate Types ---
/// Re-exports `Bytes` for efficient byte buffer handling.
pub use bytes::Bytes;

// --- Internal Project Modules ---
/// Re-exports the API client functions for server communication.
pub use crate::api_client::{
    self, // Allows using `api_client::function_name`
    put_file_content_to_server,
    get_file_content_from_server,
    get_files_from_server,
    delete_resource,
    create_directory,
    update_permissions,
    get_file_chunk_from_server
};

// --- Internal `fs` Module Types ---
/// Re-exports the core structs and constants defined in `fs/mod.rs`.
pub use super::{
    RemoteFS,      // The main filesystem state struct
    OpenWriteFile, // The struct for the in-memory write cache
    TTL,           // The default Time-To-Live for kernel caches
    ROOT_DIR_ATTR, // The static attributes for the root directory
};