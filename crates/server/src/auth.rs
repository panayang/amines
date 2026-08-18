use crate::db::{Database, UserRecord};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AuthService {
    db: Database,
    // token -> user_id
    sessions: Arc<RwLock<HashMap<String, String>>>,
}

impl AuthService {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, username: &str, password: &str) -> Result<(String, UserRecord), String> {
        let user = self.db.register_user(username, password)?;
        let token = format!("tok_{}", Uuid::new_v4());
        self.sessions.write().insert(token.clone(), user.id.clone());
        Ok((token, user))
    }

    pub fn login(&self, username: &str, password: &str) -> Result<(String, UserRecord), String> {
        let user = self.db.verify_user(username, password)?;
        let token = format!("tok_{}", Uuid::new_v4());
        self.sessions.write().insert(token.clone(), user.id.clone());
        Ok((token, user))
    }

    pub fn get_user_by_token(&self, token: &str) -> Option<UserRecord> {
        let user_id = self.sessions.read().get(token).cloned()?;
        self.db.get_user_by_id(&user_id)
    }

    pub fn get_user_id_by_token(&self, token: &str) -> Option<String> {
        self.sessions.read().get(token).cloned()
    }

    pub fn get_db(&self) -> &Database {
        &self.db
    }
}
