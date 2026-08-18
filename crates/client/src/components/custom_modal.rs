use crate::state::game_state::GameState;
use crate::state::i18n_context::I18nContext;
use leptos::prelude::*;
use shared::board::BoardConfig;

#[component]
pub fn CustomModal(
    i18n: I18nContext,
    game: GameState,
    is_open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
) -> impl IntoView {
    let (width, set_width) = signal(12usize);
    let (height, set_height) = signal(12usize);
    let (depth, set_depth) = signal(3usize);
    let (mines, set_mines) = signal(50usize);
    let (err_msg, set_err_msg) = signal(Option::<String>::None);

    let total_cells = move || width.get() * height.get() * depth.get();
    let max_density_mines = move || (total_cells() as f64 * 0.6).floor() as usize;
    let max_safe_mines = move || total_cells().saturating_sub(27);
    let max_allowed = move || max_density_mines().min(max_safe_mines());
    let density_percent = move || {
        let tot = total_cells();
        if tot == 0 {
            0.0
        } else {
            (mines.get() as f64 / tot as f64) * 100.0
        }
    };

    let on_apply = {
        let g = game;
        move |_| {
            let w = width.get();
            let h = height.get();
            let d = depth.get();
            let m = mines.get();

            match BoardConfig::custom(w, h, d, m) {
                Ok(cfg) => {
                    g.set_custom_config.set(cfg);
                    if g.mode.get() == crate::state::AppMode::SinglePlayer {
                        g.reset_sp_game(cfg);
                    }
                    set_open.set(false);
                    set_err_msg.set(None);
                }
                Err(e) => {
                    set_err_msg.set(Some(e));
                }
            }
        }
    };

    view! {
        {move || {
            if !is_open.get() {
                return view! { <div></div> }.into_any();
            }

            view! {
                <div class="modal-backdrop" on:click=move |_| set_open.set(false)>
                    <div class="modal-card" on:click=|e| e.stop_propagation()>
                        <div class="modal-header">
                            <span>{move || i18n.tr("custom_title")}</span>
                            <button class="btn btn-sm" on:click=move |_| set_open.set(false)>"✕"</button>
                        </div>

                        <div class="modal-body">
                            {move || {
                                if let Some(e) = err_msg.get() {
                                    view! {
                                        <div style="color: var(--danger); font-size: 12px; background: rgba(239, 68, 68, 0.15); padding: 6px 10px; border-radius: 4px;">
                                            {e}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }
                            }}

                            <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px;">
                                <div class="form-group">
                                    <label class="form-label">{move || i18n.tr("custom_width")}</label>
                                    <input
                                        class="form-input"
                                        type="number"
                                        min="3"
                                        max="60"
                                        prop:value=move || width.get().to_string()
                                        on:input=move |e| {
                                            if let Ok(v) = event_target_value(&e).parse::<usize>() {
                                                set_width.set(v.max(3));
                                            }
                                        }
                                    />
                                </div>
                                <div class="form-group">
                                    <label class="form-label">{move || i18n.tr("custom_height")}</label>
                                    <input
                                        class="form-input"
                                        type="number"
                                        min="3"
                                        max="60"
                                        prop:value=move || height.get().to_string()
                                        on:input=move |e| {
                                            if let Ok(v) = event_target_value(&e).parse::<usize>() {
                                                set_height.set(v.max(3));
                                            }
                                        }
                                    />
                                </div>
                                <div class="form-group">
                                    <label class="form-label">{move || i18n.tr("custom_depth")}</label>
                                    <input
                                        class="form-input"
                                        type="number"
                                        min="2"
                                        max="12"
                                        prop:value=move || depth.get().to_string()
                                        on:input=move |e| {
                                            if let Ok(v) = event_target_value(&e).parse::<usize>() {
                                                set_depth.set(v.max(2));
                                            }
                                        }
                                    />
                                </div>
                            </div>

                            <div class="form-group">
                                <label class="form-label">
                                    {move || i18n.tr("custom_mines")}
                                    " (Max: " {move || max_allowed()} ", " {move || format!("{:.1}%", density_percent())} ")"
                                </label>
                                <input
                                    class="form-input"
                                    type="number"
                                    min="1"
                                    prop:value=move || mines.get().to_string()
                                    on:input=move |e| {
                                        if let Ok(v) = event_target_value(&e).parse::<usize>() {
                                            set_mines.set(v);
                                        }
                                    }
                                />
                            </div>

                            <div style="background: var(--bg-card-subtle); padding: 10px; border-radius: var(--radius-sm); font-size: 12px; color: var(--text-secondary);">
                                <div>{move || i18n.tr("custom_total_cells")} <strong style="color: var(--primary); font-family: var(--font-mono);">{move || total_cells()}</strong></div>
                                <div>{move || i18n.tr("custom_safe_opening")} <strong style="color: var(--success); font-family: var(--font-mono);">{move || i18n.tr("custom_cells_unit")}</strong></div>
                            </div>

                            <button class="btn btn-primary" on:click=on_apply>
                                {move || i18n.tr("custom_confirm")}
                            </button>
                        </div>
                    </div>
                </div>
            }.into_any()
        }}
    }
}
