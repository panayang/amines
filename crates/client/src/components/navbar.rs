use crate::state::auth_state::AuthState;
use crate::state::game_state::{AppMode, GameState};
use crate::state::i18n_context::I18nContext;
use leptos::prelude::*;

#[component]
pub fn Navbar(
    i18n: I18nContext,
    auth: AuthState,
    game: GameState,
    on_open_auth: Callback<()>,
    on_open_stats: Callback<()>,
) -> impl IntoView {
    let current_mode = game.mode;

    view! {
        <header class="navbar">
            <div class="brand" on:click=move |_| game.set_app_mode(AppMode::SinglePlayer)>
                <div class="brand-icon">"∞"</div>
                <div class="brand-text">
                    <h1>{move || i18n.tr("app_title")}</h1>
                    <p>{move || i18n.tr("app_subtitle")}</p>
                </div>
            </div>

            <nav class="nav-center">
                <button
                    class=move || if current_mode.get() == AppMode::SinglePlayer { "nav-tab active" } else { "nav-tab" }
                    on:click=move |_| game.set_app_mode(AppMode::SinglePlayer)
                >
                    {move || i18n.tr("nav_single")}
                </button>
                <button
                    class=move || if current_mode.get() == AppMode::Multiplayer { "nav-tab active" } else { "nav-tab" }
                    on:click=move |_| game.set_app_mode(AppMode::Multiplayer)
                >
                    {move || i18n.tr("nav_multi")}
                    {move || {
                        if current_mode.get() == AppMode::Multiplayer {
                            if game.mp_connected.get() {
                                view! { <span style="font-size: 10px; color: #4ade80; margin-left: 4px;">"●"</span> }.into_any()
                            } else {
                                view! { <span style="font-size: 10px; color: #fbbf24; margin-left: 4px;">"○"</span> }.into_any()
                            }
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
                </button>
            </nav>

            <div class="nav-right">
                <button class="btn btn-sm" on:click=move |_| on_open_stats.run(())>
                    "🏆 " {move || if i18n.is_zh() { "纪录" } else { "Records" }}
                </button>

                <button class="btn btn-sm" on:click=move |_| i18n.toggle_language()>
                    "🌐 " {move || if i18n.lang.get() == shared::Language::En { "中文" } else { "English" }}
                </button>

                {move || {
                    if let Some(user) = auth.username.get() {
                        view! {
                            <button class="btn btn-sm btn-accent" on:click=move |_| on_open_stats.run(())>
                                "🏆 " {user}
                            </button>
                            <button class="btn btn-sm" on:click=move |_| auth.logout()>
                                {move || i18n.tr("nav_logout")}
                            </button>
                        }.into_any()
                    } else {
                        view! {
                            <button class="btn btn-sm btn-primary" on:click=move |_| on_open_auth.run(())>
                                "👤 " {move || i18n.tr("nav_login")}
                            </button>
                        }.into_any()
                    }
                }}
            </div>
        </header>
    }
}
