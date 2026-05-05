use crate::state::AppState;
use crate::ui::{Popup, Tab};
use std::collections::VecDeque;

pub struct AppContext {
    pub state: AppState,
    pub ui: UiState,
    pub client: crate::client::ClashClient,
    pub config_manager: crate::config::ConfigManager,
    pub running: bool,
    pub connected: bool,
}

#[derive(Debug, Clone)]
pub struct UiState {
    pub current_tab: Tab,
    pub tab_index: usize,
    pub proxy_selected: usize,
    pub provider_selected: usize,
    pub connection_selected: usize,
    pub rule_selected: usize,
    pub log_scroll: usize,
    pub proxy_expanded: Vec<bool>,
    pub provider_expanded: Vec<bool>,
    pub search_query: String,
    pub popup: Popup,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub input_fields: Vec<String>,
    pub input_field_index: usize,
    pub input_cursor: usize,
    pub modes: Vec<String>,
    pub current_mode_index: usize,
    pub auto_update: bool,
    pub search_active: bool,
    pub help_active: bool,
    pub traffic_history: VecDeque<(u64, u64)>,
    pub proxy_sort_asc: bool,
    pub provider_sort_asc: bool,
    pub connection_sort_asc: bool,
    pub rule_sort_asc: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            current_tab: Tab::Overview,
            tab_index: 0,
            proxy_selected: 0,
            provider_selected: 0,
            connection_selected: 0,
            rule_selected: 0,
            log_scroll: 0,
            proxy_expanded: Vec::new(),
            provider_expanded: Vec::new(),
            search_query: String::new(),
            popup: Popup::None,
            error_message: None,
            success_message: None,
            input_fields: vec![String::new(), String::new(), String::from("86400")],
            input_field_index: 0,
            input_cursor: 0,
            modes: vec![
                "Global".to_string(),
                "Rule".to_string(),
                "Direct".to_string(),
                "Reject".to_string(),
            ],
            current_mode_index: 1,
            auto_update: true,
            search_active: false,
            help_active: false,
            traffic_history: VecDeque::with_capacity(60),
            proxy_sort_asc: true,
            provider_sort_asc: true,
            connection_sort_asc: true,
            rule_sort_asc: true,
        }
    }
}
