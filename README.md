# post-tui

`post-tui` is a professional terminal HTTP API client for developers who want a fast, keyboard-driven API workflow without leaving the terminal.

The app is written in Rust with `ratatui`, `crossterm`, `reqwest`, `tokio`, `serde`, and local JSON/TOML storage. It is designed to stay usable on modest hardware and small terminal windows.

## Installation

Install Rust stable from <https://rustup.rs/>, then install `post-tui` from crates.io:

```bash
cargo install post-tui
```

Run it with:

```bash
post-tui
```

To build from a local source checkout:

```bash
cd post-tui
cargo build --release
./target/release/post-tui
```

## Features

- Terminal-native HTTP API client with a focused request, response, collection, and history workflow.
- Supports `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, and `OPTIONS`.
- Request builder for URL, method, headers, query params, auth, and raw body.
- Auth modes: No Auth, Bearer Token, Basic Auth, and API Key Header.
- Response viewer with status code, response time, response size, headers, body, errors, and scroll support.
- Pretty JSON response view.
- Lightweight JSON tree viewer.
- Raw response body view.
- HTML response text view.
- Open HTML responses in the system browser.
- Search within the active response tab.
- Generate a cURL command from the active request.
- Import Collection v2.1 JSON files.
- Browse imported collections, nested folders, saved requests, and request history.
- Delete collections, folders, individual collection requests, saved requests, and history items.
- Save requests locally.
- Clear request history and local HTML response exports.
- Local app data stored in the platform data directory.
- Built-in help and about modals.

## Interface

`post-tui` uses a compact layout:

- Left sidebar: collections, saved requests, and history.
- Top bar: app name, current mode, and shortcut hints.
- Main panel: request builder.
- Bottom panel: response viewer.
- Footer: contextual help and status messages.
- Modal popups: help, about, import, search, cURL, errors, and confirmations.

## Keyboard Shortcuts

Press `?` inside the app to open the built-in help modal.

| Key | Action |
| --- | --- |
| `?` | Open help |
| `a` | Open about |
| `q` | Quit |
| `r` / `Ctrl+r` | Run current request |
| `Enter` | Edit focused request field or load selected sidebar request |
| `Ctrl+s` | Save current request |
| `n` | Create a new request |
| `u` | Generate cURL for the current request |
| `Tab` / `Shift+Tab` | Switch panels |
| `j/k` or arrow keys | Navigate sidebar/request fields or scroll response |
| `h/l` or left/right arrows | Change method, auth mode, or response tab |
| `PageUp` / `PageDown` | Scroll response faster |
| `Home` / `End` | Jump to start/end of response |
| `/` | Search collections/history/sidebar entries |
| `/` in response panel | Search within the active response tab |
| `o` | Import a collection JSON file |
| `d` | Delete selected sidebar item |
| `c` | Clear history and local HTML response exports |
| `b` in HTML response tab | Open HTML response in browser |
| `Esc` | Cancel edit or close modal |

## Editing Requests

Focus the request builder with `Tab`, move between fields with `j/k`, then press `Enter` to edit the selected field.

Supported field formats:

- Headers: one per line, `Header-Name: value`
- Query params: one per line, `key=value`
- Bearer auth: token only
- Basic auth: `username:password`
- API key auth: `header-name: value`
- Raw body: any text; invalid JSON is allowed and sent as raw text

The input cursor is shown with a blinking block cursor so blank fields are easier to understand.

## Running Requests

1. Start the app:

   ```bash
   post-tui
   ```

2. Edit the URL field with `Enter`.
3. Change method with left/right arrows while focused on `Method`.
4. Press `r` to run the request.
5. Move to the response panel with `Tab`.
6. Use `h/l` to switch response tabs.

The default request URL is:

```text
https://example.com/api
```

## Response Viewer

Response tabs:

- `Pretty`: pretty JSON when possible, otherwise raw text.
- `Tree`: lightweight JSON tree view.
- `Raw`: original response body.
- `HTML`: HTML converted to readable terminal text.
- `Headers`: response headers.
- `Error`: request or network errors.

Response controls:

- `j/k` or arrow keys scroll line by line.
- `PageUp/PageDown` scroll faster.
- `Home/End` jump to the start/end.
- `/` searches the active response tab and jumps to the first match.
- `b` opens the last HTML response in the system browser from the `HTML` tab.

## cURL Generation

Press `u` to generate a cURL command for the current request. The generated command includes:

- HTTP method
- URL with query params
- Enabled headers
- Auth headers or basic auth
- Raw request body
- Content-Type when applicable

## Collection Import

Press `o`, type the path to a Collection v2.1 JSON file, then press `Enter`.

Example from this repository:

```text
examples/sample-postman-collection.json
```

Imported requests appear in the sidebar. Select a request with `j/k`; it loads into the request builder automatically.

Supported import data:

- Collection name
- Nested folders
- Request name
- Request method
- URL as raw string or structured URL object
- Headers
- Query params
- Raw body

## Collections, Saved Requests, and History

The sidebar can contain:

- Imported collections
- Nested folders
- Requests inside collections
- Saved local requests
- Request history

Useful shortcuts:

- `Ctrl+s`: save the current request.
- `d`: delete the selected sidebar item after confirmation.
- `c`: clear request history and delete local HTML response exports after confirmation.

Delete supports:

- Entire imported collections
- Folders inside collections
- Individual collection requests
- Saved requests
- Individual history items

## Local Data

`post-tui` stores app data in the platform data directory reported in the footer.

Files include:

- `collections.json`
- `history.json`
- `requests.json`
- `config.toml`
- `*.html` response exports created by the browser view action

Missing or corrupted local files are handled gracefully and recreated when data is saved.

## Sample Collection

This repository includes a sample collection:

```text
examples/sample-postman-collection.json
```

It contains sample JSON and HTML requests that are useful for testing import, response tabs, and browser view.

## Current Limitations

- Editing is intentionally compact; multiline values are entered through a simple terminal input buffer.
- Collection import supports common v2.1 structures, but not every vendor-specific feature.
- HTML responses are rendered as readable terminal text, not as a full browser engine.
- Requests currently run through the main app flow rather than a cancellable background task.
- Clipboard support is not enabled yet.
- Environment variables and variable substitution are not implemented yet.

## Roadmap

- Full multiline editor for headers and body.
- Environment variables such as `{{base_url}}` and `{{token}}`.
- Collection export.
- Background requests with cancellation.
- Clipboard integration for cURL and response bodies.
- Request tabs.
- Config screen for timeout, redirects, default headers, and theme.

## License

MIT

## Author

Created by raufendro.
