use crate::components::*;
use crate::state::*;
use leptos::ev::KeyboardEvent;
use leptos::prelude::*;

#[component]
pub fn App() -> impl IntoView {
    let i18n = I18nContext::new();
    let auth = AuthState::new();
    let game = GameState::new();

    let (is_auth_open, set_auth_open) = signal(false);
    let (show_stats_first, set_show_stats_first) = signal(false);
    let (is_custom_open, set_custom_open) = signal(false);

    // Global keyboard shortcuts (Q, E, R)
    let g_key = game;
    let on_keydown = move |e: KeyboardEvent| {
        let key = e.key();
        if key == "q" || key == "Q" {
            g_key.step_layer(-1);
        } else if key == "e" || key == "E" {
            g_key.step_layer(1);
        } else if (key == "r" || key == "R") && g_key.mode.get() == AppMode::SinglePlayer {
            let cfg = g_key.sp_config.get();
            g_key.reset_sp_game(cfg);
        }
    };

    let on_open_auth = Callback::new(move |()| {
        set_show_stats_first.set(false);
        set_auth_open.set(true);
    });

    let on_open_stats = Callback::new(move |()| {
        auth.refresh_stats();
        set_show_stats_first.set(true);
        set_auth_open.set(true);
    });

    let on_open_custom = Callback::new(move |()| {
        set_custom_open.set(true);
    });

    view! {
        <div class="app-container" on:keydown=on_keydown tabindex="0">
            // Dynamic Floating Nebula Gradient Orbs
            <div class="ambient-bg" aria-hidden="true">
                <div class="ambient-orb orb-1"></div>
                <div class="ambient-orb orb-2"></div>
                <div class="ambient-orb orb-3"></div>
                <div class="ambient-orb orb-4"></div>
                <div class="ambient-vignette"></div>
            </div>

            <Navbar
                i18n=i18n
                auth=auth
                game=game
                on_open_auth=on_open_auth
                on_open_stats=on_open_stats
            />

            <div class="global-notice">
                <span>"✨ "</span>
                <span>{move || i18n.tr("global_banner_text")}</span>
            </div>

            <main class="main-content">
                <section class="game-panel">
                    <Hud
                        i18n=i18n
                        auth=auth
                        game=game
                        on_open_custom=on_open_custom
                    />

                    <LayerNav
                        i18n=i18n
                        game=game
                    />

                    <BoardView
                        i18n=i18n
                        auth=auth
                        game=game
                    />
                </section>

                <aside class="side-panel">
                    {move || {
                        if game.mode.get() == AppMode::Multiplayer {
                            view! {
                                <Lobby
                                    i18n=i18n
                                    auth=auth
                                    game=game
                                    on_open_custom=on_open_custom
                                />
                                <Chat
                                    i18n=i18n
                                    game=game
                                />
                            }.into_any()
                        } else {
                            let (hint_text, set_hint_text) = signal(Option::<String>::None);
                            let g_solve = game;
                            let a_tok = auth.token;

                            view! {
                                <div class="side-card" style="border-color: rgba(167, 139, 250, 0.4); box-shadow: 0 4px 20px rgba(124, 58, 237, 0.2);">
                                    <div class="side-card-header" style="color: var(--accent-blue-violet);">
                                        <span>"🤖 " {move || i18n.tr("bot_manager_title")}</span>
                                    </div>
                                    <div style="font-size: 12px; color: var(--text-secondary); line-height: 1.5; display: flex; flex-direction: column; gap: 10px;">
                                        <p style="font-size: 11px; color: var(--text-muted);">
                                            {move || i18n.tr("bot_panel_desc")}
                                        </p>

                                        <div style="font-size: 11px; font-weight: 700; color: var(--accent-gold); margin-top: 2px;">
                                            "💡 AI Hint (Highlight & Focus):"
                                        </div>
                                        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 6px;">
                                            <button
                                                class="btn btn-sm"
                                                on:click={
                                                    let g = g_solve;
                                                    move |_| {
                                                        if let Some(res) = g.sp_ai_hint(shared::ai_solver::BotTier::Novice) {
                                                            set_hint_text.set(Some(format!("Pascal Hint: {res}")));
                                                        }
                                                    }
                                                }
                                            >
                                                "Pascal"
                                            </button>
                                            <button
                                                class="btn btn-sm"
                                                on:click={
                                                    let g = g_solve;
                                                    move |_| {
                                                        if let Some(res) = g.sp_ai_hint(shared::ai_solver::BotTier::Intermediate) {
                                                            set_hint_text.set(Some(format!("Boole Hint: {res}")));
                                                        }
                                                    }
                                                }
                                            >
                                                "Boole"
                                            </button>
                                            <button
                                                class="btn btn-sm"
                                                on:click={
                                                    let g = g_solve;
                                                    move |_| {
                                                        if let Some(res) = g.sp_ai_hint(shared::ai_solver::BotTier::Advanced) {
                                                            set_hint_text.set(Some(format!("Lovelace Hint: {res}")));
                                                        }
                                                    }
                                                }
                                            >
                                                "Lovelace"
                                            </button>
                                            <button
                                                class="btn btn-sm btn-accent"
                                                on:click={
                                                    let g = g_solve;
                                                    move |_| {
                                                        if let Some(res) = g.sp_ai_hint(shared::ai_solver::BotTier::Master) {
                                                            set_hint_text.set(Some(format!("Turing Hint: {res}")));
                                                        }
                                                    }
                                                }
                                            >
                                                "Turing"
                                            </button>
                                        </div>

                                        <div style="font-size: 11px; font-weight: 700; color: var(--accent-blue-violet); margin-top: 4px;">
                                            "🤖 AI Auto-Move (Step):"
                                        </div>
                                        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 6px;">
                                            <button
                                                class="btn btn-sm"
                                                on:click={
                                                    let g = g_solve;
                                                    let tok = a_tok;
                                                    move |_| {
                                                        if let Some(res) = g.sp_ai_step(shared::ai_solver::BotTier::Novice, tok.get_untracked()) {
                                                            set_hint_text.set(Some(format!("Pascal Move: {res}")));
                                                        }
                                                    }
                                                }
                                            >
                                                "Pascal (Novice)"
                                            </button>
                                            <button
                                                class="btn btn-sm"
                                                on:click={
                                                    let g = g_solve;
                                                    let tok = a_tok;
                                                    move |_| {
                                                        if let Some(res) = g.sp_ai_step(shared::ai_solver::BotTier::Intermediate, tok.get_untracked()) {
                                                            set_hint_text.set(Some(format!("Boole (Inter): {res}")));
                                                        }
                                                    }
                                                }
                                            >
                                                "Boole (Inter)"
                                            </button>
                                            <button
                                                class="btn btn-sm"
                                                on:click={
                                                    let g = g_solve;
                                                    let tok = a_tok;
                                                    move |_| {
                                                        if let Some(res) = g.sp_ai_step(shared::ai_solver::BotTier::Advanced, tok.get_untracked()) {
                                                            set_hint_text.set(Some(format!("Lovelace (Adv): {res}")));
                                                        }
                                                    }
                                                }
                                            >
                                                "Lovelace (Adv)"
                                            </button>
                                            <button
                                                class="btn btn-sm btn-primary"
                                                on:click={
                                                    let g = g_solve;
                                                    let tok = a_tok;
                                                    move |_| {
                                                        if let Some(res) = g.sp_ai_step(shared::ai_solver::BotTier::Master, tok.get_untracked()) {
                                                            set_hint_text.set(Some(format!("Turing (Master): {res}")));
                                                        }
                                                    }
                                                }
                                            >
                                                "Turing (Master)"
                                            </button>
                                        </div>

                                        {move || hint_text.get().map(|h| view! {
                                            <div style="padding: 6px 10px; background: rgba(124, 58, 237, 0.25); border: 1px solid var(--accent-blue-violet); border-radius: var(--radius-sm); font-size: 11px; color: var(--accent-gold); font-family: var(--font-mono);">
                                                "💡 " {h}
                                            </div>
                                        })}
                                    </div>
                                </div>

                                <div class="side-card">
                                    <div class="side-card-header">
                                        <span>"📐 " {move || i18n.tr("guide_title")}</span>
                                    </div>
                                    <div style="font-size: 12px; color: var(--text-secondary); line-height: 1.6; display: flex; flex-direction: column; gap: 8px;">
                                        <p>{move || i18n.tr("guide_item_1")}</p>
                                        <p>{move || i18n.tr("guide_item_2")}</p>
                                        <p>{move || i18n.tr("guide_item_3")}</p>
                                        <p>{move || i18n.tr("guide_item_4")}</p>
                                    </div>
                                </div>
                            }.into_any()
                        }
                    }}
                </aside>
            </main>

            <AuthModal
                i18n=i18n
                auth=auth
                game=game
                is_open=is_auth_open
                set_open=set_auth_open
                show_stats_first=show_stats_first
                set_show_stats_first=set_show_stats_first
            />

            <CustomModal
                i18n=i18n
                game=game
                is_open=is_custom_open
                set_open=set_custom_open
            />

            <GameOverModal
                i18n=i18n
                auth=auth
                game=game
            />

            <SpVictoryModal
                i18n=i18n
                game=game
            />
        </div>
    }
}
