use std::{fs, process::Command, time::SystemTime};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    http::HttpClient,
    input::TextInput,
    models::{
        AuthConfig, BodyConfig, Collection, Folder, Header, HistoryItem, QueryParam, RequestItem,
        RequestModel, ResponseModel,
    },
    postman,
    storage::{AppConfig, Storage},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Sidebar,
    Request,
    Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestField {
    Method,
    Url,
    Headers,
    Query,
    Auth,
    Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseTab {
    Pretty,
    JsonTree,
    Raw,
    Html,
    Headers,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Help,
    About,
    Curl(String),
    Import,
    Search,
    ResponseSearch,
    ConfirmNewRequest,
    ConfirmClearHistory,
    ConfirmDeleteSidebar(String),
    Error(String),
    Info(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTarget {
    Url,
    Headers,
    Query,
    Body,
    Bearer,
    Basic,
    ApiKey,
    ImportPath,
    Search,
    ResponseSearch,
}

#[derive(Debug, Clone)]
pub enum SidebarEntry {
    Collection {
        label: String,
        collection_index: Option<usize>,
    },
    Folder {
        label: String,
        collection_index: usize,
        folder_path: Vec<usize>,
    },
    Request {
        label: String,
        request: RequestModel,
        source: RequestSource,
    },
    History {
        label: String,
        request: RequestModel,
        index: usize,
    },
    Saved {
        label: String,
        request: RequestModel,
        index: usize,
    },
}

#[derive(Debug, Clone)]
pub enum RequestSource {
    CollectionRoot {
        collection_index: usize,
        request_index: usize,
    },
    CollectionFolder {
        collection_index: usize,
        folder_path: Vec<usize>,
        request_index: usize,
    },
}

pub struct App {
    pub should_quit: bool,
    pub focus: Panel,
    pub request_field: RequestField,
    pub response_tab: ResponseTab,
    pub response_scroll: u16,
    pub current_request: RequestModel,
    pub response: Option<ResponseModel>,
    pub collections: Vec<Collection>,
    pub history: Vec<HistoryItem>,
    pub saved_requests: Vec<RequestItem>,
    pub sidebar_entries: Vec<SidebarEntry>,
    pub selected_sidebar: usize,
    pub modal: Option<Modal>,
    pub edit_target: Option<EditTarget>,
    pub input: TextInput,
    pub response_search: String,
    pub status_line: String,
    pub is_sending: bool,
    pub blink_on: bool,
    blink_tick: u8,
    pub storage: Storage,
    pub config: AppConfig,
}

impl App {
    pub fn new() -> Result<Self> {
        let storage = Storage::new()?;
        let config = storage.load_config();
        let _ = storage.save_config(&config);
        let collections = storage.load_collections();
        let history = storage.load_history();
        let saved_requests = storage.load_saved_requests();

        let mut app = Self {
            should_quit: false,
            focus: Panel::Request,
            request_field: RequestField::Url,
            response_tab: ResponseTab::Pretty,
            response_scroll: 0,
            current_request: RequestModel::default(),
            response: None,
            collections,
            history,
            saved_requests,
            sidebar_entries: vec![],
            selected_sidebar: 0,
            modal: None,
            edit_target: None,
            input: TextInput::default(),
            response_search: String::new(),
            status_line: format!("Data: {}", storage.data_dir().display()),
            is_sending: false,
            blink_on: true,
            blink_tick: 0,
            storage,
            config,
        };
        app.refresh_sidebar();
        Ok(app)
    }

    pub fn tick(&mut self) {
        self.blink_tick = (self.blink_tick + 1) % 5;
        if self.blink_tick == 0 {
            self.blink_on = !self.blink_on;
        }
    }

    pub async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if self.edit_target.is_some() {
            return self.handle_edit_key(key).await;
        }

        if self.modal.is_some() {
            return self.handle_modal_key(key).await;
        }

        match key {
            KeyEvent {
                code: KeyCode::Char('q'),
                ..
            } => self.should_quit = true,
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => self.save_current_request()?,
            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers,
                ..
            } if modifiers.is_empty() || modifiers.contains(KeyModifiers::CONTROL) => {
                self.send_current_request().await?
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers,
                ..
            } if modifiers.is_empty() => self.start_edit_for_focus(),
            KeyEvent {
                code: KeyCode::Char('?'),
                ..
            } => self.open_help(),
            KeyEvent {
                code: KeyCode::Char('a'),
                ..
            } => self.modal = Some(Modal::About),
            KeyEvent {
                code: KeyCode::Char('u'),
                ..
            } => self.modal = Some(Modal::Curl(generate_curl(&self.current_request))),
            KeyEvent {
                code: KeyCode::Char('o'),
                ..
            } => self.open_import(),
            KeyEvent {
                code: KeyCode::Char('n'),
                ..
            } => self.open_new_request(),
            KeyEvent {
                code: KeyCode::Char('/'),
                ..
            } => self.open_search(),
            KeyEvent {
                code: KeyCode::Char('c'),
                ..
            } => self.modal = Some(Modal::ConfirmClearHistory),
            KeyEvent {
                code: KeyCode::Char('d'),
                ..
            } if self.focus == Panel::Sidebar => self.open_delete_selected_sidebar(),
            KeyEvent {
                code: KeyCode::Char('b'),
                ..
            } if self.focus == Panel::Response && self.response_tab == ResponseTab::Html => {
                self.open_html_in_browser()?
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => self.switch_panel(true),
            KeyEvent {
                code: KeyCode::BackTab,
                ..
            } => self.switch_panel(false),
            KeyEvent {
                code: KeyCode::Char('h') | KeyCode::Left,
                ..
            } => self.move_horizontal(false),
            KeyEvent {
                code: KeyCode::Char('l') | KeyCode::Right,
                ..
            } => self.move_horizontal(true),
            KeyEvent {
                code: KeyCode::Char('j') | KeyCode::Down,
                ..
            } => self.move_vertical(true),
            KeyEvent {
                code: KeyCode::Char('k') | KeyCode::Up,
                ..
            } => self.move_vertical(false),
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } if self.focus == Panel::Response => self.scroll_response(10),
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } if self.focus == Panel::Response => self.scroll_response(-10),
            KeyEvent {
                code: KeyCode::Home,
                ..
            } if self.focus == Panel::Response => self.response_scroll = 0,
            KeyEvent {
                code: KeyCode::End, ..
            } if self.focus == Panel::Response => {
                self.response_scroll = self.response_scroll_limit()
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn send_current_request(&mut self) -> Result<()> {
        self.is_sending = true;
        self.status_line = "Sending request...".to_string();
        let client = HttpClient::new()?;
        let response = client.send(&self.current_request).await;
        self.is_sending = false;
        self.status_line = if let Some(error) = &response.error {
            error.clone()
        } else {
            format!(
                "Response received: {} in {} ms",
                response.status.unwrap_or_default(),
                response.elapsed_ms
            )
        };

        self.history.insert(
            0,
            HistoryItem {
                name: self.current_request.name.clone(),
                method: self.current_request.method,
                url: self.current_request.url.clone(),
                status: response.status,
                elapsed_ms: response.elapsed_ms,
                timestamp: timestamp(),
                request: self.current_request.clone(),
            },
        );
        self.history.truncate(100);
        self.storage.save_history(&self.history)?;
        self.response = Some(response);
        self.response_scroll = 0;
        self.response_search.clear();
        self.refresh_sidebar();
        Ok(())
    }

    pub fn import_postman_collection(&mut self, path: &str) -> Result<()> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Could not read import file: {path}"))?;
        let collection = postman::parse_collection(&contents)?;
        let name = collection.name.clone();
        self.collections.push(collection);
        self.storage.save_collections(&self.collections)?;
        self.refresh_sidebar();
        self.modal = Some(Modal::Info(format!("Imported collection: {name}")));
        self.status_line = "Collection imported successfully.".to_string();
        Ok(())
    }

    pub fn save_current_request(&mut self) -> Result<()> {
        let mut request = self.current_request.clone();
        if request.name.trim().is_empty() || request.name == "Untitled request" {
            request.name = request.url.clone();
        }
        self.saved_requests.insert(
            0,
            RequestItem {
                name: request.name.clone(),
                request,
            },
        );
        self.storage.save_saved_requests(&self.saved_requests)?;
        self.refresh_sidebar();
        self.status_line = "Request saved locally.".to_string();
        Ok(())
    }

    pub fn new_request(&mut self) {
        self.current_request = RequestModel::default();
        self.current_request.name = "New request".to_string();
        self.response = None;
        self.response_scroll = 0;
        self.request_field = RequestField::Url;
        self.focus = Panel::Request;
        self.status_line =
            "Created a new request. Edit URL, method, headers, auth, or body.".to_string();
    }

    pub fn open_html_in_browser(&mut self) -> Result<()> {
        let Some(response) = &self.response else {
            self.status_line = "No response to open in a browser.".to_string();
            return Ok(());
        };

        if response.html_body.is_none() {
            self.status_line = "The last response does not look like HTML.".to_string();
            return Ok(());
        }

        let path = self.storage.data_dir().join("last-response.html");
        fs::write(&path, &response.body)
            .with_context(|| format!("failed to write {}", path.display()))?;
        open_path_in_browser(&path)?;
        self.status_line = format!("Opened HTML response: {}", path.display());
        Ok(())
    }

    pub fn clear_history_and_html_exports(&mut self) -> Result<()> {
        self.history.clear();
        self.storage.clear_history()?;
        let deleted_html = self.storage.delete_html_exports()?;
        self.refresh_sidebar();
        self.selected_sidebar = 0;
        self.modal = Some(Modal::Info(format!(
            "History cleared. Deleted {deleted_html} local HTML export file(s)."
        )));
        self.status_line = "History cleared, including local HTML response exports.".to_string();
        Ok(())
    }

    pub fn delete_selected_sidebar_entry(&mut self) -> Result<()> {
        let Some(entry) = self.sidebar_entries.get(self.selected_sidebar).cloned() else {
            return Ok(());
        };

        let message = match entry {
            SidebarEntry::Collection {
                label,
                collection_index: Some(index),
            } => {
                if index < self.collections.len() {
                    self.collections.remove(index);
                    self.storage.save_collections(&self.collections)?;
                    format!("Deleted collection: {label}")
                } else {
                    "Collection was already removed.".to_string()
                }
            }
            SidebarEntry::Folder {
                label,
                collection_index,
                folder_path,
            } => {
                if let Some(collection) = self.collections.get_mut(collection_index) {
                    let deleted = remove_folder(&mut collection.folders, &folder_path).is_some();
                    if deleted {
                        self.storage.save_collections(&self.collections)?;
                        format!("Deleted folder: {}", label.trim())
                    } else {
                        "Folder was already removed.".to_string()
                    }
                } else {
                    "Collection was already removed.".to_string()
                }
            }
            SidebarEntry::Request {
                label,
                request: _,
                source,
            } => {
                let deleted = match source {
                    RequestSource::CollectionRoot {
                        collection_index,
                        request_index,
                    } => self
                        .collections
                        .get_mut(collection_index)
                        .and_then(|collection| {
                            remove_index(&mut collection.requests, request_index)
                        }),
                    RequestSource::CollectionFolder {
                        collection_index,
                        folder_path,
                        request_index,
                    } => self
                        .collections
                        .get_mut(collection_index)
                        .and_then(|collection| {
                            get_folder_mut(&mut collection.folders, &folder_path)
                        })
                        .and_then(|folder| remove_index(&mut folder.requests, request_index)),
                };

                if deleted.is_some() {
                    self.storage.save_collections(&self.collections)?;
                    format!("Deleted request: {}", label.trim())
                } else {
                    "Request was already removed.".to_string()
                }
            }
            SidebarEntry::Saved { label, index, .. } => {
                if remove_index(&mut self.saved_requests, index).is_some() {
                    self.storage.save_saved_requests(&self.saved_requests)?;
                    format!("Deleted saved request: {}", label.trim())
                } else {
                    "Saved request was already removed.".to_string()
                }
            }
            SidebarEntry::History { label, index, .. } => {
                if remove_index(&mut self.history, index).is_some() {
                    self.storage.save_history(&self.history)?;
                    format!("Deleted history item: {}", label.trim())
                } else {
                    "History item was already removed.".to_string()
                }
            }
            SidebarEntry::Collection {
                collection_index: None,
                ..
            } => {
                self.status_line = "This sidebar section cannot be deleted.".to_string();
                return Ok(());
            }
        };

        self.selected_sidebar = self.selected_sidebar.saturating_sub(1);
        self.refresh_sidebar();
        self.modal = Some(Modal::Info(message.clone()));
        self.status_line = message;
        Ok(())
    }

    pub fn switch_panel(&mut self, forward: bool) {
        self.focus = match (self.focus, forward) {
            (Panel::Sidebar, true) => Panel::Request,
            (Panel::Request, true) => Panel::Response,
            (Panel::Response, true) => Panel::Sidebar,
            (Panel::Sidebar, false) => Panel::Response,
            (Panel::Request, false) => Panel::Sidebar,
            (Panel::Response, false) => Panel::Request,
        };
    }

    pub fn open_help(&mut self) {
        self.modal = Some(Modal::Help);
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
    }

    pub fn refresh_sidebar(&mut self) {
        let filter = match &self.modal {
            Some(Modal::Search) => self.input.value().to_ascii_lowercase(),
            _ => String::new(),
        };
        let mut entries = Vec::new();

        if !self.saved_requests.is_empty() {
            entries.push(SidebarEntry::Collection {
                label: "Saved requests".to_string(),
                collection_index: None,
            });
            for (index, item) in self.saved_requests.iter().enumerate() {
                push_if_matches(
                    &mut entries,
                    SidebarEntry::Saved {
                        label: item.name.clone(),
                        request: item.request.clone(),
                        index,
                    },
                    &filter,
                );
            }
        }

        for (collection_index, collection) in self.collections.iter().enumerate() {
            entries.push(SidebarEntry::Collection {
                label: collection.name.clone(),
                collection_index: Some(collection_index),
            });
            for (request_index, request) in collection.requests.iter().enumerate() {
                push_if_matches(
                    &mut entries,
                    SidebarEntry::Request {
                        label: request.name.clone(),
                        request: request.request.clone(),
                        source: RequestSource::CollectionRoot {
                            collection_index,
                            request_index,
                        },
                    },
                    &filter,
                );
            }
            for (folder_index, folder) in collection.folders.iter().enumerate() {
                flatten_folder(
                    &mut entries,
                    folder,
                    collection_index,
                    vec![folder_index],
                    1,
                    &filter,
                );
            }
        }

        if !self.history.is_empty() {
            entries.push(SidebarEntry::Collection {
                label: "History".to_string(),
                collection_index: None,
            });
            for (index, item) in self.history.iter().enumerate() {
                push_if_matches(
                    &mut entries,
                    SidebarEntry::History {
                        label: format!(
                            "{} {} {}",
                            item.method.as_str(),
                            item.status
                                .map(|status| status.to_string())
                                .unwrap_or_else(|| "ERR".to_string()),
                            item.url
                        ),
                        request: item.request.clone(),
                        index,
                    },
                    &filter,
                );
            }
        }

        if entries.is_empty() {
            entries.push(SidebarEntry::Collection {
                label: "No collections yet".to_string(),
                collection_index: None,
            });
        }

        self.sidebar_entries = entries;
        self.selected_sidebar = self
            .selected_sidebar
            .min(self.sidebar_entries.len().saturating_sub(1));
    }

    fn handle_modal_key<'a>(
        &'a mut self,
        key: KeyEvent,
    ) -> impl std::future::Future<Output = Result<()>> + 'a {
        async move {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.close_modal(),
                KeyCode::Char('n') if matches!(self.modal, Some(Modal::ConfirmNewRequest)) => {
                    self.close_modal()
                }
                KeyCode::Char('y') if matches!(self.modal, Some(Modal::ConfirmNewRequest)) => {
                    self.close_modal();
                    self.new_request();
                }
                KeyCode::Char('n') if matches!(self.modal, Some(Modal::ConfirmClearHistory)) => {
                    self.close_modal()
                }
                KeyCode::Char('y') if matches!(self.modal, Some(Modal::ConfirmClearHistory)) => {
                    if let Err(error) = self.clear_history_and_html_exports() {
                        self.modal = Some(Modal::Error(format!("{error:#}")));
                    }
                }
                KeyCode::Char('n')
                    if matches!(self.modal, Some(Modal::ConfirmDeleteSidebar(_))) =>
                {
                    self.close_modal()
                }
                KeyCode::Char('y')
                    if matches!(self.modal, Some(Modal::ConfirmDeleteSidebar(_))) =>
                {
                    if let Err(error) = self.delete_selected_sidebar_entry() {
                        self.modal = Some(Modal::Error(format!("{error:#}")));
                    }
                }
                KeyCode::Char('?') => self.open_help(),
                KeyCode::Enter => {
                    if matches!(self.modal, Some(Modal::Import)) {
                        self.edit_target = Some(EditTarget::ImportPath);
                        self.input.set_value("");
                    }
                }
                KeyCode::Char('/') if matches!(self.modal, Some(Modal::Search)) => {
                    self.edit_target = Some(EditTarget::Search);
                }
                KeyCode::Char('/') if matches!(self.modal, Some(Modal::ResponseSearch)) => {
                    self.edit_target = Some(EditTarget::ResponseSearch);
                }
                _ => {}
            }
            Ok(())
        }
    }

    async fn handle_edit_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.edit_target = None;
                if matches!(self.modal, Some(Modal::Search)) {
                    self.close_modal();
                    self.input.set_value("");
                    self.refresh_sidebar();
                } else if matches!(self.modal, Some(Modal::ResponseSearch)) {
                    self.close_modal();
                    self.input.set_value("");
                }
            }
            KeyCode::Enter => self.commit_edit().await?,
            _ => {
                if self.input.handle_key(key)
                    && matches!(self.edit_target, Some(EditTarget::Search))
                {
                    self.refresh_sidebar();
                }
            }
        }
        Ok(())
    }

    async fn commit_edit(&mut self) -> Result<()> {
        let Some(target) = self.edit_target.take() else {
            return Ok(());
        };
        let value = self.input.take();
        match target {
            EditTarget::Url => self.current_request.url = value,
            EditTarget::Headers => self.current_request.headers = parse_key_values(&value),
            EditTarget::Query => self.current_request.query_params = parse_query_values(&value),
            EditTarget::Body => {
                if value.trim().is_empty() {
                    self.current_request.body = BodyConfig::Empty;
                } else {
                    if serde_json::from_str::<serde_json::Value>(&value).is_err() {
                        self.status_line =
                            "Body is not valid JSON; it will still be sent as raw text."
                                .to_string();
                    }
                    self.current_request.body = BodyConfig::Raw {
                        content_type: "application/json".to_string(),
                        body: value,
                    };
                }
            }
            EditTarget::Bearer => {
                self.current_request.auth = if value.trim().is_empty() {
                    AuthConfig::None
                } else {
                    AuthConfig::Bearer { token: value }
                };
            }
            EditTarget::Basic => {
                let (username, password) = value.split_once(':').unwrap_or((&value, ""));
                self.current_request.auth = AuthConfig::Basic {
                    username: username.to_string(),
                    password: password.to_string(),
                };
            }
            EditTarget::ApiKey => {
                let (key, value) = value.split_once(':').unwrap_or((&value, ""));
                self.current_request.auth = AuthConfig::ApiKeyHeader {
                    key: key.trim().to_string(),
                    value: value.trim().to_string(),
                };
            }
            EditTarget::ImportPath => {
                self.modal = None;
                if let Err(error) = self.import_postman_collection(value.trim()) {
                    self.modal = Some(Modal::Error(format!("{error:#}")));
                }
            }
            EditTarget::Search => {
                self.modal = None;
                self.refresh_sidebar();
            }
            EditTarget::ResponseSearch => {
                self.response_search = value;
                self.modal = None;
                self.jump_to_response_search();
            }
        }
        Ok(())
    }

    fn start_edit_for_focus(&mut self) {
        match self.focus {
            Panel::Request => match self.request_field {
                RequestField::Method => {
                    self.current_request.method = self.current_request.method.next()
                }
                RequestField::Url => {
                    self.input.set_value(self.current_request.url.clone());
                    self.edit_target = Some(EditTarget::Url);
                }
                RequestField::Headers => {
                    self.input
                        .set_value(format_headers(&self.current_request.headers));
                    self.edit_target = Some(EditTarget::Headers);
                }
                RequestField::Query => {
                    self.input
                        .set_value(format_query(&self.current_request.query_params));
                    self.edit_target = Some(EditTarget::Query);
                }
                RequestField::Auth => {
                    self.input
                        .set_value(auth_to_edit_value(&self.current_request.auth));
                    self.edit_target = Some(match self.current_request.auth {
                        AuthConfig::None | AuthConfig::Bearer { .. } => EditTarget::Bearer,
                        AuthConfig::Basic { .. } => EditTarget::Basic,
                        AuthConfig::ApiKeyHeader { .. } => EditTarget::ApiKey,
                    });
                }
                RequestField::Body => {
                    let body = match &self.current_request.body {
                        BodyConfig::Empty => String::new(),
                        BodyConfig::Raw { body, .. } => body.clone(),
                    };
                    self.input.set_value(body);
                    self.edit_target = Some(EditTarget::Body);
                }
            },
            Panel::Sidebar => self.load_selected_sidebar(),
            Panel::Response => {
                self.response_tab = self.response_tab_next();
                self.response_scroll = 0;
            }
        }
    }

    fn open_import(&mut self) {
        self.input.set_value("");
        self.modal = Some(Modal::Import);
        self.edit_target = Some(EditTarget::ImportPath);
    }

    fn open_new_request(&mut self) {
        if self.current_request != RequestModel::default() || self.response.is_some() {
            self.modal = Some(Modal::ConfirmNewRequest);
        } else {
            self.new_request();
        }
    }

    fn open_search(&mut self) {
        self.input.set_value("");
        if self.focus == Panel::Response {
            self.modal = Some(Modal::ResponseSearch);
            self.edit_target = Some(EditTarget::ResponseSearch);
        } else {
            self.modal = Some(Modal::Search);
            self.edit_target = Some(EditTarget::Search);
            self.refresh_sidebar();
        }
    }

    fn open_delete_selected_sidebar(&mut self) {
        let Some(entry) = self.sidebar_entries.get(self.selected_sidebar) else {
            return;
        };

        if matches!(
            entry,
            SidebarEntry::Collection {
                collection_index: None,
                ..
            }
        ) {
            self.status_line =
                "Select a collection, folder, request, saved item, or history item to delete."
                    .to_string();
            return;
        }

        self.modal = Some(Modal::ConfirmDeleteSidebar(
            sidebar_label(entry).trim().to_string(),
        ));
    }

    fn move_horizontal(&mut self, forward: bool) {
        match self.focus {
            Panel::Request if self.request_field == RequestField::Method => {
                self.current_request.method = if forward {
                    self.current_request.method.next()
                } else {
                    self.current_request.method.previous()
                };
            }
            Panel::Request if self.request_field == RequestField::Auth => {
                self.current_request.auth = rotate_auth(&self.current_request.auth, forward);
            }
            Panel::Response => {
                self.response_tab = if forward {
                    self.response_tab_next()
                } else {
                    self.response_tab_prev()
                };
                self.response_scroll = 0;
            }
            _ => {}
        }
    }

    fn move_vertical(&mut self, down: bool) {
        match self.focus {
            Panel::Sidebar => {
                if down {
                    self.selected_sidebar = (self.selected_sidebar + 1)
                        .min(self.sidebar_entries.len().saturating_sub(1));
                } else {
                    self.selected_sidebar = self.selected_sidebar.saturating_sub(1);
                }
                self.load_selected_sidebar_if_request();
            }
            Panel::Request => self.request_field = next_request_field(self.request_field, down),
            Panel::Response => {
                if down {
                    self.scroll_response(1);
                } else {
                    self.scroll_response(-1);
                }
            }
        }
    }

    fn scroll_response(&mut self, delta: i16) {
        let limit = self.response_scroll_limit();
        if delta.is_negative() {
            self.response_scroll = self.response_scroll.saturating_sub(delta.unsigned_abs());
        } else {
            self.response_scroll = self.response_scroll.saturating_add(delta as u16).min(limit);
        }
    }

    fn response_scroll_limit(&self) -> u16 {
        self.response_text_for_current_tab()
            .lines()
            .count()
            .saturating_sub(1)
            .min(u16::MAX as usize) as u16
    }

    fn response_text_for_current_tab(&self) -> String {
        let Some(response) = &self.response else {
            return "Send a request to see the response here.".to_string();
        };

        match self.response_tab {
            ResponseTab::Pretty if self.config.response_pretty => response
                .pretty_body
                .clone()
                .unwrap_or_else(|| response.body.clone()),
            ResponseTab::Pretty => response.body.clone(),
            ResponseTab::JsonTree => response
                .json_tree_body
                .clone()
                .unwrap_or_else(|| "No JSON tree view for this response.".to_string()),
            ResponseTab::Raw => response.body.clone(),
            ResponseTab::Html => response
                .html_body
                .clone()
                .unwrap_or_else(|| "No HTML view for this response.".to_string()),
            ResponseTab::Headers => response
                .headers
                .iter()
                .map(|header| format!("{}: {}", header.key, header.value))
                .collect::<Vec<_>>()
                .join("\n"),
            ResponseTab::Error => response
                .error
                .clone()
                .unwrap_or_else(|| "No error for the last response.".to_string()),
        }
    }

    fn load_selected_sidebar_if_request(&mut self) {
        if let Some(
            SidebarEntry::Request { request, .. }
            | SidebarEntry::History { request, .. }
            | SidebarEntry::Saved { request, .. },
        ) = self.sidebar_entries.get(self.selected_sidebar)
        {
            self.current_request = request.clone();
        }
    }

    fn load_selected_sidebar(&mut self) {
        self.load_selected_sidebar_if_request();
        self.focus = Panel::Request;
    }

    fn response_tab_next(&self) -> ResponseTab {
        match self.response_tab {
            ResponseTab::Pretty => ResponseTab::JsonTree,
            ResponseTab::JsonTree => ResponseTab::Raw,
            ResponseTab::Raw => ResponseTab::Html,
            ResponseTab::Html => ResponseTab::Headers,
            ResponseTab::Headers => ResponseTab::Error,
            ResponseTab::Error => ResponseTab::Pretty,
        }
    }

    fn response_tab_prev(&self) -> ResponseTab {
        match self.response_tab {
            ResponseTab::Pretty => ResponseTab::Error,
            ResponseTab::JsonTree => ResponseTab::Pretty,
            ResponseTab::Raw => ResponseTab::JsonTree,
            ResponseTab::Html => ResponseTab::Raw,
            ResponseTab::Headers => ResponseTab::Html,
            ResponseTab::Error => ResponseTab::Headers,
        }
    }

    fn jump_to_response_search(&mut self) {
        if self.response_search.trim().is_empty() {
            self.response_scroll = 0;
            self.status_line = "Response search cleared.".to_string();
            return;
        }

        let query = self.response_search.to_ascii_lowercase();
        let text = self.response_text_for_current_tab();
        if let Some(index) = text
            .lines()
            .position(|line| line.to_ascii_lowercase().contains(&query))
        {
            self.response_scroll = index.min(u16::MAX as usize) as u16;
            self.status_line = format!("Found response search match on line {}.", index + 1);
        } else {
            self.status_line = format!("No response matches for '{}'.", self.response_search);
        }
    }
}

