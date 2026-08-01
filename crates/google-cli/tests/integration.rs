// Consolidated integration test target.
// Each former `tests/*.rs` is declared as a submodule here so the crate
// links one integration test binary instead of many. This keeps the
// dev-loop link phase O(crates) instead of O(test-files).

#[path = "integration/common/mod.rs"]
pub mod common;
#[path = "integration/common/native_calendar.rs"]
pub mod native_calendar;
#[path = "integration/common/native_drive.rs"]
pub mod native_drive;
#[path = "integration/common/native_gmail.rs"]
pub mod native_gmail;

#[path = "integration/account_resolution_shared.rs"]
mod account_resolution_shared;
#[path = "integration/auth_account_resolution.rs"]
mod auth_account_resolution;
#[path = "integration/auth_cli_contract.rs"]
mod auth_cli_contract;
#[path = "integration/auth_oauth_flow.rs"]
mod auth_oauth_flow;
#[path = "integration/auth_storage.rs"]
mod auth_storage;
#[path = "integration/calendar_read.rs"]
mod calendar_read;
#[path = "integration/cli_contract.rs"]
mod cli_contract;
#[path = "integration/drive_cli_contract.rs"]
mod drive_cli_contract;
#[path = "integration/drive_download.rs"]
mod drive_download;
#[path = "integration/drive_read.rs"]
mod drive_read;
#[path = "integration/drive_upload.rs"]
mod drive_upload;
#[path = "integration/gmail_cli_contract.rs"]
mod gmail_cli_contract;
#[path = "integration/gmail_read.rs"]
mod gmail_read;
#[path = "integration/gmail_send.rs"]
mod gmail_send;
#[path = "integration/gmail_thread.rs"]
mod gmail_thread;
#[path = "integration/native_dependency_probe.rs"]
mod native_dependency_probe;
#[path = "integration/native_no_gog.rs"]
mod native_no_gog;
