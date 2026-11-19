pub mod document_service;
pub mod folder_service;
pub mod lan_sync;
#[cfg(desktop)]
pub mod mcp;
pub mod sync_engine;
pub mod utils;
pub mod webdav_sync;

// Stub modules (to be fully implemented)
pub mod auth_service;
pub mod lock_service;
pub mod version_service;
