use crate::db;
use crate::services::utils::{new_id, now_str};
use rand::Rng;
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub email: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub user: Option<AuthUser>,
    pub access_token: Option<String>,
}

fn ensure_tables() {
    db::with_db(|conn| {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                created_at TEXT NOT NULL
            );",
        )
        .ok();
    });
}

fn generate_token() -> String {
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    token
}

fn get_user_by_email(email: &str) -> Result<AuthUser, String> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, email, username FROM users WHERE email = ?",
            params![email],
            |row| {
                Ok(AuthUser {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    username: row.get(2)?,
                })
            },
        )
        .map_err(|_| "用户不存在".to_string())
    })
}

fn get_user_by_username(username: &str) -> Result<AuthUser, String> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, email, username FROM users WHERE username = ?",
            params![username],
            |row| {
                Ok(AuthUser {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    username: row.get(2)?,
                })
            },
        )
        .map_err(|_| "用户不存在".to_string())
    })
}

fn create_session(user_id: &str) -> Result<String, String> {
    let token = generate_token();
    let now = now_str();
    db::with_db(|conn| {
        conn.execute(
            "INSERT INTO sessions (token, user_id, created_at) VALUES (?, ?, ?)",
            params![token, user_id, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(token)
    })
}

#[allow(dead_code)]
fn get_user_by_session(token: &str) -> Result<AuthUser, String> {
    db::with_db(|conn| {
        let user_id: String = conn
            .query_row(
                "SELECT user_id FROM sessions WHERE token = ?",
                params![token],
                |row| row.get(0),
            )
            .map_err(|_| "会话无效".to_string())?;
        get_user_by_email_internal(conn, &user_id)
    })
}

#[allow(dead_code)]
fn get_user_by_email_internal(conn: &rusqlite::Connection, user_id: &str) -> Result<AuthUser, String> {
    conn.query_row(
        "SELECT id, email, username FROM users WHERE id = ?",
        params![user_id],
        |row| {
            Ok(AuthUser {
                id: row.get(0)?,
                email: row.get(1)?,
                username: row.get(2)?,
            })
        },
    )
    .map_err(|_| "用户不存在".to_string())
}

pub fn sign_up(email: &str, password: &str, username: &str) -> Result<AuthSession, String> {
    if email.is_empty() || password.is_empty() || username.is_empty() {
        return Err("邮箱、密码和用户名不能为空".to_string());
    }
    if password.len() < 6 {
        return Err("密码长度不能少于 6 位".to_string());
    }

    ensure_tables();

    let password_hash =
        bcrypt::hash(password, 10).map_err(|e| format!("加密失败: {}", e))?;
    let user_id = new_id();
    let now = now_str();

    db::with_db(|conn| {
        conn.execute(
            "INSERT INTO users (id, email, username, password_hash, created_at) VALUES (?, ?, ?, ?, ?)",
            params![user_id, email, username, password_hash, now],
        )
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                "邮箱或用户名已存在".to_string()
            } else {
                format!("注册失败: {}", msg)
            }
        })?;
        Ok::<(), String>(())
    })?;

    let token = create_session(&user_id)?;

    Ok(AuthSession {
        user: Some(AuthUser {
            id: user_id,
            email: Some(email.to_string()),
            username: Some(username.to_string()),
        }),
        access_token: Some(token),
    })
}

pub fn sign_in(identifier: &str, password: &str) -> Result<AuthSession, String> {
    if identifier.is_empty() {
        return Err("请输入邮箱或用户名".to_string());
    }

    ensure_tables();

    // Try email first, then username
    let user = get_user_by_email(identifier).or_else(|_| get_user_by_username(identifier))?;

    let stored_hash: String = db::with_db(|conn| {
        conn.query_row(
            "SELECT password_hash FROM users WHERE id = ?",
            params![user.id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
    })?;

    let valid = bcrypt::verify(password, &stored_hash).map_err(|e| e.to_string())?;
    if !valid {
        return Err("密码错误".to_string());
    }

    let token = create_session(&user.id)?;

    Ok(AuthSession {
        user: Some(AuthUser {
            id: user.id.clone(),
            email: user.email.clone(),
            username: user.username.clone(),
        }),
        access_token: Some(token),
    })
}

pub fn sign_out() -> Result<(), String> {
    // Session cleanup handled by frontend clearing localStorage
    Ok(())
}

pub fn get_current_user() -> Result<Option<AuthUser>, String> {
    Ok(None)
}

pub fn get_session() -> Result<Option<AuthSession>, String> {
    Ok(None)
}
