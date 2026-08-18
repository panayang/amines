use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use shared::protocol::UserStatsResponse;

const STORAGE_KEY_TOKEN: &str = "amine_auth_token";
const STORAGE_KEY_USERNAME: &str = "amine_auth_username";

#[derive(Clone, Copy)]
pub struct AuthState {
    pub token: ReadSignal<Option<String>>,
    pub set_token: WriteSignal<Option<String>>,
    pub username: ReadSignal<Option<String>>,
    pub set_username: WriteSignal<Option<String>>,
    pub stats: ReadSignal<Option<UserStatsResponse>>,
    pub set_stats: WriteSignal<Option<UserStatsResponse>>,
}

impl AuthState {
    pub fn new() -> Self {
        let initial_token = LocalStorage::get::<String>(STORAGE_KEY_TOKEN).ok();
        let initial_username = LocalStorage::get::<String>(STORAGE_KEY_USERNAME).ok();

        let (token, set_token) = signal(initial_token);
        let (username, set_username) = signal(initial_username);
        let (stats, set_stats) = signal(None::<UserStatsResponse>);

        let state = Self {
            token,
            set_token,
            username,
            set_username,
            stats,
            set_stats,
        };

        if token.get_untracked().is_some() {
            state.refresh_stats();
        }

        state
    }

    pub fn set_logged_in(&self, token: String, username: String) {
        let _ = LocalStorage::set(STORAGE_KEY_TOKEN, token.clone());
        let _ = LocalStorage::set(STORAGE_KEY_USERNAME, username.clone());
        self.set_token.set(Some(token));
        self.set_username.set(Some(username));
        self.refresh_stats();
    }

    pub fn logout(&self) {
        LocalStorage::delete(STORAGE_KEY_TOKEN);
        LocalStorage::delete(STORAGE_KEY_USERNAME);
        self.set_token.set(None);
        self.set_username.set(None);
        self.set_stats.set(None);
    }

    pub fn is_logged_in(&self) -> bool {
        self.token.get().is_some()
    }

    pub fn refresh_stats(&self) {
        let current_token = self.token.get_untracked();
        let set_stats = self.set_stats;

        if let Some(tok) = current_token {
            wasm_bindgen_futures::spawn_local(async move {
                let url = "/api/stats";
                let req = Request::get(url)
                    .header("Authorization", &format!("Bearer {tok}"))
                    .send()
                    .await;

                if let Ok(resp) = req {
                    if resp.status() == 200 {
                        if let Ok(data) = resp.json::<UserStatsResponse>().await {
                            set_stats.set(Some(data));
                        }
                    }
                }
            });
        }
    }
}
