use crate::state::auth_state::AuthState;
use crate::state::i18n_context::I18nContext;
use gloo_net::http::Request;
use leptos::prelude::*;
use shared::protocol::{AuthRequest, AuthResponse};

#[component]
pub fn AuthModal(
    i18n: I18nContext,
    auth: AuthState,
    is_open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    show_stats_first: ReadSignal<bool>,
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

    let format_pb = |ms_opt: Option<u64>| match ms_opt {
        Some(ms) => {
            let s = ms / 1000;
            format!("{:02}:{:02}", s / 60, s % 60)
        }
        None => "--:--".to_string(),
    };

    view! {
        {move || {
            if !is_open.get() {
                return view! { <div></div> }.into_any();
            }

            let is_logged = auth.is_logged_in();
            let show_stats = show_stats_first.get() || is_logged;

            view! {
                <div class="modal-backdrop" on:click=move |_| set_open.set(false)>
                    <div class="modal-card" on:click=|e| e.stop_propagation()>
                        <div class="modal-header">
                            <span>
                                {move || {
                                    if show_stats {
                                        i18n.tr("nav_stats")
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
                            {if show_stats && is_logged {
                                // User stats panel
                                let stats = auth.stats.get();
                                view! {
                                    <div style="display: flex; flex-direction: column; gap: 10px;">
                                        <div style="font-size: 16px; font-weight: 700; color: var(--primary);">
                                            "👤 " {auth.username.get().unwrap_or_default()}
                                        </div>

                                        <div style="background: var(--bg-card-subtle); padding: 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-color);">
                                            <div style="font-weight: 700; font-size: 13px; margin-bottom: 8px; color: var(--accent-gold);">
                                                "🏆 " {move || i18n.tr("hud_pb")}
                                            </div>
                                            <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; text-align: center;">
                                                <div style="background: var(--bg-elevated); padding: 6px; border-radius: 4px;">
                                                    <div style="font-size: 11px; color: var(--text-muted);">{move || i18n.tr("diff_easy")}</div>
                                                    <div style="font-family: var(--font-mono); font-weight: 700; color: var(--primary);">{format_pb(stats.as_ref().and_then(|s| s.easy_pb_ms))}</div>
                                                </div>
                                                <div style="background: var(--bg-elevated); padding: 6px; border-radius: 4px;">
                                                    <div style="font-size: 11px; color: var(--text-muted);">{move || i18n.tr("diff_medium")}</div>
                                                    <div style="font-family: var(--font-mono); font-weight: 700; color: var(--primary);">{format_pb(stats.as_ref().and_then(|s| s.medium_pb_ms))}</div>
                                                </div>
                                                <div style="background: var(--bg-elevated); padding: 6px; border-radius: 4px;">
                                                    <div style="font-size: 11px; color: var(--text-muted);">{move || i18n.tr("diff_expert")}</div>
                                                    <div style="font-family: var(--font-mono); font-weight: 700; color: var(--primary);">{format_pb(stats.as_ref().and_then(|s| s.expert_pb_ms))}</div>
                                                </div>
                                            </div>
                                        </div>

                                        <div style="background: var(--bg-card-subtle); padding: 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); display: flex; justify-content: space-around; text-align: center;">
                                            <div>
                                                <div style="font-size: 11px; color: var(--text-muted);">"SP Games"</div>
                                                <div style="font-family: var(--font-mono); font-weight: 700;">{stats.as_ref().map(|s| s.sp_games_played).unwrap_or(0)}</div>
                                            </div>
                                            <div>
                                                <div style="font-size: 11px; color: var(--text-muted);">"MP Wins"</div>
                                                <div style="font-family: var(--font-mono); font-weight: 700; color: var(--success);">{stats.as_ref().map(|s| s.mp_games_won).unwrap_or(0)}</div>
                                            </div>
                                            <div>
                                                <div style="font-size: 11px; color: var(--text-muted);">"MP Total Score"</div>
                                                <div style="font-family: var(--font-mono); font-weight: 700; color: var(--primary);">{stats.as_ref().map(|s| s.mp_total_score).unwrap_or(0)}</div>
                                            </div>
                                        </div>

                                        <button class="btn btn-sm btn-danger" style="margin-top: 8px;" on:click=move |_| auth.logout()>
                                            {move || i18n.tr("nav_logout")}
                                        </button>
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