#[cfg(target_os = "linux")]
fn open_path_in_browser(path: &std::path::Path) -> Result<()> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .context("failed to launch xdg-open")?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_path_in_browser(path: &std::path::Path) -> Result<()> {
    Command::new("open")
        .arg(path)
        .spawn()
        .context("failed to launch open")?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_path_in_browser(path: &std::path::Path) -> Result<()> {
    Command::new("cmd")
        .arg("/C")
        .arg("start")
        .arg("")
        .arg(path)
        .spawn()
        .context("failed to launch the default browser")?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn open_path_in_browser(_path: &std::path::Path) -> Result<()> {
    anyhow::bail!("opening a browser is not supported on this platform yet")
}

fn next_request_field(field: RequestField, down: bool) -> RequestField {
    use RequestField::*;
    match (field, down) {
        (Method, true) => Url,
        (Url, true) => Headers,
        (Headers, true) => Query,
        (Query, true) => Auth,
        (Auth, true) => Body,
        (Body, true) => Method,
        (Method, false) => Body,
        (Url, false) => Method,
        (Headers, false) => Url,
        (Query, false) => Headers,
        (Auth, false) => Query,
        (Body, false) => Auth,
    }
}

fn rotate_auth(auth: &AuthConfig, forward: bool) -> AuthConfig {
    let idx = match auth {
        AuthConfig::None => 0,
        AuthConfig::Bearer { .. } => 1,
        AuthConfig::Basic { .. } => 2,
        AuthConfig::ApiKeyHeader { .. } => 3,
    };
    let next = if forward {
        (idx + 1) % 4
    } else {
        (idx + 3) % 4
    };
    match next {
        1 => AuthConfig::Bearer {
            token: String::new(),
        },
        2 => AuthConfig::Basic {
            username: String::new(),
            password: String::new(),
        },
        3 => AuthConfig::ApiKeyHeader {
            key: "x-api-key".to_string(),
            value: String::new(),
        },
        _ => AuthConfig::None,
    }
}

fn generate_curl(request: &RequestModel) -> String {
    let mut lines = vec![
        "curl".to_string(),
        format!("  -X {}", request.method.as_str()),
    ];

    for header in request.headers.iter().filter(|header| header.enabled) {
        if !header.key.trim().is_empty() {
            lines.push(format!(
                "  -H {}",
                shell_quote(&format!("{}: {}", header.key.trim(), header.value))
            ));
        }
    }

    match &request.auth {
        AuthConfig::None => {}
        AuthConfig::Bearer { token } if !token.trim().is_empty() => {
            lines.push(format!(
                "  -H {}",
                shell_quote(&format!("Authorization: Bearer {}", token.trim()))
            ));
        }
        AuthConfig::Basic { username, password } => {
            lines.push(format!(
                "  -u {}",
                shell_quote(&format!("{username}:{password}"))
            ));
        }
        AuthConfig::ApiKeyHeader { key, value } if !key.trim().is_empty() => {
            lines.push(format!(
                "  -H {}",
                shell_quote(&format!("{}: {}", key.trim(), value))
            ));
        }
        _ => {}
    }

    if let BodyConfig::Raw { content_type, body } = &request.body {
        if !body.is_empty() {
            if !content_type.trim().is_empty()
                && !request
                    .headers
                    .iter()
                    .any(|header| header.key.eq_ignore_ascii_case("content-type"))
            {
                lines.push(format!(
                    "  -H {}",
                    shell_quote(&format!("content-type: {}", content_type.trim()))
                ));
            }
            lines.push(format!("  --data {}", shell_quote(body)));
        }
    }

    lines.push(format!(
        "  {}",
        shell_quote(&request_url_with_query(request))
    ));
    lines.join(" \\\n")
}

fn request_url_with_query(request: &RequestModel) -> String {
    let Ok(mut url) = reqwest::Url::parse(request.url.trim()) else {
        return request.url.clone();
    };

    {
        let mut pairs = url.query_pairs_mut();
        for param in request.query_params.iter().filter(|param| param.enabled) {
            if !param.key.trim().is_empty() {
                pairs.append_pair(param.key.trim(), &param.value);
            }
        }
    }

    url.to_string()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn parse_key_values(input: &str) -> Vec<Header> {
    input
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some(Header {
                key: key.trim().to_string(),
                value: value.trim().to_string(),
                enabled: true,
            })
        })
        .collect()
}

fn parse_query_values(input: &str) -> Vec<QueryParam> {
    input
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some(QueryParam {
                key: key.trim().to_string(),
                value: value.trim().to_string(),
                enabled: true,
            })
        })
        .collect()
}

