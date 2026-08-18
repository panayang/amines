use crate::state::auth_state::AuthState;
use crate::state::game_state::GameState;
use crate::state::i18n_context::I18nContext;
use leptos::prelude::*;

#[component]
pub fn GameOverModal(i18n: I18nContext, auth: AuthState, game: GameState) -> impl IntoView {
    let data_sig = game.mp_game_over_data;
    let room_sig = game.mp_room;

    view! {
        {move || {
            let data = match data_sig.get() {
                Some(d) => d,
                None => return view! { <div></div> }.into_any(),
            };

            let room = room_sig.get();
            let is_host = room.as_ref().map(|r| {
                r.players.iter().any(|p| p.is_host && auth.username.get().map(|u| u == p.username).unwrap_or(false))
            }).unwrap_or(false);

            let is_won = data.is_board_cleared;
            let title = if is_won {
                i18n.tr("game_over_title_won")
            } else {
                i18n.tr("game_over_title_lost")
            };
            let subtitle = if is_won {
                i18n.tr("game_over_subtitle_won")
            } else {
                i18n.tr("game_over_subtitle_lost")
            };

            let g_start = game;
            let g_leave = game;
            let g_close = game;

            view! {
                <div class="modal-backdrop" on:click=move |_| g_close.set_mp_game_over_data.set(None)>
                    <div class=if is_won { "modal-card settlement-card won" } else { "modal-card settlement-card lost" } on:click=move |e| e.stop_propagation()>
                        <div class="settlement-header">
                            <div class="settlement-icon">
                                {if is_won { "🏆" } else { "💥" }}
                            </div>
                            <h2 class="settlement-title">{title}</h2>
                            <p class="settlement-subtitle">{subtitle}</p>
                        </div>

                        // Match stats row
                        <div class="settlement-stats-bar">
                            <div class="settlement-stat-item">
                                <span class="stat-lbl">{move || i18n.tr("game_over_time")}</span>
                                <span class="stat-val">{data.elapsed_seconds} "s"</span>
                            </div>
                            <div class="settlement-stat-item">
                                <span class="stat-lbl">{move || i18n.tr("game_over_revealed")}</span>
                                <span class="stat-val">{data.revealed_count} "/" {data.total_non_mines}</span>
                            </div>
                            <div class="settlement-stat-item">
                                <span class="stat-lbl">{move || i18n.tr("game_over_player")}</span>
                                <span class="stat-val">{data.player_rankings.len()}</span>
                            </div>
                        </div>

                        // Leaderboard list
                        <div class="settlement-leaderboard">
                            <div class="settlement-section-title">
                                <span>"📊 " {move || i18n.tr("game_over_leaderboard")}</span>
                            </div>

                            <div class="ranking-list">
                                {data.player_rankings.into_iter().map(|p| {
                                    let rank_badge = match p.rank {
                                        1 => "👑 1st",
                                        2 => "🥈 2nd",
                                        3 => "🥉 3rd",
                                        r => Box::leak(format!("#{r}").into_boxed_str()),
                                    };
                                    let is_first = p.rank == 1;

                                    view! {
                                        <div class=if is_first { "ranking-row champion" } else { "ranking-row" }>
                                            <div class="ranking-left">
                                                <span class=if is_first { "rank-tag first" } else { "rank-tag" }>
                                                    {rank_badge}
                                                </span>
                                                <span class="player-badge" style=format!("background-color: {};", p.color)></span>
                                                <span class="rank-name">{p.username}</span>
                                                <span class=if p.is_eliminated { "rank-status elim" } else { "rank-status alive" }>
                                                    {move || if p.is_eliminated { i18n.tr("game_over_status_eliminated") } else { i18n.tr("game_over_status_survived") }}
                                                </span>
                                            </div>
                                            <div class="ranking-right">
                                                <span class="rank-pts">{p.score}</span>
                                                <span class="rank-pts-lbl">" PTS"</span>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>

                        // Action footer
                        <div class="settlement-actions">
                            {if is_host {
                                view! {
                                    <button
                                        class="btn btn-primary"
                                        style="flex: 1;"
                                        on:click=move |_| g_start.mp_start_game()
                                    >
                                        "🔄 " {move || i18n.tr("game_over_play_again")}
                                    </button>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }}

                            <button
                                class="btn btn-sm"
                                style="flex: 1;"
                                on:click=move |_| g_close.set_mp_game_over_data.set(None)
                            >
                                "👁️ " {move || i18n.tr("game_over_close")}
                            </button>

                            <button
                                class="btn btn-sm btn-danger"
                                style="flex: 1;"
                                on:click=move |_| g_leave.mp_leave_room()
                            >
                                "🚪 " {move || i18n.tr("game_over_leave_room")}
                            </button>
                        </div>
                    </div>
                </div>
            }.into_any()
        }}
    }
}
