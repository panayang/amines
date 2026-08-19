use crate::state::auth_state::AuthState;
use crate::state::i18n_context::I18nContext;
use gloo_net::http::Request;
use leptos::prelude::*;
use shared::protocol::{AuthRequest, AuthResponse};

#[component]
pub fn AuthModal(
    i18n: I18nContext,
    auth: AuthState,
    game: crate::state::game_state::GameState,
    is_open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    show_stats_first: ReadSignal<bool>,
    set_show_stats_first: WriteSignal<bool>,
) -> impl IntoView {
    let (is_register_tab, set_is_register_tab) = signal(false);
    let (username_input, set_username_input) = signal("".to_string());
    let (password_input, set_password_input) = signal("".to_string());
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);

    let on_submit = {
        let auth_state = auth;
        move |_| {
            let u = username_input.get().trim().to_string();
            let p = password_input.get().trim().to_string();
            if u.is_empty() || p.is_empty() {
                set_error_msg.set(Some("Please fill in both fields".to_string()));
                return;
            }

            set_loading.set(true);
            set_error_msg.set(None);

            let is_reg = is_register_tab.get();
            let url = if is_reg {
                "/api/auth/register"
            } else {
                "/api/auth/login"
            };

            wasm_bindgen_futures::spawn_local(async move {
                let req = Request::post(url)
                    .json(&AuthRequest {
                        username: u.clone(),
                        password: p,
                    })
                    .unwrap()
                    .send()
                    .await;

                set_loading.set(false);
                match req {
                    Ok(resp) => {
                        if resp.status() == 200 {
                            if let Ok(data) = resp.json::<AuthResponse>().await {
                                auth_state.set_logged_in(data.token, data.username);
                                set_open.set(false);
                            } else {
                                set_error_msg.set(Some("Failed to parse response".to_string()));
                            }
                        } else {
                            let text = resp
                                .text()
                                .await
                                .unwrap_or_else(|_| "Auth failed".to_string());
                            set_error_msg.set(Some(text));
                        }
                    }
                    Err(e) => {
                        set_error_msg.set(Some(format!("Network error: {e}")));
                    }
                }
            });
        }
    };

    let get_display_pb = move |diff: shared::board::Difficulty| -> String {
        let pbs = game.sp_pb_records.get();
        let local_pb = pbs.get_pb(diff).cloned();
        let cloud_pb_ms = auth.stats.get().as_ref().and_then(|s| match diff {
            shared::board::Difficulty::Easy => s.easy_pb_ms,
            shared::board::Difficulty::Medium => s.medium_pb_ms,
            shared::board::Difficulty::Expert => s.expert_pb_ms,
            shared::board::Difficulty::Custom => None,
        });

        match (local_pb, cloud_pb_ms) {
            (Some(loc), Some(cloud_ms)) => {
                let cloud_s = cloud_ms / 1000;
                let best_s = loc.time_secs.min(cloud_s);
                format!("{}s ({}m)", best_s, loc.moves)
            }
            (Some(loc), None) => {
                format!("{}s ({}m)", loc.time_secs, loc.moves)
            }
            (None, Some(cloud_ms)) => {
                let s = cloud_ms / 1000;
                format!("{}s", s)
            }
            (None, None) => "--:--".to_string(),
        }
    };

    view! {
        {move || {
            if !is_open.get() {
                return view! { <div></div> }.into_any();
            }

            let is_logged = auth.is_logged_in();
            let show_stats = show_stats_first.get();

            view! {
                <div class="modal-backdrop" on:click=move |_| set_open.set(false)>
                    <div class="modal-card" on:click=|e| e.stop_propagation()>
                        <div class="modal-header">
                            <span>
                                {move || {
                                    if show_stats {
                                        if i18n.is_zh() { "🏆 个人纪录与统计" } else { "🏆 Records & Statistics" }
                                    } else if is_register_tab.get() {
                                        i18n.tr("auth_register_title")
                                    } else {
                                        i18n.tr("auth_login_title")
                                    }
                                }}
                            </span>
                            <button class="btn btn-sm" on:click=move |_| set_open.set(false)>"✕"</button>
                        </div>

                        <div class="modal-body">
                            {if show_stats {
                                let stats = auth.stats.get();
                                view! {
                                    <div style="display: flex; flex-direction: column; gap: 12px;">
                                        // User Identity Header
                                        <div style="display: flex; justify-content: space-between; align-items: center;">
                                            <div style="font-size: 15px; font-weight: 700; color: var(--accent-gold);">
                                                {if is_logged {
                                                    view! { <span>"👤 " {auth.username.get().unwrap_or_default()}</span> }.into_any()
                                                } else {
                                                    view! { <span>"💾 " {move || if i18n.is_zh() { "访客模式 (本地存储)" } else { "Guest (Local Storage)" }}</span> }.into_any()
                                                }}
                                            </div>
                                            <div>
                                                {if is_logged {
                                                    view! { <span style="font-size: 11px; background: rgba(52, 211, 153, 0.15); color: #34d399; padding: 2px 8px; border-radius: 999px; font-weight: 600;">"☁️ Cloud"</span> }.into_any()
                                                } else {
                                                    view! { <span style="font-size: 11px; background: rgba(251, 191, 36, 0.15); color: #fbbf24; padding: 2px 8px; border-radius: 999px; font-weight: 600;">"💾 Local"</span> }.into_any()
                                                }}
                                            </div>
                                        </div>

                                        // All 4-Difficulty Personal Bests Grid
                                        <div style="background: var(--bg-card-subtle); padding: 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-color);">
                                            <div style="font-weight: 700; font-size: 12px; margin-bottom: 8px; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.5px;">
                                                "🏆 " {move || if i18n.is_zh() { "全难度最佳成绩 (Personal Bests)" } else { "Personal Bests (All Difficulties)" }}
                                            </div>
                                            <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px;">
                                                <div style="background: var(--bg-elevated); padding: 8px; border-radius: 4px;">
                                                    <div style="font-size: 11px; color: var(--text-muted); font-weight: 600;">{move || i18n.tr("diff_easy")} " (9x9x3)"</div>
                                                    <div style="font-family: var(--font-mono); font-weight: 700; color: #34d399; font-size: 14px; margin-top: 2px;">
                                                        {get_display_pb(shared::board::Difficulty::Easy)}
                                                    </div>
                                                </div>
                                                <div style="background: var(--bg-elevated); padding: 8px; border-radius: 4px;">
                                                    <div style="font-size: 11px; color: var(--text-muted); font-weight: 600;">{move || i18n.tr("diff_medium")} " (16x16x4)"</div>
                                                    <div style="font-family: var(--font-mono); font-weight: 700; color: #34d399; font-size: 14px; margin-top: 2px;">
                                                        {get_display_pb(shared::board::Difficulty::Medium)}
                                                    </div>
                                                </div>
                                                <div style="background: var(--bg-elevated); padding: 8px; border-radius: 4px;">
                                                    <div style="font-size: 11px; color: var(--text-muted); font-weight: 600;">{move || i18n.tr("diff_expert")} " (30x16x6)"</div>
                                                    <div style="font-family: var(--font-mono); font-weight: 700; color: #34d399; font-size: 14px; margin-top: 2px;">
                                                        {get_display_pb(shared::board::Difficulty::Expert)}
                                                    </div>
                                                </div>
                                                <div style="background: var(--bg-elevated); padding: 8px; border-radius: 4px;">
                                                    <div style="font-size: 11px; color: var(--text-muted); font-weight: 600;">{move || i18n.tr("diff_custom")}</div>
                                                    <div style="font-family: var(--font-mono); font-weight: 700; color: #34d399; font-size: 14px; margin-top: 2px;">
                                                        {get_display_pb(shared::board::Difficulty::Custom)}
                                                    </div>
                                                </div>
                                            </div>
                                        </div>

                                        // Cloud Stats if logged in
                                        {if is_logged {
                                            view! {
                                                <div style="background: var(--bg-card-subtle); padding: 10px 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); display: flex; justify-content: space-around; text-align: center;">
                                                    <div>
                                                        <div style="font-size: 11px; color: var(--text-muted);">"SP Games"</div>
                                                        <div style="font-family: var(--font-mono); font-weight: 700; font-size: 13px;">{stats.as_ref().map(|s| s.sp_games_played).unwrap_or(0)}</div>
                                                    </div>
                                                    <div>
                                                        <div style="font-size: 11px; color: var(--text-muted);">"MP Wins"</div>
                                                        <div style="font-family: var(--font-mono); font-weight: 700; color: var(--success); font-size: 13px;">{stats.as_ref().map(|s| s.mp_games_won).unwrap_or(0)}</div>
                                                    </div>
                                                    <div>
                                                        <div style="font-size: 11px; color: var(--text-muted);">"MP Score"</div>
                                                        <div style="font-family: var(--font-mono); font-weight: 700; color: var(--primary); font-size: 13px;">{stats.as_ref().map(|s| s.mp_total_score).unwrap_or(0)}</div>
                                                    </div>
                                                </div>

                                                <button class="btn btn-sm btn-danger" style="margin-top: 4px;" on:click=move |_| auth.logout()>
                                                    {move || i18n.tr("nav_logout")}
                                                </button>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <div style="font-size: 11px; color: var(--text-secondary); text-align: center;">
                                                    {move || if i18n.is_zh() { "💡 登录或注册账号可将个人纪录同步至全球排行榜数据库。" } else { "💡 Log in or register to sync your personal bests to the cloud database." }}
                                                </div>
                                                <button
                                                    class="btn btn-primary"
                                                    on:click=move |_| {
                                                        set_show_stats_first.set(false);
                                                    }
                                                >
                                                    "🔑 " {move || if i18n.is_zh() { "前往登录 / 注册" } else { "Log In / Register" }}
                                                </button>
                                            }.into_any()
                                        }}
                                    </div>
                                }.into_any()
                            } else {
                                // Login / Register form
                                view! {
                                    <div style="display: flex; flex-direction: column; gap: 12px;">
                                        {move || {
                                            if let Some(err) = error_msg.get() {
                                                view! {
                                                    <div style="color: var(--danger); font-size: 12px; background: rgba(239, 68, 68, 0.15); padding: 6px 10px; border-radius: 4px;">
                                                        {err}
                                                    </div>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }
                                        }}

                                        <div class="form-group">
                                            <label class="form-label">{move || i18n.tr("auth_username")}</label>
                                            <input
                                                class="form-input"
                                                type="text"
                                                prop:value=move || username_input.get()
                                                on:input=move |e| set_username_input.set(event_target_value(&e))
                                            />
                                        </div>

                                        <div class="form-group">
                                            <label class="form-label">{move || i18n.tr("auth_password")}</label>
                                            <input
                                                class="form-input"
                                                type="password"
                                                prop:value=move || password_input.get()
                                                on:input=move |e| set_password_input.set(event_target_value(&e))
                                            />
                                        </div>

                                        <button
                                            class="btn btn-primary"
                                            style="margin-top: 6px;"
                                            disabled=move || loading.get()
                                            on:click=on_submit
                                        >
                                            {move || if is_register_tab.get() {
                                                i18n.tr("auth_submit_register")
                                            } else {
                                                i18n.tr("auth_submit_login")
                                            }}
                                        </button>

                                        <div
                                            style="font-size: 12px; color: var(--primary); text-align: center; cursor: pointer; margin-top: 4px;"
                                            on:click=move |_| {
                                                set_is_register_tab.update(|v| *v = !*v);
                                                set_error_msg.set(None);
                                            }
                                        >
                                            {move || if is_register_tab.get() {
                                                i18n.tr("auth_switch_to_login")
                                            } else {
                                                i18n.tr("auth_switch_to_reg")
                                            }}
                                        </div>

                                        <button
                                            class="btn btn-sm btn-subtle"
                                            style="margin-top: 6px;"
                                            on:click=move |_| {
                                                set_show_stats_first.set(true);
                                            }
                                        >
                                            "🏆 " {move || if i18n.is_zh() { "返回查看个人纪录" } else { "View Personal Bests" }}
                                        </button>
                                    </div>
                                }.into_any()
                            }}
                        </div>
                    </div>
                </div>
            }.into_any()
        }}
    }
}
