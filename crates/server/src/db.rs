use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rusqlite::{params, Connection};
use shared::protocol::UserStatsResponse;
use shared::Difficulty;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub password_hash: String,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS personal_bests (
                user_id TEXT NOT NULL,
                difficulty TEXT NOT NULL,
                config_hash TEXT NOT NULL,
                time_ms INTEGER NOT NULL,
                moves INTEGER NOT NULL,
                achieved_at TEXT NOT NULL,
                PRIMARY KEY (user_id, difficulty, config_hash),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS user_stats (
                user_id TEXT PRIMARY KEY,
                sp_games_played INTEGER NOT NULL DEFAULT 0,
                sp_games_won INTEGER NOT NULL DEFAULT 0,
                mp_games_played INTEGER NOT NULL DEFAULT 0,
                mp_games_won INTEGER NOT NULL DEFAULT 0,
                mp_total_score INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );
            ",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn register_user(&self, username: &str, password: &str) -> Result<UserRecord, String> {
        if username.trim().is_empty() || password.len() < 4 {
            return Err(
                "Username cannot be empty and password must be at least 4 characters".into(),
            );
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("Password hash error: {e}"))?
            .to_string();

        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![user_id, username, password_hash, now],
        )
        .map_err(|e| format!("Username may already exist or database error: {e}"))?;

        conn.execute(
            "INSERT INTO user_stats (user_id) VALUES (?1)",
            params![user_id],
        )
        .map_err(|e| format!("Stats init error: {e}"))?;

        Ok(UserRecord {
            id: user_id,
            username: username.to_string(),
            password_hash,
        })
    }

    pub fn verify_user(&self, username: &str, password: &str) -> Result<UserRecord, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, username, password_hash FROM users WHERE username = ?1")
            .map_err(|e| e.to_string())?;

        let user = stmt
            .query_row(params![username], |row| {
                Ok(UserRecord {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                })
            })
            .map_err(|_| "Invalid username or password".to_string())?;

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|e| format!("Invalid stored hash: {e}"))?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| "Invalid username or password".to_string())?;

        Ok(user)
    }

    pub fn get_user_by_id(&self, user_id: &str) -> Option<UserRecord> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, username, password_hash FROM users WHERE id = ?1")
            .ok()?;

        stmt.query_row(params![user_id], |row| {
            Ok(UserRecord {
                id: row.get(0)?,
                username: row.get(1)?,
                password_hash: row.get(2)?,
            })
        })
        .ok()
    }

    pub fn record_sp_result(
        &self,
        user_id: &str,
        difficulty: Difficulty,
        config_hash: &str,
        time_ms: u64,
        moves: u32,
        won: bool,
    ) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        let diff_str = format!("{difficulty:?}");

        // Update games count
        conn.execute(
            "UPDATE user_stats SET sp_games_played = sp_games_played + 1, sp_games_won = sp_games_won + ?1 WHERE user_id = ?2",
            params![if won { 1 } else { 0 }, user_id],
        ).map_err(|e| e.to_string())?;

        if !won {
            return Ok(false);
        }

        // Check current PB
        let mut stmt = conn
            .prepare(
                "SELECT time_ms FROM personal_bests WHERE user_id = ?1 AND difficulty = ?2 AND config_hash = ?3",
            )
            .map_err(|e| e.to_string())?;

        let existing_pb: Option<u64> = stmt
            .query_row(params![user_id, diff_str, config_hash], |row| row.get(0))
            .ok();

        let is_new_pb = match existing_pb {
            Some(curr) => time_ms < curr,
            None => true,
        };

        if is_new_pb {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO personal_bests (user_id, difficulty, config_hash, time_ms, moves, achieved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(user_id, difficulty, config_hash) DO UPDATE SET
                 time_ms = excluded.time_ms, moves = excluded.moves, achieved_at = excluded.achieved_at",
                params![user_id, diff_str, config_hash, time_ms, moves, now],
            ).map_err(|e| e.to_string())?;
        }

        Ok(is_new_pb)
    }

    pub fn get_user_stats(&self, user_id: &str) -> Result<UserStatsResponse, String> {
        let conn = self.conn.lock().unwrap();

        let username: String = conn
            .query_row(
                "SELECT username FROM users WHERE id = ?1",
                params![user_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;

        let stats_row = conn.query_row(
            "SELECT sp_games_played, sp_games_won, mp_games_played, mp_games_won, mp_total_score FROM user_stats WHERE user_id = ?1",
            params![user_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).unwrap_or((0, 0, 0, 0, 0));

        let mut pb_stmt = conn
            .prepare("SELECT difficulty, time_ms FROM personal_bests WHERE user_id = ?1")
            .map_err(|e| e.to_string())?;

        let mut easy_pb = None;
        let mut medium_pb = None;
        let mut expert_pb = None;

        let pbs = pb_stmt
            .query_map(params![user_id], |row| {
                let diff: String = row.get(0)?;
                let time: u64 = row.get(1)?;
                Ok((diff, time))
            })
            .map_err(|e| e.to_string())?;

        for item in pbs.flatten() {
            match item.0.as_str() {
                "Easy" => easy_pb = Some(item.1),
                "Medium" => medium_pb = Some(item.1),
                "Expert" => expert_pb = Some(item.1),
                _ => {}
            }
        }

        Ok(UserStatsResponse {
            username,
            easy_pb_ms: easy_pb,
            medium_pb_ms: medium_pb,
            expert_pb_ms: expert_pb,
            sp_games_played: stats_row.0,
            sp_games_won: stats_row.1,
            mp_games_played: stats_row.2,
            mp_games_won: stats_row.3,
            mp_total_score: stats_row.4,
        })
    }

    pub fn record_mp_match(&self, player_scores: &[(String, u32)], winner_id: Option<&str>) {
        let conn = self.conn.lock().unwrap();
        for (user_id, score) in player_scores {
            let is_winner = match winner_id {
                Some(w) => w == user_id,
                None => false,
            };
            let _ = conn.execute(
                "UPDATE user_stats SET mp_games_played = mp_games_played + 1,
                 mp_games_won = mp_games_won + ?1,
                 mp_total_score = mp_total_score + ?2
                 WHERE user_id = ?3",
                params![if is_winner { 1 } else { 0 }, score, user_id],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_user_registration_and_auth() {
        let db = Database::new(":memory:").expect("Failed to create in-memory db");
        let user = db
            .register_user("alice", "secret123")
            .expect("Register failed");
        assert_eq!(user.username, "alice");

        // Verify valid login
        let verified = db.verify_user("alice", "secret123").expect("Verify failed");
        assert_eq!(verified.id, user.id);

        // Verify invalid password
        assert!(db.verify_user("alice", "wrong_pass").is_err());

        // Duplicate username should fail
        assert!(db.register_user("alice", "another_pass").is_err());
    }

    #[test]
    fn test_db_personal_best_tracking() {
        let db = Database::new(":memory:").expect("Failed to create in-memory db");
        let user = db
            .register_user("bob", "secret123")
            .expect("Register failed");

        // Initial win records PB
        let is_pb1 = db
            .record_sp_result(&user.id, Difficulty::Easy, "9-9-3-25", 15000, 12, true)
            .unwrap();
        assert!(is_pb1);

        let stats1 = db.get_user_stats(&user.id).unwrap();
        assert_eq!(stats1.easy_pb_ms, Some(15000));
        assert_eq!(stats1.sp_games_won, 1);

        // Slower time does not update PB
        let is_pb2 = db
            .record_sp_result(&user.id, Difficulty::Easy, "9-9-3-25", 18000, 15, true)
            .unwrap();
        assert!(!is_pb2);

        let stats2 = db.get_user_stats(&user.id).unwrap();
        assert_eq!(stats2.easy_pb_ms, Some(15000));

        // Faster time updates PB
        let is_pb3 = db
            .record_sp_result(&user.id, Difficulty::Easy, "9-9-3-25", 12000, 10, true)
            .unwrap();
        assert!(is_pb3);

        let stats3 = db.get_user_stats(&user.id).unwrap();
        assert_eq!(stats3.easy_pb_ms, Some(12000));
    }
}
