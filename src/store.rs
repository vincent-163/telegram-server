//! SQLite-backed persistence for users, chats, messages, files, auth keys and
//! the Bot API update queue.

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Canonical form of a phone number: digits only, so `+1 555 000 0001` and
/// `15550000001` resolve to the same account. Both registration and lookup go
/// through this so the stored value and the queried value always agree.
pub fn normalize_phone(p: &str) -> String {
    p.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Rebuilds the `messages` table when it still carries the old global
/// `id INTEGER PRIMARY KEY AUTOINCREMENT`. Message ids are per dialog, so a
/// database created before the composite key cannot store the first message of
/// a second dialog. Existing rows are preserved; their ids already came from a
/// per-dialog sequence, so no renumbering is needed.
fn migrate_messages_primary_key(conn: &Connection) -> Result<()> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'messages'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let Some(sql) = sql else { return Ok(()) };
    let normalised = sql.to_lowercase();
    let has_global_pk =
        normalised.contains("id integer primary key") || normalised.contains("autoincrement");
    if !has_global_pk {
        return Ok(());
    }
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         BEGIN;
         ALTER TABLE messages RENAME TO messages_legacy;
         CREATE TABLE messages (
             id INTEGER NOT NULL,
             dialog_type TEXT NOT NULL,
             dialog_id INTEGER NOT NULL,
             sender_id INTEGER NOT NULL,
             sender_chat_id INTEGER,
             date INTEGER NOT NULL,
             message TEXT NOT NULL DEFAULT '',
             media_kind TEXT NOT NULL DEFAULT '',
             media_file_id TEXT,
             edited INTEGER NOT NULL DEFAULT 0,
             reply_to INTEGER,
             random_id INTEGER NOT NULL DEFAULT 0,
             deleted INTEGER NOT NULL DEFAULT 0,
             PRIMARY KEY (dialog_type, dialog_id, id)
         );
         INSERT INTO messages (id,dialog_type,dialog_id,sender_id,sender_chat_id,date,message,
                               media_kind,media_file_id,edited,reply_to,random_id,deleted)
             SELECT id,dialog_type,dialog_id,sender_id,sender_chat_id,date,message,
                    media_kind,media_file_id,edited,reply_to,random_id,deleted
             FROM messages_legacy;
         DROP TABLE messages_legacy;
         CREATE INDEX IF NOT EXISTS messages_dialog ON messages(dialog_type, dialog_id, id DESC);
         COMMIT;
         PRAGMA foreign_keys = ON;",
    )
    .context("migrate messages primary key")?;
    Ok(())
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Default)]
pub struct UserRow {
    pub id: i64,
    pub access_hash: i64,
    pub phone: String,
    pub first_name: String,
    pub last_name: String,
    pub username: String,
    pub about: String,
    pub is_bot: bool,
    pub bot_token: String,
    pub photo_file_id: Option<String>,
    pub admin: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct ChatRow {
    pub id: i64,
    pub title: String,
    pub username: String,
    pub creator_id: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: i32,
    pub dialog_type: String,
    pub dialog_id: i64,
    pub sender_id: i64,
    pub sender_chat_id: Option<i64>,
    pub date: i64,
    pub message: String,
    pub media_kind: String,
    pub media_file_id: Option<String>,
    pub edited: bool,
    pub reply_to: Option<i32>,
    pub random_id: i64,
}

#[derive(Debug, Clone)]
pub struct DialogRow {
    pub dialog_type: String,
    pub dialog_id: i64,
    pub top_message: i32,
    pub unread_count: i32,
    pub read_max_id: i32,
    pub pinned: bool,
    pub updated_at: i64,
}

const SCHEMA: &str = r#"
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    access_hash INTEGER NOT NULL,
    phone TEXT NOT NULL DEFAULT '',
    first_name TEXT NOT NULL DEFAULT '',
    last_name TEXT NOT NULL DEFAULT '',
    username TEXT NOT NULL DEFAULT '',
    about TEXT NOT NULL DEFAULT '',
    is_bot INTEGER NOT NULL DEFAULT 0,
    bot_token TEXT NOT NULL DEFAULT '',
    photo_file_id TEXT,
    admin INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS users_phone ON users(phone) WHERE phone != '';
CREATE UNIQUE INDEX IF NOT EXISTS users_username ON users(username) WHERE username != '';
CREATE UNIQUE INDEX IF NOT EXISTS users_bot_token ON users(bot_token) WHERE bot_token != '';

CREATE TABLE IF NOT EXISTS chats (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    username TEXT NOT NULL DEFAULT '',
    creator_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_members (
    chat_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL DEFAULT 'member',
    joined_at INTEGER NOT NULL,
    PRIMARY KEY (chat_id, user_id)
);

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER NOT NULL,
    dialog_type TEXT NOT NULL,
    dialog_id INTEGER NOT NULL,
    sender_id INTEGER NOT NULL,
    sender_chat_id INTEGER,
    date INTEGER NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    media_kind TEXT NOT NULL DEFAULT '',
    media_file_id TEXT,
    edited INTEGER NOT NULL DEFAULT 0,
    reply_to INTEGER,
    random_id INTEGER NOT NULL DEFAULT 0,
    deleted INTEGER NOT NULL DEFAULT 0,
    -- Telegram numbers messages per dialog, so two dialogs each have a
    -- message with id 1. A global primary key on `id` alone cannot express
    -- that and makes the second dialog's first message collide.
    PRIMARY KEY (dialog_type, dialog_id, id)
);
CREATE INDEX IF NOT EXISTS messages_dialog ON messages(dialog_type, dialog_id, id DESC);

CREATE TABLE IF NOT EXISTS dialogs (
    user_id INTEGER NOT NULL,
    dialog_type TEXT NOT NULL,
    dialog_id INTEGER NOT NULL,
    top_message INTEGER NOT NULL DEFAULT 0,
    unread_count INTEGER NOT NULL DEFAULT 0,
    read_max_id INTEGER NOT NULL DEFAULT 0,
    pinned INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, dialog_type, dialog_id)
);

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mime TEXT NOT NULL DEFAULT 'application/octet-stream',
    size INTEGER NOT NULL,
    sha256 TEXT NOT NULL DEFAULT '',
    data BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS file_parts (
    file_id INTEGER NOT NULL,
    part INTEGER NOT NULL,
    data BLOB NOT NULL,
    PRIMARY KEY (file_id, part)
);

