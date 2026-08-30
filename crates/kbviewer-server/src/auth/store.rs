//! Users and sessions, persisted as JSON.
//!
//! Sessions live server-side rather than in a signed token so they can be revoked: losing
//! a phone should be recoverable by killing the session, which a self-contained JWT does
//! not allow. Both files are written atomically and `users.json` is chmod 600, because it
//! holds password hashes.

use crate::auth::passwords::{hash_password, random_token, verify_password, PasswordError};
use crate::auth::rate_limit::RateLimiter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

const SESSION_LIFETIME_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Only rewrite a session's expiry after this much elapsed, so an active browser does not
/// cause a disk write on every request.
const SESSION_REFRESH_AFTER_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("no account for that email")]
    NoSuchUser,
    #[error("an account with that email already exists")]
    DuplicateEmail,
    #[error(transparent)]
    Password(#[from] PasswordError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("could not parse {0}: {1}")]
    Corrupt(PathBuf, String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub created_at: i64,
    pub expires_at: i64,
}

pub struct AuthStore {
    dir: PathBuf,
    users: RwLock<Vec<User>>,
    sessions: RwLock<HashMap<String, Session>>,
    pub limiter: RateLimiter,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn normalise_email(email: &str) -> String {
    email.trim().to_lowercase()
}

impl AuthStore {
    pub fn open(dir: &Path) -> Result<Self, AuthError> {
        std::fs::create_dir_all(dir)?;
        let users: Vec<User> = read_json(&dir.join("users.json"))?.unwrap_or_default();
        let sessions: HashMap<String, Session> =
            read_json(&dir.join("sessions.json"))?.unwrap_or_default();

        let now = now_ms();
        let live = sessions
            .into_iter()
            .filter(|(_, session)| session.expires_at > now)
            .collect();

        Ok(Self {
            dir: dir.to_path_buf(),
            users: RwLock::new(users),
            sessions: RwLock::new(live),
            limiter: RateLimiter::new(),
        })
    }

    pub fn user_count(&self) -> usize {
        self.users.read().unwrap().len()
    }

    pub fn list_users(&self) -> Vec<User> {
        self.users.read().unwrap().clone()
    }

    pub fn add_user(&self, email: &str, password: &str) -> Result<User, AuthError> {
        let email = normalise_email(email);
        if self.find_by_email(&email).is_some() {
            return Err(AuthError::DuplicateEmail);
        }
        let user = User {
            id: random_token(),
            email,
            password_hash: hash_password(password)?,
            created_at: now_ms(),
        };
        self.users.write().unwrap().push(user.clone());
        self.persist_users()?;
        Ok(user)
    }

    pub fn set_password(&self, email: &str, password: &str) -> Result<(), AuthError> {
        let email = normalise_email(email);
        let hash = hash_password(password)?;
        {
            let mut users = self.users.write().unwrap();
            let user = users
                .iter_mut()
                .find(|u| u.email == email)
                .ok_or(AuthError::NoSuchUser)?;
            user.password_hash = hash;
        }
        self.persist_users()
    }

    pub fn remove_user(&self, email: &str) -> Result<(), AuthError> {
        let email = normalise_email(email);
        let removed = {
            let mut users = self.users.write().unwrap();
            let before = users.len();
            users.retain(|u| u.email != email);
            before != users.len()
        };
        if !removed {
            return Err(AuthError::NoSuchUser);
        }
        self.persist_users()?;
        // Sessions belonging to a deleted account must not outlive it.
        self.revoke_all_sessions()
    }

    pub fn find_by_email(&self, email: &str) -> Option<User> {
        let email = normalise_email(email);
        self.users
            .read()
            .unwrap()
            .iter()
            .find(|u| u.email == email)
            .cloned()
    }

    /// Verify credentials. Returns `None` for both "no such user" and "wrong password",
    /// and costs the same either way — see `passwords::decoy_hash`.
    pub fn authenticate(&self, email: &str, password: &str) -> Option<User> {
        match self.find_by_email(email) {
            Some(user) if verify_password(password, &user.password_hash) => Some(user),
            Some(_) => None,
            None => {
                verify_password(password, crate::auth::passwords::decoy_hash());
                None
            }
        }
    }

    pub fn create_session(&self, user_id: &str) -> Result<Session, AuthError> {
        let now = now_ms();
        let session = Session {
            id: random_token(),
            user_id: user_id.to_string(),
            created_at: now,
            expires_at: now + SESSION_LIFETIME_MS,
        };
        self.sessions
            .write()
            .unwrap()
            .insert(session.id.clone(), session.clone());
        self.persist_sessions()?;
        Ok(session)
    }

    /// Look up a session, dropping it if expired and sliding its expiry if it is old
    /// enough to be worth rewriting.
    pub fn session_user(&self, session_id: &str) -> Option<User> {
        let now = now_ms();
        let (user_id, needs_refresh) = {
            let sessions = self.sessions.read().unwrap();
            let session = sessions.get(session_id)?;
            if session.expires_at <= now {
                None
            } else {
                Some((
                    session.user_id.clone(),
                    session.expires_at - now < SESSION_LIFETIME_MS - SESSION_REFRESH_AFTER_MS,
                ))
            }
        }?;

        let user = self
            .users
            .read()
            .unwrap()
            .iter()
            .find(|u| u.id == user_id)
            .cloned();
        if user.is_none() {
            // The account was deleted; the session is dead with it.
            self.destroy_session(session_id).ok();
            return None;
        }

        if needs_refresh {
            if let Some(session) = self.sessions.write().unwrap().get_mut(session_id) {
                session.expires_at = now + SESSION_LIFETIME_MS;
            }
            self.persist_sessions().ok();
        }
        user
    }

    pub fn destroy_session(&self, session_id: &str) -> Result<(), AuthError> {
        self.sessions.write().unwrap().remove(session_id);
        self.persist_sessions()
    }

    pub fn revoke_all_sessions(&self) -> Result<(), AuthError> {
        self.sessions.write().unwrap().clear();
        self.persist_sessions()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.read().unwrap().len()
    }

    fn persist_users(&self) -> Result<(), AuthError> {
        let users = self.users.read().unwrap();
        write_json_private(&self.dir.join("users.json"), &*users)
    }

    fn persist_sessions(&self) -> Result<(), AuthError> {
        let sessions = self.sessions.read().unwrap();
        write_json_private(&self.dir.join("sessions.json"), &*sessions)
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, AuthError> {
    match std::fs::read_to_string(path) {
        Ok(raw) if raw.trim().is_empty() => Ok(None),
        Ok(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| AuthError::Corrupt(path.to_path_buf(), e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Write atomically and restrict to the owner: this file holds password hashes and
/// live session ids, so a partial write or a world-readable mode would both be bugs.
fn write_json_private<T: Serialize>(path: &Path, value: &T) -> Result<(), AuthError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temp, value)
        .map_err(|e| AuthError::Corrupt(path.to_path_buf(), e.to_string()))?;
    temp.as_file().sync_all()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    temp.persist(path).map_err(|e| AuthError::Io(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(label: &str) -> AuthStore {
        let dir = std::env::temp_dir().join(format!("kbviewer-auth-{label}"));
        let _ = std::fs::remove_dir_all(&dir);
        AuthStore::open(&dir).unwrap()
    }

    #[test]
    fn a_new_user_can_authenticate() {
        let store = store("basic");
        store.add_user("me@example.com", "hunter2").unwrap();
        assert!(store.authenticate("me@example.com", "hunter2").is_some());
        assert!(store.authenticate("me@example.com", "wrong").is_none());
    }

    #[test]
    fn emails_are_normalised_on_both_sides() {
        let store = store("normalise");
        store.add_user("  Me@Example.COM ", "pw").unwrap();
        assert!(store.authenticate("me@example.com", "pw").is_some());
        assert!(store.authenticate("ME@EXAMPLE.COM", "pw").is_some());
    }

    #[test]
    fn duplicate_emails_are_refused() {
        let store = store("dup");
        store.add_user("a@b.c", "pw").unwrap();
        assert!(matches!(
            store.add_user("A@B.C", "pw2"),
            Err(AuthError::DuplicateEmail)
        ));
    }

    #[test]
    fn an_unknown_email_does_not_authenticate() {
        let store = store("unknown");
        assert!(store.authenticate("nobody@example.com", "pw").is_none());
    }

    #[test]
    fn sessions_round_trip_and_can_be_destroyed() {
        let store = store("session");
        let user = store.add_user("a@b.c", "pw").unwrap();
        let session = store.create_session(&user.id).unwrap();
        assert_eq!(store.session_user(&session.id).unwrap().id, user.id);
        store.destroy_session(&session.id).unwrap();
        assert!(store.session_user(&session.id).is_none());
    }

    #[test]
    fn an_unknown_session_id_resolves_to_nobody() {
        let store = store("badsession");
        assert!(store.session_user("not-a-real-session").is_none());
    }

    #[test]
    fn users_and_sessions_survive_a_restart() {
        let dir = std::env::temp_dir().join("kbviewer-auth-persist");
        let _ = std::fs::remove_dir_all(&dir);

        let session_id = {
            let store = AuthStore::open(&dir).unwrap();
            let user = store.add_user("a@b.c", "pw").unwrap();
            store.create_session(&user.id).unwrap().id
        };

        let reopened = AuthStore::open(&dir).unwrap();
        assert!(
            reopened.authenticate("a@b.c", "pw").is_some(),
            "user must persist"
        );
        assert!(
            reopened.session_user(&session_id).is_some(),
            "a restart must not log you out"
        );
    }

    #[test]
    fn revoking_kills_every_session() {
        let store = store("revoke");
        let user = store.add_user("a@b.c", "pw").unwrap();
        let one = store.create_session(&user.id).unwrap();
        let two = store.create_session(&user.id).unwrap();
        store.revoke_all_sessions().unwrap();
        assert!(store.session_user(&one.id).is_none());
        assert!(store.session_user(&two.id).is_none());
    }

    #[test]
    fn deleting_a_user_invalidates_their_sessions() {
        let store = store("delete");
        let user = store.add_user("a@b.c", "pw").unwrap();
        let session = store.create_session(&user.id).unwrap();
        store.remove_user("a@b.c").unwrap();
        assert!(
            store.session_user(&session.id).is_none(),
            "a deleted account must not stay logged in"
        );
    }

    #[test]
    fn changing_a_password_invalidates_the_old_one() {
        let store = store("passwd");
        store.add_user("a@b.c", "old").unwrap();
        store.set_password("a@b.c", "new").unwrap();
        assert!(store.authenticate("a@b.c", "old").is_none());
        assert!(store.authenticate("a@b.c", "new").is_some());
    }

    #[test]
    fn an_expired_session_is_not_accepted() {
        let dir = std::env::temp_dir().join("kbviewer-auth-expired");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let store = AuthStore::open(&dir).unwrap();
        let user = store.add_user("a@b.c", "pw").unwrap();
        let session = store.create_session(&user.id).unwrap();

        // Rewrite the stored session so it is already in the past, then reopen.
        let mut sessions: HashMap<String, Session> =
            serde_json::from_str(&std::fs::read_to_string(dir.join("sessions.json")).unwrap())
                .unwrap();
        sessions.get_mut(&session.id).unwrap().expires_at = now_ms() - 1000;
        std::fs::write(
            dir.join("sessions.json"),
            serde_json::to_string(&sessions).unwrap(),
        )
        .unwrap();

        let reopened = AuthStore::open(&dir).unwrap();
        assert!(reopened.session_user(&session.id).is_none());
        assert_eq!(
            reopened.session_count(),
            0,
            "expired sessions are dropped at load"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_user_file_is_not_world_readable() {
        use std::os::unix::fs::PermissionsExt;
        let store = store("perms");
        store.add_user("a@b.c", "pw").unwrap();
        let dir = std::env::temp_dir().join("kbviewer-auth-perms");
        let mode = std::fs::metadata(dir.join("users.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "password hashes must not be readable by other users"
        );
    }
}
