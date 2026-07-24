use std::collections::HashMap;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::domain::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Details,
    Credits,
    Accounts,
    Filters,
    Help,
}

#[derive(Debug, Clone, Default)]
pub struct FilterOptions {
    pub provider_filter: Option<String>,
    pub account_search: String,
    pub min_usage_percent: Option<f64>,
}

#[derive(Debug)]
pub struct ApplicationState {
    pub current_screen: Screen,
    pub previous_screen: Screen,
    pub selected_account_index: usize,
    pub accounts: Vec<Account>,
    pub snapshots: HashMap<AccountId, UsageSnapshot>,
    pub filters: FilterOptions,
    pub should_quit: bool,
    pub is_refreshing: bool,
    pub status_message: Option<String>,
    pub search_input_active: bool,
}

impl ApplicationState {
    pub fn new(accounts: Vec<Account>, snapshots: Vec<UsageSnapshot>) -> Self {
        let snap_map = snapshots
            .into_iter()
            .map(|s| (s.account_id.clone(), s))
            .collect();

        Self {
            current_screen: Screen::Dashboard,
            previous_screen: Screen::Dashboard,
            selected_account_index: 0,
            accounts,
            snapshots: snap_map,
            filters: FilterOptions::default(),
            should_quit: false,
            is_refreshing: false,
            status_message: None,
            search_input_active: false,
        }
    }

    pub fn filtered_accounts(&self) -> Vec<&Account> {
        self.accounts
            .iter()
            .filter(|acc| {
                if let Some(ref prov) = self.filters.provider_filter {
                    if !acc.provider.to_lowercase().contains(&prov.to_lowercase()) {
                        return false;
                    }
                }

                if !self.filters.account_search.is_empty() {
                    let search = self.filters.account_search.to_lowercase();
                    let identity_match = acc.identity.to_lowercase().contains(&search);
                    let alias_match = acc
                        .alias
                        .as_ref()
                        .map(|a| a.to_lowercase().contains(&search))
                        .unwrap_or(false);
                    let id_match = acc.id.to_lowercase().contains(&search);
                    if !identity_match && !alias_match && !id_match {
                        return false;
                    }
                }

                if let Some(min_pct) = self.filters.min_usage_percent {
                    let used = self
                        .snapshots
                        .get(&acc.id)
                        .and_then(|s| s.effective_used_percent)
                        .unwrap_or(0.0);
                    if used < min_pct {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    pub fn selected_account(&self) -> Option<&Account> {
        let filtered = self.filtered_accounts();
        if filtered.is_empty() {
            None
        } else {
            let idx = self.selected_account_index.min(filtered.len() - 1);
            Some(filtered[idx])
        }
    }

    pub fn selected_snapshot(&self) -> Option<&UsageSnapshot> {
        self.selected_account().and_then(|acc| self.snapshots.get(&acc.id))
    }

    pub fn next_account(&mut self) {
        let len = self.filtered_accounts().len();
        if len > 0 {
            self.selected_account_index = (self.selected_account_index + 1) % len;
        }
    }

    pub fn previous_account(&mut self) {
        let len = self.filtered_accounts().len();
        if len > 0 {
            if self.selected_account_index == 0 {
                self.selected_account_index = len - 1;
            } else {
                self.selected_account_index -= 1;
            }
        }
    }

    pub fn switch_screen(&mut self, screen: Screen) {
        if self.current_screen != screen {
            self.previous_screen = self.current_screen;
            self.current_screen = screen;
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if self.search_input_active {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.search_input_active = false;
                }
                KeyCode::Backspace => {
                    self.filters.account_search.pop();
                }
                KeyCode::Char(c) => {
                    self.filters.account_search.push(c);
                }
                _ => {}
            }
            return;
        }

        // Global shortcuts (when search input not active)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.next_account();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.previous_account();
            }
            KeyCode::Enter | KeyCode::Char('d') => {
                if self.current_screen == Screen::Help || self.current_screen == Screen::Filters {
                    self.switch_screen(self.previous_screen);
                } else if self.current_screen == Screen::Dashboard {
                    self.switch_screen(Screen::Details);
                } else {
                    self.switch_screen(Screen::Details);
                }
            }
            KeyCode::Esc => {
                if self.current_screen == Screen::Help || self.current_screen == Screen::Filters {
                    self.switch_screen(self.previous_screen);
                } else if self.current_screen != Screen::Dashboard {
                    self.switch_screen(Screen::Dashboard);
                } else {
                    self.filters.account_search.clear();
                    self.filters.provider_filter = None;
                    self.filters.min_usage_percent = None;
                }
            }
            KeyCode::Tab => {
                let next = match self.current_screen {
                    Screen::Dashboard => Screen::Details,
                    Screen::Details => Screen::Credits,
                    Screen::Credits => Screen::Accounts,
                    Screen::Accounts => Screen::Filters,
                    Screen::Filters => Screen::Help,
                    Screen::Help => Screen::Dashboard,
                };
                self.switch_screen(next);
            }
            KeyCode::Char('a') => {
                self.switch_screen(Screen::Accounts);
            }
            KeyCode::Char('c') => {
                self.switch_screen(Screen::Credits);
            }
            KeyCode::Char('f') => {
                self.switch_screen(Screen::Filters);
            }
            KeyCode::Char('/') => {
                self.search_input_active = true;
            }
            KeyCode::Char('?') | KeyCode::Char('h') => {
                if self.current_screen == Screen::Help {
                    self.switch_screen(self.previous_screen);
                } else {
                    self.switch_screen(Screen::Help);
                }
            }
            KeyCode::Char('1') => self.switch_screen(Screen::Dashboard),
            KeyCode::Char('2') => self.switch_screen(Screen::Details),
            KeyCode::Char('3') => self.switch_screen(Screen::Credits),
            KeyCode::Char('4') => self.switch_screen(Screen::Accounts),
            KeyCode::Char('5') => self.switch_screen(Screen::Filters),
            _ => {}
        }
    }
}
