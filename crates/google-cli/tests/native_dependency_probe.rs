//! Compile probe ensuring the native Google dependency stack stays type-compatible.

use directories::ProjectDirs;
use google_drive3 as drive3;
use google_gmail1 as gmail1;
use keyring::Entry;
use mail_builder::MessageBuilder;
use mime_guess::from_path;
use reqwest::Client;
use wiremock::MockServer;
use yup_oauth2::ApplicationSecret;

fn compile_probe() {
    let _project_dirs = ProjectDirs::from("com", "nils", "google-cli");

    let _gmail_message = gmail1::api::Message::default();
    let _drive_file = drive3::api::File::default();

    let _oauth_secret = ApplicationSecret::default();
    let _token_entry = Entry::new("nils-google-cli", "default-account");

    let _mail = MessageBuilder::new();
    let _mime = from_path("sample.pdf").first_or_octet_stream();

    let _http_client = Client::new();
    let _mock_server_type = std::mem::size_of::<MockServer>();

    let _google_common_type = std::any::type_name::<google_apis_common::Error>();
    let _browser_launcher = |url: &str| open::that_detached(url);
}

#[test]
fn native_dependency_stack_compiles() {
    compile_probe();
}
