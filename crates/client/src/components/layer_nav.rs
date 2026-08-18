use crate::state::game_state::{AppMode, GameState};
use crate::state::i18n_context::I18nContext;
use leptos::prelude::*;

#[component]
pub fn LayerNav(i18n: I18nContext, game: GameState) -> impl IntoView {
    let mode = game.mode;
    let current_layer = move || {
        if mode.get() == AppMode::SinglePlayer {
            game.sp_current_layer.get()
        } else {
            game.mp_current_layer.get()
        }
    };

    let total_layers = move || {
        if mode.get() == AppMode::SinglePlayer {
            game.sp_config.get().depth
        } else {
            game.mp_room.get().map(|r| r.config.depth).unwrap_or(3)
        }
    };

    let game_clone = game;

    view! {
        <div class="layer-bar">
            <div class="layer-title">
                "🌌 " {move || i18n.tr("hud_layer")} ": "
                <strong style="color: var(--primary); font-family: var(--font-mono); font-size: 15px;">
                    {move || format!("Z = {} / {}", current_layer(), total_layers().saturating_sub(1))}
                </strong>
            </div>

            <button class="btn btn-sm" on:click={
                let g = game_clone;
                move |_| g.step_layer(-1)
            }>
                "◀ Q"
            </button>

            <div class="layer-buttons">
                {move || {
                    let max_d = total_layers();
                    let curr = current_layer();
                    let g = game_clone;
                    (0..max_d).map(|z| {
                        let g_sub = g;
                        let is_active = z == curr;
                        view! {
                            <button
                                class=if is_active { "layer-btn active" } else { "layer-btn" }
                                on:click=move |_| g_sub.set_layer(z)
                            >
                                {z.to_string()}
                            </button>
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>

            <button class="btn btn-sm" on:click={
                let g = game_clone;
                move |_| g.step_layer(1)
            }>
                "E ▶"
            </button>
        </div>
    }
}
