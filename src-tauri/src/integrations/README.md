# Tori Integrations

Rust integration crate for a Tauri 2 macOS app.

OAuth credentials and tokens are not stored in a database. OAuth tokens are persisted through the operating system credential store. On macOS, the `keyring` v1 backend uses Keychain Services.

## Included integrations

Google:

- OAuth 2.0 desktop authorization code flow with PKCE and a loopback callback
- Gmail list, get, send
- Google Calendar list calendars, list events, create event
- Google Drive list app-accessible files, multipart upload
- Google Docs create document and insert text

Microsoft:

- Microsoft Entra ID authorization code flow with PKCE and a loopback callback
- Outlook list and send mail through Microsoft Graph
- Outlook Calendar list and create events through Microsoft Graph
- OneDrive list root and upload files up to 250 MB through Microsoft Graph

## Add to a Tauri app

Place this folder at `src-tauri/integrations` and add this to the parent `src-tauri/Cargo.toml`:

```toml
tori-integrations = { path = "integrations" }
```

Initialize the state in your Tauri builder:

```rust
use tori_integrations::{Integrations, google::GoogleConfig, microsoft::MicrosoftConfig};

let integrations = Integrations::new(
    "com.yourcompany.tori",
    GoogleConfig::workspace(
        std::env::var("TORI_GOOGLE_CLIENT_ID").expect("TORI_GOOGLE_CLIENT_ID"),
        std::env::var("TORI_GOOGLE_CLIENT_SECRET").ok(),
    ),
    MicrosoftConfig::graph(
        std::env::var("TORI_MICROSOFT_CLIENT_ID").expect("TORI_MICROSOFT_CLIENT_ID"),
    ),
)?;
```

The `account` argument used by the commands is only a local Keychain slot name. It is not a cloud user ID. Use `"default"` for a single connected Google account and a single connected Microsoft account, or use stable local aliases if you want multiple accounts.

Manage it and register whichever commands your UI uses:

```rust
.manage(integrations)
.invoke_handler(tauri::generate_handler![
    tori_integrations::commands::google_connect,
    tori_integrations::commands::google_disconnect,
    tori_integrations::commands::google_status,
    tori_integrations::commands::google_gmail_list,
    tori_integrations::commands::google_gmail_get,
    tori_integrations::commands::google_gmail_send,
    tori_integrations::commands::google_calendar_list,
    tori_integrations::commands::google_calendar_events,
    tori_integrations::commands::google_calendar_create_event,
    tori_integrations::commands::google_drive_list,
    tori_integrations::commands::google_drive_upload,
    tori_integrations::commands::google_docs_create,
    tori_integrations::commands::microsoft_connect,
    tori_integrations::commands::microsoft_disconnect,
    tori_integrations::commands::microsoft_status,
    tori_integrations::commands::microsoft_outlook_list,
    tori_integrations::commands::microsoft_outlook_send,
    tori_integrations::commands::microsoft_calendar_list,
    tori_integrations::commands::microsoft_calendar_events,
    tori_integrations::commands::microsoft_calendar_create_event,
    tori_integrations::commands::microsoft_onedrive_list,
    tori_integrations::commands::microsoft_onedrive_upload,
])
```

## Google setup

Create a Google OAuth client with application type `Desktop app`. Enable Gmail API, Google Calendar API, Google Drive API, and Google Docs API.

The default scopes are:

```text
https://www.googleapis.com/auth/gmail.readonly
https://www.googleapis.com/auth/gmail.send
https://www.googleapis.com/auth/calendar.calendarlist.readonly
https://www.googleapis.com/auth/calendar.events
https://www.googleapis.com/auth/drive.file
```

`drive.file` intentionally limits Drive access to files the app creates or files the user explicitly grants to the app. It is also sufficient for the Docs create/update operations used by this crate. Replace it with a broader Drive scope only if the product genuinely needs broad Drive visibility.

## Microsoft setup

Create an app registration in Microsoft Entra ID. Configure it as a public desktop client, add `http://localhost/oauth/callback` as a Mobile and desktop application redirect URI, and enable public client flows. Microsoft ignores the ephemeral port when matching a registered `localhost` redirect URI. Do not create or embed a Microsoft client secret in the desktop app.

The default delegated scopes are:

```text
offline_access
Mail.Read
Mail.Send
Calendars.ReadWrite
Files.ReadWrite
```

The default authority uses the `common` tenant so both personal Microsoft accounts and organizational accounts can authenticate. Set `MicrosoftConfig.tenant` to a tenant ID or tenant domain if Tori should be organization-only.

## Local security model

OAuth state and PKCE verifier values live only in memory for the active authentication attempt. The loopback listener binds only to `127.0.0.1` on a random port and shuts down after the callback. Tokens are persisted locally through Keychain on macOS. Microsoft Graph pagination URLs are restricted to `https://graph.microsoft.com/v1.0/`, and automatic HTTP redirects are disabled on authenticated API clients to avoid forwarding bearer tokens to another origin. No Prisma, Drizzle, PostgreSQL, hosted token service, or remote application database is used by this crate.