CREATE TABLE IF NOT EXISTS auth_keys (
    key_id INTEGER PRIMARY KEY,
    key BLOB NOT NULL,
    uid INTEGER,
    created_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS auth_sessions (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    auth_key_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    device_model TEXT NOT NULL DEFAULT '',
    platform TEXT NOT NULL DEFAULT '',
    api_id INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS login_codes (
    phone TEXT PRIMARY KEY,
    code TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS updates_state (
    user_id INTEGER PRIMARY KEY,
    pts INTEGER NOT NULL DEFAULT 1,
    qts INTEGER NOT NULL DEFAULT 0,
    seq INTEGER NOT NULL DEFAULT 1,
    date INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS update_queue (
    seqno INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS update_queue_user ON update_queue(user_id, seqno);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS bot_updates (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    bot_user_id INTEGER NOT NULL,
    update_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    consumed INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS bot_updates_chat ON bot_updates(chat_id, seq);

CREATE TABLE IF NOT EXISTS rate_limits (
    key TEXT NOT NULL,
    window INTEGER NOT NULL,
    count INTEGER NOT NULL,
    PRIMARY KEY (key, window)
);
"#;

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("open sqlite")?;
        conn.execute_batch(SCHEMA).context("apply schema")?;
        migrate_messages_primary_key(&conn)?;
        Ok(Store {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn conn(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.conn.lock()
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        Ok(rows.next()?.map(|r| r.get::<_, String>(0)).transpose()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---------------------------------------------------------------- users

    pub fn next_user_id(&self) -> Result<i64> {
        let conn = self.conn();
        let id: i64 =
            conn.query_row("SELECT COALESCE(MAX(id), 999999) + 1 FROM users", [], |r| {
                r.get(0)
            })?;
        Ok(id)
    }

    pub fn create_user(
        &self,
        phone: &str,
        first_name: &str,
        last_name: &str,
        username: &str,
        is_bot: bool,
        admin: bool,
    ) -> Result<UserRow> {
        let conn = self.conn();
        let phone = normalize_phone(phone);
        let id = conn.query_row("SELECT COALESCE(MAX(id), 999999) + 1 FROM users", [], |r| {
            r.get::<_, i64>(0)
        })?;
        let access_hash: i64 = rand::random();
        conn.execute(
            "INSERT INTO users(id,access_hash,phone,first_name,last_name,username,is_bot,admin,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                id,
                access_hash,
                phone,
                first_name,
                last_name,
                username,
                is_bot as i64,
                admin as i64,
                now()
            ],
        )?;
        drop(conn);
        self.get_user(id)?.ok_or_else(|| anyhow!("user vanished"))
    }

    pub fn get_user(&self, id: i64) -> Result<Option<UserRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id,access_hash,phone,first_name,last_name,username,about,is_bot,bot_token,
                    photo_file_id,admin,created_at FROM users WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(user_from_row(row)?));
        }
        Ok(None)
    }

    pub fn get_user_by_phone(&self, phone: &str) -> Result<Option<UserRow>> {
        let phone = normalize_phone(phone);
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id,access_hash,phone,first_name,last_name,username,about,is_bot,bot_token,
                    photo_file_id,admin,created_at FROM users WHERE phone = ?1",
        )?;
        let mut rows = stmt.query(params![phone])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(user_from_row(row)?));
        }
        Ok(None)
    }

    pub fn get_user_by_token(&self, token: &str) -> Result<Option<UserRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id,access_hash,phone,first_name,last_name,username,about,is_bot,bot_token,
                    photo_file_id,admin,created_at FROM users WHERE bot_token = ?1",
        )?;
        let mut rows = stmt.query(params![token])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(user_from_row(row)?));
        }
        Ok(None)
    }

    pub fn get_user_by_username(&self, username: &str) -> Result<Option<UserRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id,access_hash,phone,first_name,last_name,username,about,is_bot,bot_token,
                    photo_file_id,admin,created_at FROM users WHERE username = ?1",
        )?;
        let mut rows = stmt.query(params![username])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(user_from_row(row)?));
        }
        Ok(None)
    }

    pub fn get_users_by_ids(&self, ids: &[i64]) -> Result<Vec<UserRow>> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(u) = self.get_user(*id)? {
                out.push(u);
            }
        }
        Ok(out)
    }

    pub fn all_users(&self) -> Result<Vec<UserRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id,access_hash,phone,first_name,last_name,username,about,is_bot,bot_token,
                    photo_file_id,admin,created_at FROM users ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            user_from_row(row).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                    e.to_string(),
                )))
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn update_profile(
        &self,
        id: i64,
        first: Option<&str>,
        last: Option<&str>,
        about: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn();
        if let Some(v) = first {
            conn.execute(
                "UPDATE users SET first_name = ?1 WHERE id = ?2",
                params![v, id],
            )?;
        }
        if let Some(v) = last {
            conn.execute(
                "UPDATE users SET last_name = ?1 WHERE id = ?2",
                params![v, id],
            )?;
        }
        if let Some(v) = about {
            conn.execute("UPDATE users SET about = ?1 WHERE id = ?2", params![v, id])?;
        }
        Ok(())
    }

    pub fn set_username(&self, id: i64, username: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE users SET username = ?1 WHERE id = ?2",
            params![username, id],
        )?;
        Ok(())
    }

    pub fn set_bot_token(&self, id: i64, token: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE users SET bot_token = ?1 WHERE id = ?2",
            params![token, id],
        )?;
        Ok(())
    }

    // --------------------------------------------------------------- chats

    pub fn create_chat(&self, title: &str, creator_id: i64) -> Result<ChatRow> {
        let conn = self.conn();
        let id: i64 = conn.query_row(
            "SELECT COALESCE(MAX(id), -1000000000000) + 1 FROM chats",
            [],
            |r| r.get(0),
        )?;
        let created = now();
        conn.execute(
            "INSERT INTO chats(id,title,creator_id,created_at) VALUES(?1,?2,?3,?4)",
            params![id, title, creator_id, created],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO chat_members(chat_id,user_id,role,joined_at) VALUES(?1,?2,'owner',?3)",
            params![id, creator_id, created],
        )?;
        Ok(ChatRow {
            id,
            title: title.to_string(),
            username: String::new(),
            creator_id,
            created_at: created,
        })
    }

    pub fn get_chat(&self, id: i64) -> Result<Option<ChatRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id,title,username,creator_id,created_at FROM chats WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(ChatRow {
                id: row.get(0)?,
                title: row.get(1)?,
                username: row.get(2)?,
                creator_id: row.get(3)?,
                created_at: row.get(4)?,
            }));
        }
        Ok(None)
    }

    pub fn add_chat_member(&self, chat_id: i64, user_id: i64, role: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR IGNORE INTO chat_members(chat_id,user_id,role,joined_at) VALUES(?1,?2,?3,?4)",
            params![chat_id, user_id, role, now()],
        )?;
        Ok(())
    }

    pub fn chat_members(&self, chat_id: i64) -> Result<Vec<(i64, String)>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT user_id, role FROM chat_members WHERE chat_id = ?1")?;
        let rows = stmt.query_map(params![chat_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Chats that both users are members of.
    pub fn chat_members_shared(&self, a: i64, b: i64) -> Result<Vec<ChatRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT c.id,c.title,c.username,c.creator_id,c.created_at FROM chats c
             JOIN chat_members ma ON ma.chat_id = c.id AND ma.user_id = ?1
             JOIN chat_members mb ON mb.chat_id = c.id AND mb.user_id = ?2
             ORDER BY c.id",
        )?;
        let rows = stmt.query_map(params![a, b], |row| {
            Ok(ChatRow {
                id: row.get(0)?,
                title: row.get(1)?,
                username: row.get(2)?,
                creator_id: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn is_chat_member(&self, chat_id: i64, user_id: i64) -> Result<bool> {
        let conn = self.conn();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chat_members WHERE chat_id = ?1 AND user_id = ?2",
            params![chat_id, user_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    // ------------------------------------------------------------ dialogs

    pub fn upsert_dialog(&self, user_id: i64, kind: &str, id: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR IGNORE INTO dialogs(user_id,dialog_type,dialog_id,updated_at)
             VALUES(?1,?2,?3,?4)",
            params![user_id, kind, id, now()],
        )?;
        Ok(())
    }

    pub fn touch_dialog(
        &self,
        user_id: i64,
        kind: &str,
        id: i64,
        top: i32,
        bump_unread: bool,
    ) -> Result<()> {
        let conn = self.conn();
        self.upsert_dialog_inner(&conn, user_id, kind, id)?;
        conn.execute(
            "UPDATE dialogs SET top_message = MAX(top_message, ?4), unread_count = unread_count + ?5,
                updated_at = ?6
             WHERE user_id = ?1 AND dialog_type = ?2 AND dialog_id = ?3",
            params![user_id, kind, id, top, if bump_unread { 1 } else { 0 }, now()],
        )?;
        Ok(())
    }

    fn upsert_dialog_inner(
        &self,
        conn: &Connection,
        user_id: i64,
        kind: &str,
        id: i64,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO dialogs(user_id,dialog_type,dialog_id,updated_at)
             VALUES(?1,?2,?3,?4)",
            params![user_id, kind, id, now()],
        )?;
        Ok(())
    }

    pub fn dialogs_for(&self, user_id: i64) -> Result<Vec<DialogRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT dialog_type,dialog_id,top_message,unread_count,read_max_id,pinned,updated_at
             FROM dialogs WHERE user_id = ?1 ORDER BY pinned DESC, updated_at DESC",
        )?;
        let rows = stmt.query_map(params![user_id], |r| {
            Ok(DialogRow {
                dialog_type: r.get(0)?,
                dialog_id: r.get(1)?,
                top_message: r.get(2)?,
                unread_count: r.get(3)?,
                read_max_id: r.get(4)?,
                pinned: r.get::<_, i64>(5)? != 0,
                updated_at: r.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn set_dialog_read(&self, user_id: i64, kind: &str, id: i64, max_id: i32) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE dialogs SET read_max_id = MAX(read_max_id, ?4), unread_count = 0
             WHERE user_id = ?1 AND dialog_type = ?2 AND dialog_id = ?3",
            params![user_id, kind, id, max_id],
        )?;
        Ok(())
    }

    // ----------------------------------------------------------- messages

    pub fn next_message_id(&self, kind: &str, dialog_id: i64) -> Result<i32> {
        let conn = self.conn();
        let id: i64 = conn.query_row(
            "SELECT COALESCE(MAX(id), 0) + 1 FROM messages WHERE dialog_type = ?1 AND dialog_id = ?2",
            params![kind, dialog_id],
            |r| r.get::<_, i64>(0),
        )?;
        Ok(id as i32)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_message(
        &self,
        kind: &str,
        dialog_id: i64,
        sender_id: i64,
        sender_chat_id: Option<i64>,
        message: &str,
        media_kind: &str,
        media_file_id: Option<&str>,
        reply_to: Option<i32>,
        random_id: i64,
    ) -> Result<i32> {
        let conn = self.conn();
        let id: i64 = conn.query_row(
            "SELECT COALESCE(MAX(id), 0) + 1 FROM messages WHERE dialog_type = ?1 AND dialog_id = ?2",
            params![kind, dialog_id],
            |r| r.get::<_, i64>(0),
        )?;
        let id = id as i32;
        conn.execute(
            "INSERT INTO messages(id,dialog_type,dialog_id,sender_id,sender_chat_id,date,message,
                                  media_kind,media_file_id,reply_to,random_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                id,
                kind,
                dialog_id,
                sender_id,
                sender_chat_id,
                now(),
                message,
                media_kind,
                media_file_id,
                reply_to,
                random_id
            ],
        )?;
        Ok(id)
    }

    pub fn get_message(&self, kind: &str, dialog_id: i64, id: i32) -> Result<Option<MessageRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id,dialog_type,dialog_id,sender_id,sender_chat_id,date,message,media_kind,
                    media_file_id,edited,reply_to,random_id
             FROM messages WHERE dialog_type = ?1 AND dialog_id = ?2 AND id = ?3",
        )?;
        let mut rows = stmt.query(params![kind, dialog_id, id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(message_from_row(row)?));
        }
        Ok(None)
    }

    pub fn history(
        &self,
        kind: &str,
        dialog_id: i64,
        limit: i32,
        before_id: i32,
    ) -> Result<Vec<MessageRow>> {
        let conn = self.conn();
        let limit = limit.clamp(1, 500);
        let mut stmt = conn.prepare_cached(
            "SELECT id,dialog_type,dialog_id,sender_id,sender_chat_id,date,message,media_kind,
                    media_file_id,edited,reply_to,random_id
             FROM messages WHERE dialog_type = ?1 AND dialog_id = ?2 AND deleted = 0
                AND (?3 = 0 OR id < ?3)
             ORDER BY id DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![kind, dialog_id, before_id, limit], |r| {
            message_from_row(r).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                    e.to_string(),
                )))
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn edit_message(&self, kind: &str, dialog_id: i64, id: i32, text: &str) -> Result<bool> {
        let conn = self.conn();
        let n = conn.execute(
            "UPDATE messages SET message = ?4, edited = 1 WHERE dialog_type = ?1 AND dialog_id = ?2 AND id = ?3",
            params![kind, dialog_id, id, text],
        )?;
        Ok(n > 0)
    }

    pub fn delete_messages(&self, kind: &str, dialog_id: i64, ids: &[i32]) -> Result<usize> {
        let conn = self.conn();
        let mut n = 0;
        for id in ids {
            n += conn.execute(
                "UPDATE messages SET deleted = 1 WHERE dialog_type = ?1 AND dialog_id = ?2 AND id = ?3",
                params![kind, dialog_id, id],
            )?;
        }
        Ok(n)
    }

    pub fn message_count(&self, kind: &str, dialog_id: i64) -> Result<i64> {
        let conn = self.conn();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE dialog_type = ?1 AND dialog_id = ?2 AND deleted = 0",
            params![kind, dialog_id],
            |r| r.get::<_, i64>(0),
        )?)
    }

    /// Full-text search over message bodies, optionally restricted to one
    /// dialog and/or one sender.
    pub fn search_messages(
        &self,
        kind: Option<&str>,
        dialog_id: Option<i64>,
        sender_id: Option<i64>,
        query: &str,
        limit: i32,
        offset_id: i32,
    ) -> Result<Vec<MessageRow>> {
        let conn = self.conn();
        let limit = limit.clamp(1, 500);
        let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = conn.prepare_cached(
            "SELECT id,dialog_type,dialog_id,sender_id,sender_chat_id,date,message,media_kind,
                    media_file_id,edited,reply_to,random_id
             FROM messages WHERE deleted = 0
                AND (?1 = '' OR dialog_type = ?1)
                AND (?2 = 0 OR dialog_id = ?2)
                AND (?3 = 0 OR sender_id = ?3)
                AND (?4 = 0 OR id < ?4)
                AND message LIKE ?5 ESCAPE '\\'
             ORDER BY date DESC, id DESC LIMIT ?6",
        )?;
        let rows = stmt.query_map(
            params![
                kind.unwrap_or(""),
                dialog_id.unwrap_or(0),
                sender_id.unwrap_or(0),
                offset_id,
                pattern,
                limit
            ],
            |r| {
                message_from_row(r).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                        e.to_string(),
                    )))
                })
            },
        )?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Total number of live messages in one dialog.
    pub fn dialog_message_count(&self, kind: &str, dialog_id: i64) -> Result<i32> {
        Ok(self.message_count(kind, dialog_id)? as i32)
    }

    /// Messages newer than `min_id` in one dialog, oldest first.
    pub fn messages_after(
        &self,
        kind: &str,
        dialog_id: i64,
        min_id: i32,
    ) -> Result<Vec<MessageRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id,dialog_type,dialog_id,sender_id,sender_chat_id,date,message,media_kind,
                    media_file_id,edited,reply_to,random_id
             FROM messages WHERE dialog_type = ?1 AND dialog_id = ?2 AND deleted = 0 AND id > ?3
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![kind, dialog_id, min_id], |r| {
            message_from_row(r).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                    e.to_string(),
                )))
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ------------------------------------------------------- login sessions

    /// Active authorization sessions for one user, newest first.
    pub fn sessions_for_user(
        &self,
        user_id: i64,
    ) -> Result<Vec<(String, i64, i64, String, String, i32)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT id,auth_key_id,created_at,device_model,platform,api_id
             FROM auth_sessions WHERE user_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![user_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i32>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn drop_session(&self, user_id: i64, session_id: &str) -> Result<bool> {
        let conn = self.conn();
        let n = conn.execute(
            "DELETE FROM auth_sessions WHERE user_id = ?1 AND id = ?2",
            params![user_id, session_id],
        )?;
        Ok(n > 0)
    }

    pub fn drop_all_sessions(&self, user_id: i64, keep: Option<&str>) -> Result<usize> {
        let conn = self.conn();
        let n = match keep {
            Some(id) => conn.execute(
                "DELETE FROM auth_sessions WHERE user_id = ?1 AND id != ?2",
                params![user_id, id],
            )?,
            None => conn.execute(
                "DELETE FROM auth_sessions WHERE user_id = ?1",
                params![user_id],
            )?,
        };
        Ok(n)
    }

    pub fn count_sessions(&self, user_id: i64) -> Result<i32> {
        let conn = self.conn();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM auth_sessions WHERE user_id = ?1",
            params![user_id],
            |r| r.get::<_, i32>(0),
        )?)
    }

    // ------------------------------------------------- notify settings

    /// Per-peer notification settings, stored as an opaque JSON payload.
    pub fn get_notify_settings(&self, user_id: i64, peer: &str) -> Result<Option<String>> {
        self.setting(&format!("notify:{user_id}:{peer}"))
    }

    pub fn set_notify_settings(&self, user_id: i64, peer: &str, json: &str) -> Result<()> {
        self.set_setting(&format!("notify:{user_id}:{peer}"), json)
    }

    /// Per-peer notification settings for every peer the user touched.
    pub fn all_notify_settings(&self, user_id: i64) -> Result<Vec<(String, String)>> {
        let conn = self.conn();
        let prefix = format!("notify:{user_id}:");
        let mut stmt =
            conn.prepare_cached("SELECT key,value FROM settings WHERE key LIKE ?1 ESCAPE '\\'")?;
        let rows = stmt.query_map(params![format!("{prefix}%")], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (key, value) = r?;
            if let Some(peer) = key.strip_prefix(&prefix) {
                out.push((peer.to_string(), value));
            }
        }
        Ok(out)
    }

    // --------------------------------------------------------- upload blobs

    /// Blob id sequence used by `upload.saveFilePart` / `upload.getFile`.
    pub fn next_blob_id(&self) -> Result<i64> {
        let conn = self.conn();
        let cur: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'next_blob_id'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let next = cur.and_then(|v| v.parse::<i64>().ok()).unwrap_or(1);
        conn.execute(
            "INSERT OR REPLACE INTO settings(key,value) VALUES('next_blob_id',?1)",
            params![(next + 1).to_string()],
        )?;
        Ok(next)
    }

    pub fn put_blob(&self, id: &str, name: &str, mime: &str, data: &[u8]) -> Result<()> {
        self.put_file(id, name, mime, data, "")
    }

    pub fn get_blob(&self, id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.get_file(id)?.map(|(_, _, data)| data))
    }

    pub fn blob_size(&self, id: &str) -> Result<Option<i64>> {
        let conn = self.conn();
        let row: Option<i64> = conn
            .query_row("SELECT size FROM files WHERE id = ?1", params![id], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(row)
    }

    /// Sticker sets known to this server (empty by default).
    pub fn sticker_sets(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    // ------------------------------------------------------------- files

    pub fn put_file(
        &self,
        id: &str,
        name: &str,
        mime: &str,
        data: &[u8],
        sha256: &str,
    ) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO files(id,name,mime,size,sha256,data) VALUES(?1,?2,?3,?4,?5,?6)",
            params![id, name, mime, data.len() as i64, sha256, data],
        )?;
        Ok(())
    }

    pub fn get_file(&self, id: &str) -> Result<Option<(String, String, Vec<u8>)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT name,mime,data FROM files WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?)));
        }
        Ok(None)
    }

    pub fn put_file_part(&self, file_id: i64, part: i32, data: &[u8]) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO file_parts(file_id,part,data) VALUES(?1,?2,?3)",
            params![file_id, part, data],
        )?;
        Ok(())
    }

    pub fn assemble_file(&self, file_id: i64, total: i32) -> Result<Vec<u8>> {
        let conn = self.conn();
        let mut out = Vec::new();
        for part in 0..total {
            let row: Option<Vec<u8>> = conn
                .query_row(
                    "SELECT data FROM file_parts WHERE file_id = ?1 AND part = ?2",
                    params![file_id, part],
                    |r| r.get(0),
                )
                .optional()?;
            match row {
                Some(d) => out.extend_from_slice(&d),
                None => return Err(anyhow!("missing file part {}", part)),
            }
        }
        Ok(out)
    }

    // --------------------------------------------------------- auth keys

    pub fn save_auth_key(&self, key_id: i64, key: &[u8]) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO auth_keys(key_id,key,created_at,last_seen) VALUES(?1,?2,?3,?4)",
            params![key_id, key, now(), now()],
        )?;
        Ok(())
    }

    pub fn load_auth_key(&self, key_id: i64) -> Result<Option<[u8; 256]>> {
        let conn = self.conn();
        let row: Option<Vec<u8>> = conn
            .query_row(
                "SELECT key FROM auth_keys WHERE key_id = ?1",
                params![key_id],
                |r| r.get(0),
            )
            .optional()?;
        match row {
            Some(v) if v.len() == 256 => {
                let mut out = [0u8; 256];
                out.copy_from_slice(&v);
                Ok(Some(out))
            }
            _ => Ok(None),
        }
    }

    // ----------------------------------------------------------- sessions

    pub fn save_session(
        &self,
        session_id: &str,
        user_id: i64,
        auth_key_id: i64,
        device_model: &str,
        platform: &str,
        api_id: i32,
        ttl: i64,
    ) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO auth_sessions(id,user_id,auth_key_id,created_at,expires_at,device_model,platform,api_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                session_id,
                user_id,
                auth_key_id,
                now(),
                now() + ttl,
                device_model,
                platform,
                api_id
            ],
        )?;
        Ok(())
    }

    /// Remembers the TL layer a client announced via `invokeWithLayer`, so
    /// replies can be downgraded to a schema that client can parse. The layer
    /// is carried by the client, not the request, and every later message is
    /// dispatched from a fresh context, so it has to be persisted.
    pub fn set_session_layer(&self, auth_key_id: i64, layer: i32) -> Result<()> {
        self.set_setting(&format!("layer:{}", auth_key_id), &layer.to_string())
    }

    pub fn session_layer(&self, auth_key_id: i64) -> Result<Option<i32>> {
        Ok(self
            .setting(&format!("layer:{}", auth_key_id))?
            .and_then(|v| v.parse().ok()))
    }

    pub fn session_user(&self, auth_key_id: i64) -> Result<Option<i64>> {
        let conn = self.conn();
        let row: Option<i64> = conn
            .query_row(
                "SELECT user_id FROM auth_sessions WHERE auth_key_id = ?1 AND expires_at > ?2
                 ORDER BY created_at DESC LIMIT 1",
                params![auth_key_id, now()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(row)
    }

    pub fn device_info(&self, auth_key_id: i64) -> Result<(String, String, i32)> {
        let conn = self.conn();
        let row = conn
            .query_row(
                "SELECT device_model, platform, api_id FROM auth_sessions
                 WHERE auth_key_id = ?1 ORDER BY created_at DESC LIMIT 1",
                params![auth_key_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i32>(2)?,
                    ))
                },
            )
            .optional()?;
        Ok(row.unwrap_or_else(|| ("".into(), "".into(), 0)))
    }

    // -------------------------------------------------------- login codes

    pub fn issue_login_code(&self, phone: &str, code: &str, ttl: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO login_codes(phone,code,created_at,expires_at,attempts)
             VALUES(?1,?2,?3,?4,0)
             ON CONFLICT(phone) DO UPDATE SET code=excluded.code, created_at=excluded.created_at,
                expires_at=excluded.expires_at, attempts=0",
            params![phone, code, now(), now() + ttl],
        )?;
        Ok(())
    }

    pub fn check_login_code(&self, phone: &str, code: &str) -> Result<bool> {
        let conn = self.conn();
        let row: Option<(String, i64, i64, i64)> = conn
            .query_row(
                "SELECT code, expires_at, attempts, created_at FROM login_codes WHERE phone = ?1",
                params![phone],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        match row {
            Some((stored, expires, attempts, _)) if expires > now() && attempts < 5 => {
                conn.execute(
                    "UPDATE login_codes SET attempts = attempts + 1 WHERE phone = ?1",
                    params![phone],
                )?;
                Ok(stored == code)
            }
            _ => Ok(false),
        }
    }

    // ----------------------------------------------------------- updates

    pub fn state_for(&self, user_id: i64) -> Result<(i32, i32, i32, i32)> {
        let conn = self.conn();
        let row = conn
            .query_row(
                "SELECT pts,qts,seq,date FROM updates_state WHERE user_id = ?1",
                params![user_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        if let Some(row) = row {
            return Ok(row);
        }
        conn.execute(
            "INSERT INTO updates_state(user_id,pts,qts,seq,date) VALUES(?1,1,0,1,?2)",
            params![user_id, now() as i32],
        )?;
        Ok((1, 0, 1, now() as i32))
    }

    pub fn bump_pts(&self, user_id: i64) -> Result<(i32, i32)> {
        let conn = self.conn();
        // Read the current pts inline: `state_for` takes the same
        // non-reentrant connection guard, so calling it here would deadlock.
        let existing: Option<i32> = conn
            .query_row(
                "SELECT pts FROM updates_state WHERE user_id = ?1",
                params![user_id],
                |r| r.get(0),
            )
            .optional()?;
        let pts = match existing {
            Some(pts) => pts,
            None => {
                conn.execute(
                    "INSERT INTO updates_state(user_id,pts,qts,seq,date) VALUES(?1,1,0,1,?2)",
                    params![user_id, now() as i32],
                )?;
                1
            }
        };
        let new_pts = pts + 1;
        conn.execute(
            "UPDATE updates_state SET pts = ?1, date = ?2 WHERE user_id = ?3",
            params![new_pts, now() as i32, user_id],
        )?;
        Ok((new_pts, new_pts - pts))
    }

    pub fn push_update(&self, user_id: i64, kind: &str, payload: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO update_queue(user_id,kind,payload,created_at) VALUES(?1,?2,?3,?4)",
            params![user_id, kind, payload, now()],
        )?;
        Ok(())
    }

    // ------------------------------------------------------- bot updates

    #[allow(clippy::too_many_arguments)]
    pub fn push_bot_update(
        &self,
        chat_id: i64,
        user_id: i64,
        bot_user_id: i64,
        kind: &str,
        payload: &str,
    ) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO bot_updates(chat_id,user_id,bot_user_id,update_type,payload,created_at)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![chat_id, user_id, bot_user_id, kind, payload, now()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn bot_updates(
        &self,
        bot_user_id: i64,
        chat_id: Option<i64>,
        limit: i32,
        offset: i64,
    ) -> Result<Vec<(i64, i64, i64, String, String)>> {
        let conn = self.conn();
        let mut out = Vec::new();
        {
            let mut stmt = conn.prepare_cached(
                "SELECT seq,chat_id,user_id,update_type,payload FROM bot_updates
                 WHERE bot_user_id = ?1 AND consumed = 0 AND seq > ?2
                   AND (?3 IS NULL OR chat_id = ?3)
                 ORDER BY seq LIMIT ?4",
            )?;
            let rows = stmt.query_map(params![bot_user_id, offset, chat_id, limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    }

    pub fn mark_bot_updates_consumed(&self, ids: &[i64]) -> Result<()> {
        let conn = self.conn();
        for id in ids {
            conn.execute(
                "UPDATE bot_updates SET consumed = 1 WHERE seq = ?1",
                params![id],
            )?;
        }
        Ok(())
    }

    pub fn bot_chats(&self, bot_user_id: i64) -> Result<Vec<i64>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT DISTINCT chat_id FROM bot_updates WHERE bot_user_id = ?1")?;
        let rows = stmt.query_map(params![bot_user_id], |r| r.get(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

fn user_from_row(row: &rusqlite::Row<'_>) -> Result<UserRow> {
    Ok(UserRow {
        id: row.get(0)?,
        access_hash: row.get(1)?,
        phone: row.get(2)?,
        first_name: row.get(3)?,
        last_name: row.get(4)?,
        username: row.get(5)?,
        about: row.get(6)?,
        is_bot: row.get::<_, i64>(7)? != 0,
        bot_token: row.get(8)?,
        photo_file_id: row.get(9)?,
        admin: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
    })
}

fn message_from_row(row: &rusqlite::Row<'_>) -> Result<MessageRow> {
    Ok(MessageRow {
        id: row.get(0)?,
        dialog_type: row.get(1)?,
        dialog_id: row.get(2)?,
        sender_id: row.get(3)?,
        sender_chat_id: row.get(4)?,
        date: row.get(5)?,
        message: row.get(6)?,
        media_kind: row.get(7)?,
        media_file_id: row.get(8)?,
        edited: row.get::<_, i64>(9)? != 0,
        reply_to: row.get(10)?,
        random_id: row.get(11)?,
    })
}
