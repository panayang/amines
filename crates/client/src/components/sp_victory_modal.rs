use crate::state::game_state::GameState;
use crate::state::i18n_context::I18nContext;
use leptos::prelude::*;
use shared::board::Difficulty;

#[component]
pub fn SpVictoryModal(i18n: I18nContext, game: GameState) -> impl IntoView {
    let is_open_sig = game.sp_victory_modal;
    let is_new_pb_sig = game.sp_is_new_pb;
    let pb_records_sig = game.sp_pb_records;
    let time_sig = game.sp_time;
    let moves_sig = game.sp_moves;
    let config_sig = game.sp_config;

    view! {
        {move || {
            if !is_open_sig.get() {
                return view! { <div></div> }.into_any();
            }

            let is_new_pb = is_new_pb_sig.get();
            let elapsed_seconds = time_sig.get();
            let moves_count = moves_sig.get();
            let pbs = pb_records_sig.get();
            let current_cfg = config_sig.get();

            let g_restart = game;
            let g_close = game;

            let diff_rows = [
                (Difficulty::Easy, "Beginner (9x9x3)"),
                (Difficulty::Medium, "Intermediate (16x16x4)"),
                (Difficulty::Expert, "Expert (30x16x6)"),
                (Difficulty::Custom, "Custom Grid"),
            ];

            view! {
                <div class="modal-backdrop" on:click=move |_| g_close.set_sp_victory_modal.set(false)>
                    <div class="modal-card settlement-card won" on:click=move |e| e.stop_propagation() style="max-width: 520px;">
                        <div class="settlement-header">
                            <div class="settlement-icon">
                                "🏆"
                            </div>
                            <h2 class="settlement-title">
                                {move || if i18n.is_zh() { "🎉 3D 莫比乌斯全图扫空！🎉" } else { "🎉 3D MÖBIUS CLEARED! 🎉" }}
                            </h2>
                            <p class="settlement-subtitle" style="color: var(--accent-gold); font-weight: 600;">
                                {move || if is_new_pb {
                                    if i18n.is_zh() { "⭐ 创造了新的个人最好成绩 (NEW PB)! ⭐" } else { "⭐ NEW PERSONAL BEST ACHIEVED! ⭐" }
                                } else {
                                    if i18n.is_zh() { "所有安全区域均已探测完毕！" } else { "All safe sectors mapped and cleared!" }
                                }}
                            </p>
                        </div>

                        // Match stats row
                        <div class="settlement-stats-bar">
                            <div class="settlement-stat-item">
                                <span class="stat-lbl">{move || if i18n.is_zh() { "⏱️ 最终耗时" } else { "⏱️ Final Time" }}</span>
                                <span class="stat-val">{elapsed_seconds} "s"</span>
                            </div>
                            <div class="settlement-stat-item">
                                <span class="stat-lbl">{move || if i18n.is_zh() { "🎯 步数" } else { "🎯 Moves" }}</span>
                                <span class="stat-val">{moves_count}</span>
                            </div>
                            <div class="settlement-stat-item">
                                <span class="stat-lbl">{move || if i18n.is_zh() { "难度" } else { "Difficulty" }}</span>
                                <span class="stat-val" style="font-size: 13px; color: var(--accent-cyan);">{format!("{:?}", current_cfg.difficulty)}</span>
                            </div>
                        </div>

                        // All-Time Personal Bests table
                        <div class="settlement-leaderboard" style="margin-top: 14px;">
                            <div class="settlement-section-title">
                                <span>"📊 " {move || if i18n.is_zh() { "个人历史最佳纪录 (Personal Bests)" } else { "All-Time Personal Bests" }}</span>
                            </div>

                            <div style="background: rgba(0,0,0,0.3); border-radius: 8px; border: 1px solid rgba(255,255,255,0.08); padding: 8px 12px; margin-top: 8px;">
                                {diff_rows.into_iter().map(|(d, label)| {
                                    let pb_opt = pbs.get_pb(d);
                                    let is_curr = d == current_cfg.difficulty;
                                    view! {
                                        <div style=format!("display: flex; justify-content: space-between; align-items: center; padding: 6px 0; border-bottom: 1px solid rgba(255,255,255,0.05); font-size: 13px; color: {};", if is_curr { "var(--accent-gold)" } else { "var(--text-secondary)" })>
                                            <span style="font-weight: 600;">{label}</span>
                                            {match pb_opt {
                                                Some(rec) => view! {
                                                    <span style="font-weight: 700; color: #34d399;">
                                                        {format!("{}s ({} moves) [{}]", rec.time_secs, rec.moves, rec.date)}
                                                    </span>
                                                }.into_any(),
                                                None => view! {
                                                    <span style="color: var(--text-muted);">"-"</span>
                                                }.into_any(),
                                            }}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>

                        <div class="settlement-actions" style="margin-top: 18px; display: flex; gap: 12px; justify-content: flex-end;">
                            <button
                                class="btn btn-secondary"
                                on:click=move |_| g_close.set_sp_victory_modal.set(false)
                            >
                                {move || if i18n.is_zh() { "关闭" } else { "Dismiss" }}
                            </button>
                            <button
                                class="btn btn-primary"
                                on:click=move |_| {
                                    let cfg = g_restart.sp_config.get();
                                    g_restart.reset_sp_game(cfg);
                                }
                            >
                                {move || if i18n.is_zh() { "🔄 再来一局 (R)" } else { "🔄 Play Again (R)" }}
                            </button>
                        </div>
                    </div>
                </div>
            }.into_any()
        }}
    }
}
