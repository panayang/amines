use crate::state::auth_state::AuthState;
use crate::state::game_state::{AppMode, GameState};
use crate::state::i18n_context::I18nContext;
use leptos::prelude::*;
use shared::board::GameStatus;
use shared::topology::Coord3D;
use web_sys::MouseEvent;

#[component]
pub fn BoardView(i18n: I18nContext, auth: AuthState, game: GameState) -> impl IntoView {
    let current_layer = move || match game.mode.get() {
        AppMode::SinglePlayer => game.sp_current_layer.get(),
        AppMode::Multiplayer => game.mp_current_layer.get(),
    };

    let config = move || match game.mode.get() {
        AppMode::SinglePlayer => game.sp_config.get(),
        AppMode::Multiplayer => game
            .mp_room
            .get()
            .map(|r| r.config)
            .unwrap_or_else(|| game.sp_config.get()),
    };

    let on_wheel = {
        let g = game;
        move |e: web_sys::WheelEvent| {
            e.prevent_default();
            if e.delta_y() > 0.0 {
                g.step_layer(1);
            } else if e.delta_y() < 0.0 {
                g.step_layer(-1);
            }
        }
    };

    view! {
        <div class="board-viewport" on:wheel=on_wheel>
            <div class="board-wrapper">
                // Left Möbius Twist Seam Indicator
                <div
                    class="mobius-guide left"
                    title=move || i18n.tr("guide_mobius_left")
                >
                    <span class="mobius-guide-label">"MÖBIUS SEAM"</span>
                    <span style="font-size: 14px;">"⟲"</span>
                    <span class="mobius-guide-label">"INVERT Y/Z"</span>
                </div>

                // Board Matrix with X & Y Coordinates
                <div class="board-coord-container">
                    // Top X-Axis Header
                    <div
                        class="coords-x-bar"
                        style=move || {
                            let cfg = config();
                            format!(
                                "grid-template-columns: 24px repeat({}, var(--cell-size));",
                                cfg.width
                            )
                        }
                    >
                        <div class="coord-corner">"X/Y"</div>
                        {move || {
                            let w = config().width;
                            (0..w).map(|x| view! {
                                <div class="coord-x-cell">{x}</div>
                            }).collect::<Vec<_>>()
                        }}
                    </div>

                    // Board Row: Y axis sidebar + 2D Slice Viewport (Layer Z)
                    <div class="board-y-and-grid">
                        // Left Y-Axis Sidebar
                        <div
                            class="coords-y-bar"
                            style=move || {
                                let cfg = config();
                                format!(
                                    "grid-template-rows: repeat({}, var(--cell-size));",
                                    cfg.height
                                )
                            }
                        >
                            {move || {
                                let h = config().height;
                                (0..h).map(|y| view! {
                                    <div class="coord-y-cell">{y}</div>
                                }).collect::<Vec<_>>()
                            }}
                        </div>

                        // Main 2D Slice Viewport (Layer Z)
                        <div
                            class="board-grid"
                            style=move || {
                                let cfg = config();
                                format!(
                                    "grid-template-columns: repeat({}, var(--cell-size)); grid-template-rows: repeat({}, var(--cell-size));",
                                    cfg.width, cfg.height
                                )
                            }
                        >
                            {move || {
                                let z = current_layer();
                                let cfg = config();
                                let w = cfg.width;
                                let h = cfg.height;
                                let tok = auth.token.get();
                                let active_hint = game.hint_coord.get();

                                let mut views = Vec::with_capacity(w * h);

                                if game.mode.get() == AppMode::SinglePlayer {
                                    let board = game.sp_board.get();
                                    for y in 0..h {
                                        for x in 0..w {
                                            let coord = Coord3D::new(x, y, z);
                                            let cell = board.get_cell(coord);
                                            let is_revealed = cell.is_revealed;
                                            let is_flagged = cell.is_flagged;
                                            let is_mine = cell.is_mine;
                                            let adj = cell.adjacent_mines;
                                            let is_hinted = active_hint == Some(coord);

                                            let g_left = game;
                                            let tok_left = tok.clone();
                                            let on_left_click = move |e: MouseEvent| {
                                                e.prevent_default();
                                                let b = g_left.sp_board.get_untracked();
                                                if b.status == GameStatus::Lost || b.status == GameStatus::Won {
                                                    return;
                                                }
                                                let c = b.get_cell(coord);
                                                if c.is_revealed && c.adjacent_mines > 0 {
                                                    g_left.sp_chord(coord, tok_left.clone());
                                                } else if !c.is_revealed && !c.is_flagged {
                                                    g_left.sp_reveal(coord, tok_left.clone());
                                                }
                                            };

                                            let g_right = game;
                                            let on_context_menu = move |e: MouseEvent| {
                                                e.prevent_default();
                                                let b = g_right.sp_board.get_untracked();
                                                if b.status == GameStatus::Lost || b.status == GameStatus::Won {
                                                    return;
                                                }
                                                g_right.sp_toggle_flag(coord);
                                            };

                                            let mut class_str = "cell".to_string();
                                            if is_revealed {
                                                class_str.push_str(" revealed");
                                            }
                                            if is_flagged {
                                                class_str.push_str(" flagged");
                                            }
                                            if is_revealed && is_mine {
                                                class_str.push_str(" mine");
                                            }
                                            if is_hinted {
                                                class_str.push_str(" hint-highlight");
                                            }

                                            let data_val = if is_revealed && adj > 0 && !is_mine {
                                                Some(adj.to_string())
                                            } else {
                                                None
                                            };

                                            let cell_label = if is_flagged {
                                                "🚩".to_string()
                                            } else if is_revealed {
                                                if is_mine {
                                                    "💣".to_string()
                                                } else if adj > 0 {
                                                    adj.to_string()
                                                } else {
                                                    "".to_string()
                                                }
                                            } else {
                                                "".to_string()
                                            };

                                            views.push(view! {
                                                <div
                                                    class=class_str
                                                    data-val={data_val}
                                                    on:click=on_left_click
                                                    on:contextmenu=on_context_menu
                                                >
                                                    <span>{cell_label}</span>
                                                </div>
                                            }.into_any());
                                        }
                                    }
                                } else {
                                    // Multiplayer Board View
                                    if let Some(room) = game.mp_room.get() {
                                        for y in 0..h {
                                            for x in 0..w {
                                                let coord = Coord3D::new(x, y, z);
                                                let idx = coord.to_index(w, h);
                                                let cell = room.cells.get(idx);

                                                let is_revealed = cell.map(|c| c.is_revealed).unwrap_or(false);
                                                let is_flagged = cell.map(|c| c.is_flagged).unwrap_or(false);
                                                let is_mine = cell.map(|c| c.is_mine).unwrap_or(false);
                                                let adj = cell.map(|c| c.adjacent_mines).unwrap_or(0);
                                                let owner_color = cell.and_then(|c| c.player_color.clone());
                                                let is_hinted = active_hint == Some(coord);

                                                let g_left = game;
                                                let on_left_click = move |e: MouseEvent| {
                                                    e.prevent_default();
                                                    if let Some(r) = g_left.mp_room.get_untracked() {
                                                        let cell_idx = coord.to_index(r.config.width, r.config.height);
                                                        if let Some(c) = r.cells.get(cell_idx) {
                                                            if c.is_revealed && c.adjacent_mines > 0 {
                                                                g_left.mp_chord(coord);
                                                            } else if !c.is_revealed && !c.is_flagged {
                                                                g_left.mp_reveal(coord);
                                                            }
                                                        }
                                                    }
                                                };

                                                let g_right = game;
                                                let on_context_menu = move |e: MouseEvent| {
                                                    e.prevent_default();
                                                    g_right.mp_toggle_flag(coord);
                                                };

                                                let mut class_str = "cell".to_string();
                                                if is_revealed {
                                                    class_str.push_str(" revealed");
                                                }
                                                if is_flagged {
                                                    class_str.push_str(" flagged");
                                                }
                                                if is_revealed && is_mine {
                                                    class_str.push_str(" mine");
                                                }
                                                if is_hinted {
                                                    class_str.push_str(" hint-highlight");
                                                }

                                                let data_val = if is_revealed && adj > 0 && !is_mine {
                                                    Some(adj.to_string())
                                                } else {
                                                    None
                                                };

                                                let cell_label = if is_flagged {
                                                    "🚩".to_string()
                                                } else if is_revealed {
                                                    if is_mine {
                                                        "💣".to_string()
                                                    } else if adj > 0 {
                                                        adj.to_string()
                                                    } else {
                                                        "".to_string()
                                                    }
                                                } else {
                                                    "".to_string()
                                                };

                                                views.push(view! {
                                                    <div
                                                        class=class_str
                                                        data-val={data_val}
                                                        on:click=on_left_click
                                                        on:contextmenu=on_context_menu
                                                    >
                                                        <span>{cell_label}</span>
                                                        {if let Some(color) = owner_color {
                                                            view! {
                                                                <span
                                                                    class="cell-owner-dot"
                                                                    style=format!("background-color: {color};")
                                                                ></span>
                                                            }.into_any()
                                                        } else {
                                                            view! { <span></span> }.into_any()
                                                        }}
                                                    </div>
                                                }.into_any());
                                            }
                                        }
                                    }
                                }

                                views
                            }}
                        </div>
                    </div>
                </div>

                // Right Möbius Twist Seam Indicator
                <div
                    class="mobius-guide right"
                    title=move || i18n.tr("guide_mobius_right")
                >
                    <span class="mobius-guide-label">"MÖBIUS SEAM"</span>
                    <span style="font-size: 14px;">"⟲"</span>
                    <span class="mobius-guide-label">"INVERT Y/Z"</span>
                </div>
            </div>
        </div>
    }
}