fn format_headers(headers: &[Header]) -> String {
    headers
        .iter()
        .filter(|header| header.enabled)
        .map(|header| format!("{}: {}", header.key, header.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_query(query: &[QueryParam]) -> String {
    query
        .iter()
        .filter(|param| param.enabled)
        .map(|param| format!("{}={}", param.key, param.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn auth_to_edit_value(auth: &AuthConfig) -> String {
    match auth {
        AuthConfig::None => String::new(),
        AuthConfig::Bearer { token } => token.clone(),
        AuthConfig::Basic { username, password } => format!("{username}:{password}"),
        AuthConfig::ApiKeyHeader { key, value } => format!("{key}: {value}"),
    }
}

fn auth_label(auth: &AuthConfig) -> &'static str {
    match auth {
        AuthConfig::None => "No Auth",
        AuthConfig::Bearer { .. } => "Bearer Token",
        AuthConfig::Basic { .. } => "Basic Auth",
        AuthConfig::ApiKeyHeader { .. } => "API Key Header",
    }
}

pub fn request_field_label(field: RequestField) -> &'static str {
    match field {
        RequestField::Method => "Method",
        RequestField::Url => "URL",
        RequestField::Headers => "Headers",
        RequestField::Query => "Query",
        RequestField::Auth => "Auth",
        RequestField::Body => "Body",
    }
}

pub fn summarize_auth(auth: &AuthConfig) -> String {
    match auth {
        AuthConfig::None => "No Auth".to_string(),
        _ => auth_label(auth).to_string(),
    }
}

fn flatten_folder(
    entries: &mut Vec<SidebarEntry>,
    folder: &Folder,
    collection_index: usize,
    folder_path: Vec<usize>,
    depth: usize,
    filter: &str,
) {
    entries.push(SidebarEntry::Folder {
        label: format!("{}{}", "  ".repeat(depth), folder.name),
        collection_index,
        folder_path: folder_path.clone(),
    });
    for (request_index, request) in folder.requests.iter().enumerate() {
        push_if_matches(
            entries,
            SidebarEntry::Request {
                label: format!("{}{}", "  ".repeat(depth + 1), request.name),
                request: request.request.clone(),
                source: RequestSource::CollectionFolder {
                    collection_index,
                    folder_path: folder_path.clone(),
                    request_index,
                },
            },
            filter,
        );
    }
    for (folder_index, folder) in folder.folders.iter().enumerate() {
        let mut child_path = folder_path.clone();
        child_path.push(folder_index);
        flatten_folder(
            entries,
            folder,
            collection_index,
            child_path,
            depth + 1,
            filter,
        );
    }
}

fn push_if_matches(entries: &mut Vec<SidebarEntry>, entry: SidebarEntry, filter: &str) {
    if filter.is_empty() || sidebar_label(&entry).to_ascii_lowercase().contains(filter) {
        entries.push(entry);
    }
}

pub fn sidebar_label(entry: &SidebarEntry) -> &str {
    match entry {
        SidebarEntry::Collection { label, .. }
        | SidebarEntry::Folder { label, .. }
        | SidebarEntry::Request { label, .. }
        | SidebarEntry::History { label, .. }
        | SidebarEntry::Saved { label, .. } => label,
    }
}

fn remove_index<T>(items: &mut Vec<T>, index: usize) -> Option<T> {
    if index < items.len() {
        Some(items.remove(index))
    } else {
        None
    }
}

fn get_folder_mut<'a>(folders: &'a mut [Folder], path: &[usize]) -> Option<&'a mut Folder> {
    let (first, rest) = path.split_first()?;
    let folder = folders.get_mut(*first)?;
    if rest.is_empty() {
        Some(folder)
    } else {
        get_folder_mut(&mut folder.folders, rest)
    }
}

fn remove_folder(folders: &mut Vec<Folder>, path: &[usize]) -> Option<Folder> {
    let (first, rest) = path.split_first()?;
    if rest.is_empty() {
        return remove_index(folders, *first);
    }
    let parent = folders.get_mut(*first)?;
    remove_folder(&mut parent.folders, rest)
}

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    secs.to_string()
}
