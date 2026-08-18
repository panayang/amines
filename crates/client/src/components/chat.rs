use crate::state::game_state::GameState;
use crate::state::i18n_context::I18nContext;
use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use shared::i18n::render_system_event;

#[component]
pub fn Chat(i18n: I18nContext, game: GameState) -> impl IntoView {
    let (active_tab, set_active_tab) = signal(0); // 0 = all/chat, 1 = system logs
    let (chat_input, set_chat_input) = signal("".to_string());

    let send_current_chat = {
        let g = game;
        move || {
            let txt = chat_input.get().trim().to_string();
            if !txt.is_empty() {
                g.mp_send_chat(txt);
                set_chat_input.set("".to_string());
            }
        }
    };

    let on_keydown = {
        let send = send_current_chat;
        move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                send();
            }
        }
    };

    view! {
        <div class="side-card chat-container">
            <div class="chat-tabs">
                <button
                    class=move || if active_tab.get() == 0 { "chat-tab active" } else { "chat-tab" }
                    on:click=move |_| set_active_tab.set(0)
                >
                    {move || i18n.tr("tab_chat")}
                </button>
                <button
                    class=move || if active_tab.get() == 1 { "chat-tab active" } else { "chat-tab" }
                    on:click=move |_| set_active_tab.set(1)
                >
                    {move || i18n.tr("tab_events")}
                </button>
            </div>

            <div class="chat-messages">
                {move || {
                    let logs = game.mp_chat_logs.get();
                    let tab = active_tab.get();
                    let current_lang = i18n.lang.get();

                    logs.into_iter()
                        .filter(|m| {
                            if tab == 1 {
                                m.is_system
                            } else {
                                true
                            }
                        })
                        .map(|msg| {
                            let is_sys = msg.is_system;
                            let color = msg.color.unwrap_or_else(|| "#a855f7".to_string());

                            let rendered_text = if is_sys {
                                if let Some(key) = msg.event_key.as_ref() {
                                    render_system_event(current_lang, key, &msg.event_params)
                                } else {
                                    msg.text
                                }
                            } else {
                                msg.text
                            };

                            view! {
                                <div class=if is_sys { "chat-msg system" } else { "chat-msg" }>
                                    <span class="chat-author" style=format!("color: {color};")>
                                        {msg.username} ":"
                                    </span>
                                    <span>{rendered_text}</span>
                                </div>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </div>

            // Quick emoji row
            <div style="display: flex; gap: 4px; padding-top: 4px;">
                {["💣", "🚩", "⚡", "😎", "GG", "GLHF"].into_iter().map(|emoji| {
                    let g_emo = game;
                    let e_str = emoji.to_string();
                    view! {
                        <button
                            class="btn btn-sm"
                            style="padding: 2px 6px; font-size: 11px;"
                            on:click=move |_| g_emo.mp_send_chat(e_str.clone())
                        >
                            {emoji}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>

            <div class="chat-input-bar">
                <input
                    class="chat-input"
                    placeholder=move || i18n.tr("chat_placeholder")
                    prop:value=move || chat_input.get()
                    on:input=move |e| set_chat_input.set(event_target_value(&e))
                    on:keydown=on_keydown
                />
                <button
                    class="btn btn-sm btn-primary"
                    on:click={
                        let send = send_current_chat;
                        move |_| send()
                    }
                >
                    {move || i18n.tr("chat_send")}
                </button>
            </div>
        </div>
    }
}
