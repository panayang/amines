use parking_lot::RwLock;
use rand::Rng;
use shared::ai_solver::{AiAction, AiSolver, BotTier};
use shared::protocol::*;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub fn spawn_bot_worker(
    bot_id: String,
    tier: BotTier,
    mut rx: broadcast::Receiver<ServerMessage>,
    room_arc: std::sync::Weak<RwLock<crate::room::Room>>,
    cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let mut last_snapshot: Option<RoomSnapshot> = None;

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    break;
                }
                msg_res = rx.recv() => {
                    match msg_res {
                        Ok(ServerMessage::RoomState(snap)) => {
                            last_snapshot = Some(snap);
                        }
                        Ok(ServerMessage::GameStarted { .. }) => {
                            if let Some(r_weak) = room_arc.upgrade() {
                                last_snapshot = Some(r_weak.read().snapshot());
                            }
                        }
                        Ok(ServerMessage::CellsRevealed { revealed, .. }) => {
                            if let Some(ref mut snap) = last_snapshot {
                                for r in revealed {
                                    let idx = r.coord.to_index(snap.config.width, snap.config.height);
                                    if idx < snap.cells.len() {
                                        snap.cells[idx].is_revealed = true;
                                        snap.cells[idx].adjacent_mines = r.adjacent_mines;
                                        snap.cells[idx].is_mine = r.is_mine;
                                        snap.cells[idx].revealed_by = r.revealed_by;
                                        snap.cells[idx].player_color = r.player_color;
                                    }
                                }
                            }
                        }
                        Ok(ServerMessage::PlayerEliminated { player_id, hit_coord, .. }) => {
                            if player_id == bot_id {
                                continue;
                            }
                            if let Some(ref mut snap) = last_snapshot {
                                let idx = hit_coord.to_index(snap.config.width, snap.config.height);
                                if idx < snap.cells.len() {
                                    snap.cells[idx].is_revealed = true;
                                    snap.cells[idx].is_mine = true;
                                }
                            }
                        }
                        Ok(ServerMessage::PlayerFlagToggled { coord, is_flagged, .. }) => {
                            if let Some(ref mut snap) = last_snapshot {
                                let idx = coord.to_index(snap.config.width, snap.config.height);
                                if idx < snap.cells.len() {
                                    snap.cells[idx].is_flagged = is_flagged;
                                }
                            }
                        }
                        Ok(ServerMessage::GameOver { .. }) => {
                            if let Some(ref mut snap) = last_snapshot {
                                snap.status = shared::board::GameStatus::Won;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(60)) => {
                    let snap = match last_snapshot.as_ref() {
                        Some(s) if s.status == shared::board::GameStatus::Playing => s.clone(),
                        _ => continue,
                    };

                    let is_active = snap.players.iter().any(|p| p.id == bot_id && !p.is_eliminated && !p.is_spectator);
                    if !is_active {
                        continue;
                    }

                    // Compute next action using AiSolver
                    let action = AiSolver::decide_action(
                        snap.config.dims(),
                        &snap.cells,
                        tier,
                        snap.config.mines,
                    );

                    if let Some(act) = action {
                        // Calculate human-like jittered delay based on bot_speed_ms
                        let base_ms = snap.bot_speed_ms;
                        let mult = tier.speed_multiplier();
                        let jitter: i64 = {
                            let mut rng = rand::thread_rng();
                            rng.gen_range(-40..=70)
                        };
                        let total_delay = ((base_ms as f64 * mult) as i64 + jitter).max(80) as u64;

                        tokio::time::sleep(Duration::from_millis(total_delay)).await;

                        if cancel_token.is_cancelled() {
                            break;
                        }

                        // Apply action to room
                        if let Some(r_arc) = room_arc.upgrade() {
                            let mut room = r_arc.write();
                            if room.status == shared::board::GameStatus::Playing {
                                match act {
                                    AiAction::Reveal(c) => {
                                        room.handle_reveal(&bot_id, c);
                                    }
                                    AiAction::Chord(c) => {
                                        room.handle_chord(&bot_id, c);
                                    }
                                    AiAction::Flag(c) => {
                                        room.handle_toggle_flag(&bot_id, c);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}
