use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use shared::i18n::{t, Language};

const STORAGE_KEY_LANG: &str = "amine_lang_pref";

#[derive(Clone, Copy)]
pub struct I18nContext {
    pub lang: ReadSignal<Language>,
    pub set_lang: WriteSignal<Language>,
}

impl I18nContext {
    pub fn new() -> Self {
        let initial_lang = match LocalStorage::get::<String>(STORAGE_KEY_LANG) {
            Ok(code) => Language::from_code(&code),
            Err(_) => Language::En,
        };

        let (lang, set_lang) = signal(initial_lang);
        Self { lang, set_lang }
    }

    pub fn toggle_language(&self) {
        let next = self.lang.get().toggle();
        self.set_lang.set(next);
        let _ = LocalStorage::set(STORAGE_KEY_LANG, next.code().to_string());
    }

    pub fn tr(&self, key: &'static str) -> &'static str {
        let current_lang = self.lang.get();
        t(current_lang, key)
    }
}
