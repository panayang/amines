use crate::state::auth_state::AuthState;
use crate::state::game_state::{AppMode, GameState};
use crate::state::i18n_context::I18nContext;
use leptos::prelude::*;
use shared::board::{BoardConfig, Difficulty, GameStatus};

#[component]
pub fn Hud(
    i18n: I18nContext,
    auth: AuthState,
    game: GameState,
    on_open_custom: Callback<()>,
) -> impl IntoView {
    let mode = game.mode;

    let sp_config = game.sp_config;
    let sp_board = game.sp_board;
    let sp_time = game.sp_time;

    let time_formatted = move || {
        let secs = if mode.get() == AppMode::SinglePlayer {
            sp_time.get()
        } else {
            game.mp_room.get().map(|r| r.elapsed_seconds).unwrap_or(0)
        };
        let m = secs / 60;
        let s = secs % 60;
        format!("{:02}:{:02}", m, s)
    };

    let remaining_mines = move || {
        if mode.get() == AppMode::SinglePlayer {
            let board = sp_board.get();
            let total = board.config.mines;
            let flagged = board.flag_count;
            (total as i64) - (flagged as i64)
        } else {
            game.mp_room
                .get()
                .map(|r| {
                    let total = r.config.mines;
                    let flagged = r.cells.iter().filter(|c| c.is_flagged).count();
                    (total as i64) - (flagged as i64)
                })
                .unwrap_or(0)
        }
    };

    let current_pb_str = move || {
        let diff = sp_config.get().difficulty;
        if let Some(stats) = auth.stats.get() {
            let pb_ms = match diff {
                Difficulty::Easy => stats.easy_pb_ms,
                Difficulty::Medium => stats.medium_pb_ms,
                Difficulty::Expert => stats.expert_pb_ms,
                Difficulty::Custom => None,
            };
            if let Some(ms) = pb_ms {
                let s = ms / 1000;
                let m = s / 60;
                let sec = s % 60;
                return format!("{:02}:{:02}", m, sec);
            }
        }
        if let Some(rec) = game.sp_pb_records.get().get_pb(diff) {
            let s = rec.time_secs;
            let m = s / 60;
            let sec = s % 60;
            return format!("{:02}:{:02}", m, sec);
        }
        i18n.tr("hud_no_pb").to_string()
    };

    let status_view = move || {
        let status = if mode.get() == AppMode::SinglePlayer {
            sp_board.get().status
        } else {
            game.mp_room
                .get()
                .map(|r| r.status)
                .unwrap_or(GameStatus::Waiting)
        };

        match status {
            GameStatus::Waiting => {
                view! { <div class="status-banner">{move || i18n.tr("status_ready")}</div> }
                    .into_any()
            }
            GameStatus::Playing => {
                view! { <div class="status-banner">{move || i18n.tr("status_playing")}</div> }
                    .into_any()
            }
            GameStatus::Won => {
                view! { <div class="status-banner won">{move || i18n.tr("status_won")}</div> }
                    .into_any()
            }
            GameStatus::Lost => {
                view! { <div class="status-banner lost">{move || i18n.tr("status_lost")}</div> }
                    .into_any()
            }
        }
    };

    let game_clone = game;

    view! {
        <div class="hud-bar">
            {move || {
                if mode.get() == AppMode::SinglePlayer {
                    let curr_diff = sp_config.get().difficulty;
                    let g1 = game_clone;
                    let g2 = game_clone;
                    let g3 = game_clone;

                    view! {
                        <div class="diff-selector">
                            <button
                                class=if curr_diff == Difficulty::Easy { "diff-btn active" } else { "diff-btn" }
                                on:click=move |_| g1.reset_sp_game(BoardConfig::easy())
                            >
                                {move || i18n.tr("diff_easy")}
                            </button>
                            <button
                                class=if curr_diff == Difficulty::Medium { "diff-btn active" } else { "diff-btn" }
                                on:click=move |_| g2.reset_sp_game(BoardConfig::medium())
                            >
                                {move || i18n.tr("diff_medium")}
                            </button>
                            <button
                                class=if curr_diff == Difficulty::Expert { "diff-btn active" } else { "diff-btn" }
                                on:click=move |_| g3.reset_sp_game(BoardConfig::expert())
                            >
                                {move || i18n.tr("diff_expert")}
                            </button>
                            <button
                                class=if curr_diff == Difficulty::Custom { "diff-btn active" } else { "diff-btn" }
                                on:click=move |_| on_open_custom.run(())
                            >
                                {move || i18n.tr("diff_custom")}
                            </button>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div style="font-size: 13px; font-weight: 700; color: var(--primary);">
                            "⚡ " {move || i18n.tr("nav_multi")}
                        </div>
                    }.into_any()
                }
            }}

            <div class="hud-counters">
                <div class="hud-stat danger" title=move || i18n.tr("hud_mines")>
                    "💣 " {move || remaining_mines()}
                </div>

                <div class="hud-stat" title=move || i18n.tr("hud_time")>
                    "⏱ " {move || time_formatted()}
                </div>

                {move || {
                    if mode.get() == AppMode::SinglePlayer {
                        view! {
                            <div class="hud-stat gold" title=move || i18n.tr("hud_pb")>
                                <div class="hud-pb-badge">
                                    <span>{move || i18n.tr("hud_pb")}</span>
                                    <span class="hud-pb-val">{current_pb_str()}</span>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}
            </div>

                <div class="hud-tool-group" style="display: flex; gap: 6px; align-items: center;">
                    <button
                        class=move || {
                            if game_clone.is_flag_mode.get() {
                                "btn btn-sm btn-danger active"
                            } else {
                                "btn btn-sm btn-secondary"
                            }
                        }
                        title="Toggle Dig / Flag Mode (Shortcut: F)"
                        on:click=move |_| {
                            game_clone.set_is_flag_mode.update(|f| *f = !*f);
                        }
                    >
                        {move || {
                            if game_clone.is_flag_mode.get() {
                                i18n.tr("tool_flag")
                            } else {
                                i18n.tr("tool_dig")
                            }
                        }}
                    </button>

                    {move || {
                        if mode.get() == AppMode::SinglePlayer {
                            let g = game_clone;
                            view! {
                                <button
                                    class=move || {
                                        if g.sp_is_paused.get() {
                                            "btn btn-sm btn-accent active"
                                        } else {
                                            "btn btn-sm btn-secondary"
                                        }
                                    }
                                    title="Pause / Resume Timer (Shortcut: P)"
                                    on:click=move |_| {
                                        g.sp_toggle_pause();
                                    }
                                >
                                    {move || {
                                        if g.sp_is_paused.get() {
                                            format!("▶️ {} (P)", i18n.tr("hud_resume"))
                                        } else {
                                            format!("⏸️ {} (P)", i18n.tr("hud_pause"))
                                        }
                                    }}
                                </button>
                                <button
                                    class="btn btn-sm btn-primary"
                                    on:click=move |_| {
                                        let cfg = g.sp_config.get();
                                        g.reset_sp_game(cfg);
                                    }
                                >
                                    "🔄 " {move || i18n.tr("hud_restart")} " (R)"
                                </button>
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }
                    }}
                </div>
        </div>

        {status_view}

        <div class="controls-hint">
            {move || i18n.tr("guide_controls")}
        </div>
    }
}
