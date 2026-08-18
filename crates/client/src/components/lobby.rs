use crate::state::auth_state::AuthState;
use crate::state::game_state::GameState;
use crate::state::i18n_context::I18nContext;
use gloo_net::http::Request;
use leptos::prelude::*;
use shared::ai_solver::BotTier;
use shared::board::{BoardConfig, Difficulty, GameStatus};
use shared::protocol::RoomSummary;

#[component]
pub fn Lobby(
    i18n: I18nContext,
    auth: AuthState,
    game: GameState,
    on_open_custom: Callback<()>,
) -> impl IntoView {
    let (room_name, set_room_name) = signal("Möbius Arena".to_string());
    let (join_code, set_join_code) = signal("".to_string());
    let (diff_select, set_diff_select) = signal(Difficulty::Easy);
    let (public_rooms, set_public_rooms) = signal(Vec::<RoomSummary>::new());
    let (copied, set_copied) = signal(false);

    let refresh_rooms = move || {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(resp) = Request::get("/api/rooms").send().await {
                if resp.status() == 200 {
                    if let Ok(rooms) = resp.json::<Vec<RoomSummary>>().await {
                        set_public_rooms.set(rooms);
                    }
                }
            }
        });
    };

    // Initial load
    refresh_rooms();

    let on_create_room = {
        let g = game;
        move |_| {
            let name = room_name.get();
            let config = match diff_select.get() {
                Difficulty::Easy => BoardConfig::easy(),
                Difficulty::Medium => BoardConfig::medium(),
                Difficulty::Expert => BoardConfig::expert(),
                Difficulty::Custom => g.custom_config.get(),
            };
            let username = auth
                .username
                .get()
                .unwrap_or_else(|| format!("Player_{}", &uuid::Uuid::new_v4().to_string()[..4]));
            let token = auth.token.get();

            g.mp_create_room(name, config, username, token);
        }
    };

    let on_join_room = {
        let g = game;
        move |_| {
            let code = join_code.get().trim().to_uppercase();
            if code.is_empty() {
                return;
            }
            let username = auth
                .username
                .get()
                .unwrap_or_else(|| format!("Player_{}", &uuid::Uuid::new_v4().to_string()[..4]));
            let token = auth.token.get();

            g.mp_join_room(code, username, token);
        }
    };

    view! {
        <div class="side-card">
            {move || {
                if let Some(room) = game.mp_room.get() {
                    // In-room view
                    let room_code = room.room_id.clone();
                    let is_host = room.players.iter().any(|p| p.is_host && auth.username.get().map(|u| u == p.username).unwrap_or(false));
                    let is_ready = room.players.iter().any(|p| p.is_ready && auth.username.get().map(|u| u == p.username).unwrap_or(false));
                    let is_playing = room.status == GameStatus::Playing;
                    let current_bot_speed = room.bot_speed_ms;

                    let g_start = game;
                    let g_ready = game;
                    let g_leave = game;
                    let g_bot_add = game;
                    let g_bot_speed = game;

                    view! {
                        <div class="side-card-header">
                            <div>
                                <span style="color: var(--text-muted); font-size: 11px;">{move || i18n.tr("hud_room_code")} ": "</span>
                                <span style="font-family: var(--font-mono); color: var(--primary-light); font-size: 16px; font-weight: 800;">{room_code.clone()}</span>
                            </div>
                            <button
                                class="btn btn-sm"
                                on:click=move |_| {
                                    if let Some(win) = web_sys::window() {
                                        let _ = win.navigator().clipboard().write_text(&room_code);
                                        set_copied.set(true);
                                    }
                                }
                            >
                                {move || if copied.get() { i18n.tr("hud_copied") } else { i18n.tr("hud_copy_code") }}
                            </button>
                        </div>

                        // Player List
                        <div class="scoreboard-list">
                            <div style="font-size: 12px; font-weight: 700; color: var(--text-secondary); margin-bottom: 4px; display: flex; justify-content: space-between;">
                                <span>{move || i18n.tr("lobby_players")} " (" {room.players.len()} ")"</span>
                            </div>
                            {room.players.into_iter().map(|player| {
                                let p_id = player.id.clone();
                                let is_bot = player.is_bot;
                                let g_del = game;

                                view! {
                                    <div class=if player.is_eliminated { "player-score-row eliminated" } else { "player-score-row" }>
                                        <div class="player-tag">
                                            <span class="player-badge" style=format!("background-color: {};", player.color)></span>
                                            <span>{player.username}</span>
                                            {if player.is_host {
                                                view! { <span style="font-size: 10px; color: var(--accent-gold);">"👑"</span> }.into_any()
                                            } else if is_bot {
                                                view! { <span style="font-size: 10px; background: rgba(129, 140, 248, 0.25); color: var(--accent-blue-violet); padding: 1px 5px; border-radius: 4px; font-weight: 800;">"🤖 BOT"</span> }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                        </div>
                                        <div style="display: flex; align-items: center; gap: 6px;">
                                            {if is_playing {
                                                view! {
                                                    <span class="player-pts">
                                                        {player.score} " " {move || i18n.tr("hud_score")}
                                                    </span>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <span style=if player.is_ready { "color: var(--success); font-size: 11px; font-weight: 700;" } else { "color: var(--text-muted); font-size: 11px;" }>
                                                        {move || if player.is_ready { i18n.tr("lobby_status_ready") } else { i18n.tr("lobby_status_waiting") }}
                                                    </span>
                                                }.into_any()
                                            }}

                                            // Kick bot button for host
                                            {if is_host && is_bot && !is_playing {
                                                view! {
                                                    <button
                                                        class="btn btn-sm btn-danger"
                                                        style="padding: 1px 6px; font-size: 10px;"
                                                        on:click=move |_| g_del.mp_remove_bot(p_id.clone())
                                                    >
                                                        "✕"
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }}
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>

                        // AI NPC Manager Card (For Room Host)
                        {if is_host && !is_playing {
                            let g_add_1 = g_bot_add;
                            let g_add_2 = g_bot_add;
                            let g_add_3 = g_bot_add;
                            let g_add_4 = g_bot_add;

                            view! {
                                <div class="bot-manager-panel" style="background: rgba(8, 3, 20, 0.85); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); padding: 10px; display: flex; flex-direction: column; gap: 8px;">
                                    <div style="font-size: 12px; font-weight: 800; color: var(--accent-blue-violet); display: flex; align-items: center; justify-content: space-between;">
                                        <span>"🤖 " {move || i18n.tr("bot_manager_title")}</span>
                                        <span style="font-size: 11px; color: var(--text-muted); font-family: var(--font-mono);">{current_bot_speed} " ms"</span>
                                    </div>

                                    // Bot Speed Slider
                                    <div style="display: flex; flex-direction: column; gap: 4px;">
                                        <div style="display: flex; justify-content: space-between; font-size: 10px; color: var(--text-muted);">
                                            <span>{move || i18n.tr("bot_speed_label")}</span>
                                            <span>{move || i18n.tr("bot_speed_hint")}</span>
                                        </div>
                                        <input
                                            type="range"
                                            min="200"
                                            max="20000"
                                            step="200"
                                            prop:value=move || current_bot_speed.to_string()
                                            on:input=move |e| {
                                                if let Ok(spd) = event_target_value(&e).parse::<u64>() {
                                                    g_bot_speed.mp_update_bot_speed(spd);
                                                }
                                            }
                                            style="width: 100%; cursor: pointer;"
                                        />
                                    </div>

                                    // Add Bot Buttons (4 Tiers)
                                    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 6px;">
                                        <button
                                            class="btn btn-sm"
                                            style="font-size: 11px; padding: 4px 6px;"
                                            on:click=move |_| g_add_1.mp_add_bot(BotTier::Novice, Some(current_bot_speed))
                                        >
                                            {move || i18n.tr("bot_add_novice")}
                                        </button>
                                        <button
                                            class="btn btn-sm"
                                            style="font-size: 11px; padding: 4px 6px;"
                                            on:click=move |_| g_add_2.mp_add_bot(BotTier::Intermediate, Some(current_bot_speed))
                                        >
                                            {move || i18n.tr("bot_add_inter")}
                                        </button>
                                        <button
                                            class="btn btn-sm"
                                            style="font-size: 11px; padding: 4px 6px;"
                                            on:click=move |_| g_add_3.mp_add_bot(BotTier::Advanced, Some(current_bot_speed))
                                        >
                                            {move || i18n.tr("bot_add_adv")}
                                        </button>
                                        <button
                                            class="btn btn-sm btn-primary"
                                            style="font-size: 11px; padding: 4px 6px;"
                                            on:click=move |_| g_add_4.mp_add_bot(BotTier::Master, Some(current_bot_speed))
                                        >
                                            {move || i18n.tr("bot_add_master")}
                                        </button>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }}

                        <div style="display: flex; flex-direction: column; gap: 8px; margin-top: 4px;">
                            {if !is_playing {
                                view! {
                                    <div style="display: flex; gap: 6px;">
                                        <button
                                            class=if is_ready { "btn btn-primary" } else { "btn" }
                                            style="flex: 1;"
                                            on:click=move |_| g_ready.mp_set_ready(!is_ready)
                                        >
                                            {move || if is_ready { i18n.tr("lobby_unready") } else { i18n.tr("lobby_ready") }}
                                        </button>
                                        <button
                                            class="btn btn-accent"
                                            style="flex: 1;"
                                            on:click=move |_| g_start.mp_start_game()
                                        >
                                            "▶ " {move || i18n.tr("lobby_start")}
                                        </button>
                                    </div>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }}

                            <button
                                class="btn btn-sm btn-danger"
                                on:click=move |_| g_leave.mp_leave_room()
                            >
                                "🚪 " {move || i18n.tr("game_over_leave_room")}
                            </button>
                        </div>
                    }.into_any()
                } else {
                    // Lobby overview
                    view! {
                        <div class="side-card-header">
                            <span>{move || i18n.tr("lobby_title")}</span>
                            <button class="btn btn-sm" on:click=move |_| refresh_rooms()>"🔄"</button>
                        </div>

                        // Create room
                        <div class="form-group">
                            <label class="form-label">{move || i18n.tr("lobby_room_name")}</label>
                            <input
                                class="form-input"
                                type="text"
                                prop:value=move || room_name.get()
                                on:input=move |e| set_room_name.set(event_target_value(&e))
                            />
                        </div>

                        <div class="diff-selector" style="margin: 4px 0;">
                            <button
                                class=move || if diff_select.get() == Difficulty::Easy { "diff-btn active" } else { "diff-btn" }
                                on:click=move |_| set_diff_select.set(Difficulty::Easy)
                            >
                                {move || i18n.tr("diff_easy")}
                            </button>
                            <button
                                class=move || if diff_select.get() == Difficulty::Medium { "diff-btn active" } else { "diff-btn" }
                                on:click=move |_| set_diff_select.set(Difficulty::Medium)
                            >
                                {move || i18n.tr("diff_medium")}
                            </button>
                            <button
                                class=move || if diff_select.get() == Difficulty::Expert { "diff-btn active" } else { "diff-btn" }
                                on:click=move |_| set_diff_select.set(Difficulty::Expert)
                            >
                                {move || i18n.tr("diff_expert")}
                            </button>
                            <button
                                class=move || if diff_select.get() == Difficulty::Custom { "diff-btn active" } else { "diff-btn" }
                                on:click=move |_| {
                                    set_diff_select.set(Difficulty::Custom);
                                    on_open_custom.run(());
                                }
                            >
                                {move || i18n.tr("diff_custom")}
                            </button>
                        </div>

                        {move || {
                            if diff_select.get() == Difficulty::Custom {
                                let cfg = game.custom_config.get();
                                view! {
                                    <div style="font-size: 11px; color: var(--primary-light); background: rgba(139, 92, 246, 0.15); padding: 4px 8px; border-radius: 4px; display: flex; justify-content: space-between; align-items: center;">
                                        <span>{move || i18n.tr("diff_custom")} ": " {cfg.width} "×" {cfg.height} "×" {cfg.depth} " (" {cfg.mines} " 💣)"</span>
                                        <button class="btn btn-sm" style="padding: 1px 6px; font-size: 10px;" on:click=move |_| on_open_custom.run(())>{move || i18n.tr("btn_edit")}</button>
                                    </div>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }
                        }}

                        <button class="btn btn-primary" on:click=on_create_room>
                            "➕ " {move || i18n.tr("lobby_create_room")}
                        </button>

                        // Join room
                        <div style="border-top: 1px solid var(--border-color); padding-top: 8px; margin-top: 4px;">
                            <label class="form-label">{move || i18n.tr("lobby_join_room")}</label>
                            <div style="display: flex; gap: 6px; margin-top: 4px;">
                                <input
                                    class="form-input"
                                    style="flex: 1; text-transform: uppercase;"
                                    placeholder=move || i18n.tr("lobby_enter_code")
                                    prop:value=move || join_code.get()
                                    on:input=move |e| set_join_code.set(event_target_value(&e))
                                />
                                <button class="btn btn-accent" on:click=on_join_room>
                                    "➜"
                                </button>
                            </div>
                        </div>

                        // Public Rooms List
                        <div style="border-top: 1px solid var(--border-color); padding-top: 8px; margin-top: 4px;">
                            <div style="font-size: 12px; font-weight: 700; color: var(--text-secondary); margin-bottom: 6px;">
                                {move || i18n.tr("lobby_public_rooms")}
                            </div>
                            {move || {
                                let rooms = public_rooms.get();
                                if rooms.is_empty() {
                                    view! {
                                        <div style="font-size: 11px; color: var(--text-muted); text-align: center; padding: 12px 0;">
                                            {move || i18n.tr("lobby_no_rooms")}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div style="display: flex; flex-direction: column; gap: 6px;">
                                            {rooms.into_iter().map(|r| {
                                                let r_id = r.id.clone();
                                                let g_join = game;
                                                let u_name = auth.username.get().unwrap_or_else(|| format!("Player_{}", &uuid::Uuid::new_v4().to_string()[..4]));
                                                let tok = auth.token.get();
                                                view! {
                                                    <div class="player-score-row">
                                                        <div>
                                                            <div style="font-weight: 700; font-size: 13px;">{r.name}</div>
                                                            <div style="font-size: 11px; color: var(--text-muted);">
                                                                "[" {r.id} "] " {format!("{:?}", r.difficulty)} " (" {r.player_count} "P)"
                                                            </div>
                                                        </div>
                                                        <button
                                                            class="btn btn-sm btn-primary"
                                                            on:click=move |_| g_join.mp_join_room(r_id.clone(), u_name.clone(), tok.clone())
                                                        >
                                                            {move || i18n.tr("btn_join")}
                                                        </button>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
