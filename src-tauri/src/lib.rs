use chrono::{Duration, Local, NaiveDate, Utc};
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension, ToSql};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use tauri::{Emitter, Manager};
use uuid::Uuid;

const DATABASE_FILE: &str = "token-statistics.sqlite";
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

pub struct Database {
    connection: Mutex<Connection>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl AppError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }

    fn validation(message: impl Into<String>) -> Self {
        Self::new("VALIDATION_ERROR", message)
    }
    fn database(error: impl std::fmt::Display) -> Self {
        Self::new("DATABASE_ERROR", error.to_string())
    }
}

type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    pub provider: Option<String>,
    pub color: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecord {
    pub id: String,
    pub usage_date: String,
    pub model_id: String,
    pub model_name: String,
    pub provider: Option<String>,
    pub color: Option<String>,
    pub token_count: i64,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUsageRecordInput {
    pub usage_date: String,
    pub model_id: String,
    pub token_count: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageFilter {
    #[serde(alias = "from_date")]
    pub from: Option<String>,
    #[serde(alias = "to_date")]
    pub to: Option<String>,
    pub model_ids: Option<Vec<String>>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSort {
    pub field: Option<String>,
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageRequest {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecordPage {
    pub items: Vec<UsageRecord>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DailyStats {
    pub date: String,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStats {
    pub model_id: String,
    pub model_name: String,
    pub provider: Option<String>,
    pub color: Option<String>,
    pub total_tokens: i64,
    pub percentage: f64,
    pub record_days: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub from: String,
    pub to: String,
    pub total_tokens: i64,
    pub average_daily_tokens: f64,
    pub highest_usage_day: Option<DailyStats>,
    pub active_model_count: i64,
    pub daily: Vec<DailyStats>,
    pub by_model: Vec<ModelStats>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_name: String,
    pub release_notes: String,
    pub release_date: String,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: Option<u64>,
    pub release_url: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub percentage: f64,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub success: bool,
    pub backup_path: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadata {
    pub version: String,
    pub app_data_dir: String,
    pub database_path: String,
    pub backup_count: usize,
}

fn parse_version_digits(v: &str) -> Vec<u64> {
    let clean = v.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    clean
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    let cur_parts = parse_version_digits(current);
    let cand_parts = parse_version_digits(candidate);
    let max_len = cur_parts.len().max(cand_parts.len());
    for i in 0..max_len {
        let cur = cur_parts.get(i).copied().unwrap_or(0);
        let cand = cand_parts.get(i).copied().unwrap_or(0);
        if cand > cur {
            return true;
        } else if cand < cur {
            return false;
        }
    }
    false
}

pub fn backup_database_file(data_dir: &Path) -> Result<std::path::PathBuf, AppError> {
    let db_path = data_dir.join(DATABASE_FILE);
    if !db_path.exists() {
        return Ok(db_path);
    }
    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(&backups_dir)
        .map_err(|e| AppError::database(format!("创建备份目录失败: {e}")))?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_filename = format!("token-statistics-pre-update-{timestamp}.sqlite");
    let backup_path = backups_dir.join(&backup_filename);

    std::fs::copy(&db_path, &backup_path)
        .map_err(|e| AppError::database(format!("备份数据库失败: {e}")))?;

    let fixed_bak = data_dir.join("token-statistics.sqlite.bak");
    let _ = std::fs::copy(&db_path, &fixed_bak);

    Ok(backup_path)
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn validate_date(value: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::validation(format!("日期必须是 YYYY-MM-DD 格式：{value}")))
}

fn validate_range(from: &str, to: &str) -> AppResult<(NaiveDate, NaiveDate)> {
    let start = validate_date(from)?;
    let end = validate_date(to)?;
    if start > end {
        return Err(AppError::validation("开始日期不能晚于结束日期"));
    }
    Ok((start, end))
}

fn validate_token_count(token_count: i64) -> AppResult<()> {
    if !(1..=MAX_SAFE_INTEGER).contains(&token_count) {
        return Err(AppError::validation(format!(
            "Token 数量必须是 1 到 {MAX_SAFE_INTEGER} 的整数"
        )));
    }
    Ok(())
}

fn normalize_required(value: &str, field: &str, max_len: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::validation(format!("{field}不能为空")));
    }
    if value.chars().count() > max_len {
        return Err(AppError::validation(format!(
            "{field}不能超过{max_len}个字符"
        )));
    }
    Ok(value.to_string())
}

fn normalize_optional(value: Option<String>, max_len: usize) -> AppResult<Option<String>> {
    match value {
        Some(v) => {
            let trimmed = v.trim().to_string();
            if trimmed.chars().count() > max_len {
                Err(AppError::validation(format!("文本不能超过{max_len}个字符")))
            } else if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        None => Ok(None),
    }
}

fn open_database(path: &Path) -> AppResult<Database> {
    let connection = Connection::open(path).map_err(AppError::database)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(AppError::database)?;
    migrate(&connection)?;
    Ok(Database {
        connection: Mutex::new(connection),
    })
}

fn migrate(connection: &Connection) -> AppResult<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY NOT NULL, applied_at TEXT NOT NULL);")
        .map_err(AppError::database)?;
    let version: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(AppError::database)?;
    if version < 1 {
        let transaction = connection
            .unchecked_transaction()
            .map_err(AppError::database)?;
        transaction
            .execute_batch(
                "CREATE TABLE models (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL COLLATE NOCASE UNIQUE,
                provider TEXT,
                color TEXT,
                is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE usage_records (
                id TEXT PRIMARY KEY NOT NULL,
                usage_date TEXT NOT NULL,
                model_id TEXT NOT NULL REFERENCES models(id) ON UPDATE CASCADE ON DELETE RESTRICT,
                token_count INTEGER NOT NULL CHECK (token_count > 0),
                notes TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(usage_date, model_id)
            );
            CREATE INDEX idx_usage_records_date ON usage_records(usage_date);
            CREATE INDEX idx_usage_records_model ON usage_records(model_id);
            CREATE INDEX idx_usage_records_date_model ON usage_records(usage_date, model_id);",
            )
            .map_err(AppError::database)?;
        let timestamp = now();
        for (id, name, provider, color) in [
            ("openai-gpt-4o", "GPT-4o", "OpenAI", "#10a37f"),
            (
                "anthropic-claude-3-5-sonnet",
                "Claude 3.5 Sonnet",
                "Anthropic",
                "#d97757",
            ),
            (
                "google-gemini-1-5-pro",
                "Gemini 1.5 Pro",
                "Google",
                "#4285f4",
            ),
        ] {
            transaction.execute("INSERT OR IGNORE INTO models (id, name, provider, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)", params![id, name, provider, color, timestamp])
                .map_err(AppError::database)?;
        }
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
                params![now()],
            )
            .map_err(AppError::database)?;
        transaction.commit().map_err(AppError::database)?;
    }
    Ok(())
}

fn lock_connection(state: &Database) -> AppResult<std::sync::MutexGuard<'_, Connection>> {
    state
        .connection
        .lock()
        .map_err(|_| AppError::new("DATABASE_ERROR", "数据库锁已损坏"))
}

fn model_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Model> {
    Ok(Model {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        color: row.get(3)?,
        is_active: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UsageRecord> {
    Ok(UsageRecord {
        id: row.get(0)?,
        usage_date: row.get(1)?,
        model_id: row.get(2)?,
        model_name: row.get(3)?,
        provider: row.get(4)?,
        color: row.get(5)?,
        token_count: row.get(6)?,
        notes: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

mod commands {
    use super::*;

    #[tauri::command(rename_all = "camelCase")]
    pub fn list_models(
        state: tauri::State<'_, Database>,
        include_inactive: Option<bool>,
    ) -> AppResult<Vec<Model>> {
        let connection = lock_connection(&state)?;
        let sql = if include_inactive.unwrap_or(false) {
            "SELECT id,name,provider,color,is_active,created_at,updated_at FROM models ORDER BY is_active DESC, name COLLATE NOCASE"
        } else {
            "SELECT id,name,provider,color,is_active,created_at,updated_at FROM models WHERE is_active = 1 ORDER BY name COLLATE NOCASE"
        };
        let mut statement = connection.prepare(sql).map_err(AppError::database)?;
        let result = statement
            .query_map([], model_from_row)
            .map_err(AppError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::database);
        result
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn create_model(
        state: tauri::State<'_, Database>,
        name: String,
        provider: Option<String>,
        color: Option<String>,
    ) -> AppResult<Model> {
        let name = normalize_required(&name, "模型名称", 100)?;
        let provider = normalize_optional(provider, 100)?;
        let color = normalize_optional(color, 32)?;
        let mut connection = lock_connection(&state)?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        let transaction = connection.transaction().map_err(AppError::database)?;
        transaction.execute("INSERT INTO models (id,name,provider,color,is_active,created_at,updated_at) VALUES (?1,?2,?3,?4,1,?5,?5)", params![id, name, provider, color, timestamp]).map_err(|error| if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) { AppError::new("DUPLICATE_RECORD", "模型名称已存在") } else { AppError::database(error) })?;
        transaction.commit().map_err(AppError::database)?;
        connection.query_row("SELECT id,name,provider,color,is_active,created_at,updated_at FROM models WHERE id = ?1", params![id], model_from_row).map_err(AppError::database)
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn update_model(
        state: tauri::State<'_, Database>,
        id: String,
        name: String,
        provider: Option<String>,
        color: Option<String>,
    ) -> AppResult<Model> {
        let name = normalize_required(&name, "模型名称", 100)?;
        let provider = normalize_optional(provider, 100)?;
        let color = normalize_optional(color, 32)?;
        let mut connection = lock_connection(&state)?;
        let timestamp = now();
        let transaction = connection.transaction().map_err(AppError::database)?;
        let changed = transaction
            .execute(
                "UPDATE models SET name=?1, provider=?2, color=?3, updated_at=?4 WHERE id=?5",
                params![name, provider, color, timestamp, id],
            )
            .map_err(|error| {
                if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
                    AppError::new("DUPLICATE_RECORD", "模型名称已存在")
                } else {
                    AppError::database(error)
                }
            })?;
        if changed == 0 {
            return Err(AppError::new("MODEL_NOT_FOUND", "模型不存在"));
        }
        transaction.commit().map_err(AppError::database)?;
        connection.query_row("SELECT id,name,provider,color,is_active,created_at,updated_at FROM models WHERE id = ?1", params![id], model_from_row).map_err(AppError::database)
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn set_model_active(
        state: tauri::State<'_, Database>,
        id: String,
        active: bool,
    ) -> AppResult<Model> {
        let mut connection = lock_connection(&state)?;
        let transaction = connection.transaction().map_err(AppError::database)?;
        let changed = transaction
            .execute(
                "UPDATE models SET is_active=?1, updated_at=?2 WHERE id=?3",
                params![active as i64, now(), id],
            )
            .map_err(AppError::database)?;
        if changed == 0 {
            return Err(AppError::new("MODEL_NOT_FOUND", "模型不存在"));
        }
        transaction.commit().map_err(AppError::database)?;
        connection.query_row("SELECT id,name,provider,color,is_active,created_at,updated_at FROM models WHERE id = ?1", params![id], model_from_row).map_err(AppError::database)
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn delete_model(state: tauri::State<'_, Database>, id: String) -> AppResult<bool> {
        let mut connection = lock_connection(&state)?;
        let transaction = connection.transaction().map_err(AppError::database)?;
        delete_model_in_transaction(&transaction, &id)?;
        transaction.commit().map_err(AppError::database)?;
        Ok(true)
    }

    pub(super) fn delete_model_in_transaction(
        transaction: &rusqlite::Transaction<'_>,
        id: &str,
    ) -> AppResult<()> {
        let exists: Option<i64> = transaction
            .query_row("SELECT 1 FROM models WHERE id=?1", params![id], |row| {
                row.get(0)
            })
            .optional()
            .map_err(AppError::database)?;
        if exists.is_none() {
            return Err(AppError::new("MODEL_NOT_FOUND", "模型不存在"));
        }
        let record_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM usage_records WHERE model_id=?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(AppError::database)?;
        if record_count > 0 {
            return Err(AppError::new(
                "MODEL_IN_USE",
                "模型已有历史记录，不能删除，请改用停用",
            ));
        }
        transaction
            .execute("DELETE FROM models WHERE id=?1", params![id])
            .map_err(AppError::database)?;
        Ok(())
    }

    fn where_clause(filter: &UsageFilter) -> AppResult<(String, Vec<Value>)> {
        let mut clauses = vec!["1 = 1".to_string()];
        let mut values = Vec::new();
        if let (Some(from), Some(to)) = (filter.from.as_deref(), filter.to.as_deref()) {
            validate_range(from, to)?;
        }
        if let Some(from) = filter.from.as_deref() {
            validate_date(from)?;
            clauses.push("u.usage_date >= ?".to_string());
            values.push(Value::Text(from.to_string()));
        }
        if let Some(to) = filter.to.as_deref() {
            validate_date(to)?;
            clauses.push("u.usage_date <= ?".to_string());
            values.push(Value::Text(to.to_string()));
        }
        if let Some(ids) = &filter.model_ids {
            if !ids.is_empty() {
                clauses.push(format!(
                    "u.model_id IN ({})",
                    vec!["?"; ids.len()].join(",")
                ));
                values.extend(ids.iter().cloned().map(Value::Text));
            }
        }
        if let Some(search) = filter.search.as_deref() {
            let search = search.trim();
            if !search.is_empty() {
                clauses.push("(m.name LIKE ? OR COALESCE(m.provider, '') LIKE ? OR COALESCE(u.notes, '') LIKE ?)".to_string());
                let pattern = format!("%{search}%");
                values.extend([
                    Value::Text(pattern.clone()),
                    Value::Text(pattern.clone()),
                    Value::Text(pattern),
                ]);
            }
        }
        Ok((clauses.join(" AND "), values))
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn save_usage_record(
        state: tauri::State<'_, Database>,
        usage_date: String,
        model_id: String,
        token_count: i64,
        notes: Option<String>,
        overwrite: Option<bool>,
    ) -> AppResult<UsageRecord> {
        validate_date(&usage_date)?;
        validate_token_count(token_count)?;
        let notes = normalize_optional(notes, 1000)?;
        let mut connection = lock_connection(&state)?;
        let transaction = connection.transaction().map_err(AppError::database)?;
        let exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM models WHERE id = ?1",
                params![model_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppError::database)?;
        if exists.is_none() {
            return Err(AppError::new("MODEL_NOT_FOUND", "模型不存在"));
        }
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM usage_records WHERE usage_date=?1 AND model_id=?2",
                params![usage_date, model_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppError::database)?;
        let timestamp = now();
        let id = if let Some(id) = existing {
            if !overwrite.unwrap_or(false) {
                return Err(AppError::new(
                    "DUPLICATE_RECORD",
                    "该日期和模型已有记录，请确认覆盖",
                ));
            }
            transaction
                .execute(
                    "UPDATE usage_records SET token_count=?1, notes=?2, updated_at=?3 WHERE id=?4",
                    params![token_count, notes, timestamp, id],
                )
                .map_err(AppError::database)?;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            transaction.execute("INSERT INTO usage_records (id,usage_date,model_id,token_count,notes,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?6)", params![id, usage_date, model_id, token_count, notes, timestamp]).map_err(AppError::database)?;
            id
        };
        transaction.commit().map_err(AppError::database)?;
        connection.query_row("SELECT u.id,u.usage_date,u.model_id,m.name,m.provider,m.color,u.token_count,u.notes,u.created_at,u.updated_at FROM usage_records u JOIN models m ON m.id=u.model_id WHERE u.id=?1", params![id], record_from_row).map_err(AppError::database)
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn save_usage_records_batch(
        state: tauri::State<'_, Database>,
        records: Vec<BatchUsageRecordInput>,
        overwrite: Option<bool>,
    ) -> AppResult<Vec<UsageRecord>> {
        if records.is_empty() {
            return Err(AppError::validation("至少导入一条使用记录"));
        }
        if records.len() > 10_000 {
            return Err(AppError::validation("单次最多导入 10,000 条记录"));
        }
        let overwrite = overwrite.unwrap_or(false);
        let mut connection = lock_connection(&state)?;
        let transaction = connection.transaction().map_err(AppError::database)?;
        let ids = save_usage_records_batch_in_transaction(&transaction, records, overwrite)?;
        transaction.commit().map_err(AppError::database)?;
        ids.into_iter()
            .map(|id| {
                connection
                    .query_row(
                        "SELECT u.id,u.usage_date,u.model_id,m.name,m.provider,m.color,u.token_count,u.notes,u.created_at,u.updated_at FROM usage_records u JOIN models m ON m.id=u.model_id WHERE u.id=?1",
                        params![id],
                        record_from_row,
                    )
                    .map_err(AppError::database)
            })
            .collect()
    }

    pub(super) fn save_usage_records_batch_in_transaction(
        transaction: &rusqlite::Transaction<'_>,
        records: Vec<BatchUsageRecordInput>,
        overwrite: bool,
    ) -> AppResult<Vec<String>> {
        let mut ids = Vec::with_capacity(records.len());
        for input in records {
            validate_date(&input.usage_date)?;
            validate_token_count(input.token_count)?;
            let notes = normalize_optional(input.notes, 1000)?;
            let model_exists: Option<i64> = transaction
                .query_row(
                    "SELECT 1 FROM models WHERE id=?1",
                    params![input.model_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(AppError::database)?;
            if model_exists.is_none() {
                return Err(AppError::new("MODEL_NOT_FOUND", "模型不存在"));
            }
            let existing: Option<String> = transaction
                .query_row(
                    "SELECT id FROM usage_records WHERE usage_date=?1 AND model_id=?2",
                    params![input.usage_date, input.model_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(AppError::database)?;
            let timestamp = now();
            let id = if let Some(id) = existing {
                if !overwrite {
                    return Err(AppError::new(
                        "DUPLICATE_RECORD",
                        "该日期和模型已有记录，请确认覆盖",
                    ));
                }
                transaction
                    .execute(
                        "UPDATE usage_records SET token_count=?1, notes=?2, updated_at=?3 WHERE id=?4",
                        params![input.token_count, notes, timestamp, id],
                    )
                    .map_err(AppError::database)?;
                id
            } else {
                let id = Uuid::new_v4().to_string();
                transaction
                    .execute(
                        "INSERT INTO usage_records (id,usage_date,model_id,token_count,notes,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?6)",
                        params![id, input.usage_date, input.model_id, input.token_count, notes, timestamp],
                    )
                    .map_err(AppError::database)?;
                id
            };
            ids.push(id);
        }
        Ok(ids)
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn list_usage_records(
        state: tauri::State<'_, Database>,
        filter: Option<UsageFilter>,
        sort: Option<UsageSort>,
        page: Option<PageRequest>,
    ) -> AppResult<UsageRecordPage> {
        let filter = filter.unwrap_or_default();
        let sort = sort.unwrap_or_default();
        let page = page.unwrap_or_default();
        let current_page = page.page.unwrap_or(1).max(1);
        let page_size = page.page_size.unwrap_or(100).clamp(1, 1000);
        let (where_sql, values) = where_clause(&filter)?;
        let sort_field = match sort.field.as_deref() {
            Some("token_count") => "u.token_count",
            Some("model_name") => "m.name COLLATE NOCASE",
            _ => "u.usage_date",
        };
        let direction = if sort
            .direction
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case("asc"))
            .unwrap_or(false)
        {
            "ASC"
        } else {
            "DESC"
        };
        let connection = lock_connection(&state)?;
        let count_sql = format!(
        "SELECT COUNT(*) FROM usage_records u JOIN models m ON m.id=u.model_id WHERE {where_sql}"
    );
        let total: u64 = connection
            .query_row(
                &count_sql,
                params_from_iter(values.iter().map(|v| v as &dyn ToSql)),
                |row| row.get::<_, i64>(0),
            )
            .map_err(AppError::database)?
            .max(0) as u64;
        let offset = (current_page as u64 - 1).saturating_mul(page_size as u64);
        let list_sql = format!("SELECT u.id,u.usage_date,u.model_id,m.name,m.provider,m.color,u.token_count,u.notes,u.created_at,u.updated_at FROM usage_records u JOIN models m ON m.id=u.model_id WHERE {where_sql} ORDER BY {sort_field} {direction}, u.id LIMIT ? OFFSET ?");
        let mut list_values = values;
        list_values.push(Value::Integer(page_size as i64));
        list_values.push(Value::Integer(offset.min(i64::MAX as u64) as i64));
        let mut statement = connection.prepare(&list_sql).map_err(AppError::database)?;
        let items = statement
            .query_map(
                params_from_iter(list_values.iter().map(|v| v as &dyn ToSql)),
                record_from_row,
            )
            .map_err(AppError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::database)?;
        Ok(UsageRecordPage {
            items,
            total,
            page: current_page,
            page_size,
        })
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn delete_usage_record(state: tauri::State<'_, Database>, id: String) -> AppResult<bool> {
        let mut connection = lock_connection(&state)?;
        let transaction = connection.transaction().map_err(AppError::database)?;
        transaction
            .execute("DELETE FROM usage_records WHERE id=?1", params![id])
            .map_err(AppError::database)?;
        transaction.commit().map_err(AppError::database)?;
        Ok(true)
    }

    fn parse_stats_range(
        from: Option<String>,
        to: Option<String>,
    ) -> AppResult<(NaiveDate, NaiveDate)> {
        let today = Local::now().date_naive();
        let start =
            from.unwrap_or_else(|| (today - Duration::days(29)).format("%Y-%m-%d").to_string());
        let end = to.unwrap_or_else(|| today.format("%Y-%m-%d").to_string());
        validate_range(&start, &end)
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn query_stats(
        state: tauri::State<'_, Database>,
        from: Option<String>,
        to: Option<String>,
        model_ids: Option<Vec<String>>,
    ) -> AppResult<Stats> {
        let (start, end) = parse_stats_range(from, to)?;
        let filter = UsageFilter {
            from: Some(start.format("%Y-%m-%d").to_string()),
            to: Some(end.format("%Y-%m-%d").to_string()),
            model_ids,
            search: None,
        };
        let (where_sql, values) = where_clause(&filter)?;
        let connection = lock_connection(&state)?;
        let mut daily_map = std::collections::BTreeMap::<String, i64>::new();
        let mut statement = connection.prepare(&format!("SELECT u.usage_date, SUM(u.token_count) FROM usage_records u JOIN models m ON m.id=u.model_id WHERE {where_sql} GROUP BY u.usage_date ORDER BY u.usage_date")).map_err(AppError::database)?;
        let daily_rows = statement
            .query_map(
                params_from_iter(values.iter().map(|v| v as &dyn ToSql)),
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .map_err(AppError::database)?;
        for row in daily_rows {
            let (date, total) = row.map_err(AppError::database)?;
            daily_map.insert(date, total);
        }
        let mut daily = Vec::new();
        let mut cursor = start;
        while cursor <= end {
            let date = cursor.format("%Y-%m-%d").to_string();
            daily.push(DailyStats {
                date: date.clone(),
                total_tokens: daily_map.get(&date).copied().unwrap_or(0),
            });
            cursor += Duration::days(1);
        }
        let mut model_statement = connection.prepare(&format!("SELECT u.model_id,m.name,m.provider,m.color,SUM(u.token_count),COUNT(DISTINCT u.usage_date) FROM usage_records u JOIN models m ON m.id=u.model_id WHERE {where_sql} GROUP BY u.model_id,m.name,m.provider,m.color ORDER BY SUM(u.token_count) DESC")).map_err(AppError::database)?;
        let model_rows = model_statement
            .query_map(
                params_from_iter(values.iter().map(|v| v as &dyn ToSql)),
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .map_err(AppError::database)?;
        let mut raw_models = Vec::new();
        for row in model_rows {
            raw_models.push(row.map_err(AppError::database)?);
        }
        let total_tokens = daily.iter().map(|item| item.total_tokens).sum::<i64>();
        let by_model = raw_models
            .into_iter()
            .map(
                |(model_id, model_name, provider, color, total, record_days)| ModelStats {
                    model_id,
                    model_name,
                    provider,
                    color,
                    total_tokens: total,
                    percentage: if total_tokens == 0 {
                        0.0
                    } else {
                        (total as f64 / total_tokens as f64) * 100.0
                    },
                    record_days,
                },
            )
            .collect::<Vec<_>>();
        let highest_usage_day = daily
            .iter()
            .filter(|item| item.total_tokens > 0)
            .max_by_key(|item| item.total_tokens)
            .cloned();
        let day_count = daily.len().max(1) as f64;
        Ok(Stats {
            from: start.format("%Y-%m-%d").to_string(),
            to: end.format("%Y-%m-%d").to_string(),
            total_tokens,
            average_daily_tokens: total_tokens as f64 / day_count,
            highest_usage_day,
            active_model_count: by_model.len() as i64,
            daily,
            by_model,
        })
    }

    #[derive(Debug, Deserialize)]
    struct GitHubRelease {
        tag_name: String,
        name: Option<String>,
        body: Option<String>,
        published_at: Option<String>,
        html_url: String,
        #[serde(default)]
        assets: Vec<GitHubAsset>,
    }

    #[derive(Debug, Deserialize)]
    struct GitHubAsset {
        name: String,
        browser_download_url: String,
        size: u64,
    }

    async fn check_update_via_atom(
        client: &reqwest::Client,
        current_version: &str,
    ) -> Option<UpdateInfo> {
        let atom_url = "https://github.com/GodBook/token-scope/releases.atom";
        let resp = client.get(atom_url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body = resp.text().await.ok()?;
        if !body.contains("<entry>") {
            // 当前仓库尚未发布任何 Release
            return Some(UpdateInfo {
                has_update: false,
                current_version: current_version.to_string(),
                latest_version: current_version.to_string(),
                release_name: "当前已是最新版本".to_string(),
                release_notes: "当前 GitHub 仓库暂无新版本发布。".to_string(),
                release_date: "".to_string(),
                download_url: None,
                asset_name: None,
                asset_size: None,
                release_url: "https://github.com/GodBook/token-scope/releases".to_string(),
            });
        }

        let title_start = body.find("<entry>")?;
        let entry_slice = &body[title_start..];
        let t_start = entry_slice.find("<title>")? + 7;
        let t_end = entry_slice.find("</title>")?;
        let raw_title = &entry_slice[t_start..t_end];
        let tag = raw_title.trim();
        let clean_tag = tag.trim_start_matches(|c| c == 'v' || c == 'V');
        let has_update = is_newer_version(current_version, clean_tag);

        let date = if let (Some(d_start), Some(d_end)) =
            (entry_slice.find("<updated>"), entry_slice.find("</updated>"))
        {
            entry_slice[d_start + 9..d_end].to_string()
        } else {
            "".to_string()
        };

        let download_url = format!(
            "https://github.com/GodBook/token-scope/releases/download/{tag}/TokenScope_{clean_tag}_x64-setup.exe"
        );
        let asset_name = format!("TokenScope_{clean_tag}_x64-setup.exe");

        Some(UpdateInfo {
            has_update,
            current_version: current_version.to_string(),
            latest_version: clean_tag.to_string(),
            release_name: format!("TokenScope {tag}"),
            release_notes: "发现新版本发布，点击即可一键无损下载安装升级。".to_string(),
            release_date: date,
            download_url: Some(download_url),
            asset_name: Some(asset_name),
            asset_size: None,
            release_url: format!("https://github.com/GodBook/token-scope/releases/tag/{tag}"),
        })
    }

    #[tauri::command]
    pub async fn check_app_update(app: tauri::AppHandle) -> AppResult<UpdateInfo> {
        let current_version = app.package_info().version.to_string();
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .map_err(|e| AppError::new("NETWORK_ERROR", format!("创建请求客户端失败: {e}")))?;

        // 1. 优先尝试请求 GitHub REST API
        let api_url = "https://api.github.com/repos/GodBook/token-scope/releases/latest";
        if let Ok(response) = client
            .get(api_url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
        {
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(UpdateInfo {
                    has_update: false,
                    current_version: current_version.clone(),
                    latest_version: current_version,
                    release_name: "当前已是最新版本".to_string(),
                    release_notes: "当前 GitHub 仓库暂无新版本发布。".to_string(),
                    release_date: "".to_string(),
                    download_url: None,
                    asset_name: None,
                    asset_size: None,
                    release_url: "https://github.com/GodBook/token-scope/releases".to_string(),
                });
            }

            if response.status().is_success() {
                if let Ok(release) = response.json::<GitHubRelease>().await {
                    let clean_tag = release
                        .tag_name
                        .trim_start_matches(|c| c == 'v' || c == 'V')
                        .to_string();
                    let has_update = is_newer_version(&current_version, &clean_tag);

                    let asset = release
                        .assets
                        .iter()
                        .find(|a| a.name.ends_with(".exe") || a.name.ends_with(".msi"))
                        .or_else(|| release.assets.first());

                    let (download_url, asset_name, asset_size) = match asset {
                        Some(a) => (
                            Some(a.browser_download_url.clone()),
                            Some(a.name.clone()),
                            Some(a.size),
                        ),
                        None => (None, None, None),
                    };

                    return Ok(UpdateInfo {
                        has_update,
                        current_version,
                        latest_version: clean_tag,
                        release_name: release.name.unwrap_or_else(|| release.tag_name.clone()),
                        release_notes: release.body.unwrap_or_default(),
                        release_date: release.published_at.unwrap_or_default(),
                        download_url,
                        asset_name,
                        asset_size,
                        release_url: release.html_url,
                    });
                }
            }
        }

        // 2. 若 API 遭遇无鉴权速率限制 (403) 或被阻断，自动降级至 Atom Feed 免鉴权通道
        if let Some(info) = check_update_via_atom(&client, &current_version).await {
            return Ok(info);
        }

        Err(AppError::new(
            "NETWORK_ERROR",
            "无法连接 GitHub 检查更新，请检查网络或稍后重试",
        ))
    }

    #[tauri::command]
    pub async fn download_and_install_update(
        app: tauri::AppHandle,
        download_url: String,
        asset_name: String,
    ) -> AppResult<String> {
        use futures_util::StreamExt;
        use std::io::Write;

        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::new("PATH_ERROR", e.to_string()))?;

        // 1. 保障数据安全：更新前自动创建数据库完整快照
        let backup_path = backup_database_file(&data_dir)?;
        let backup_info = if backup_path.exists() {
            format!("已自动创建数据快照备份至：{}", backup_path.display())
        } else {
            "未检测到需要备份的历史数据".to_string()
        };

        // 2. 准备下载目录与文件
        let update_dir = std::env::temp_dir().join("token-scope-update");
        std::fs::create_dir_all(&update_dir)
            .map_err(|e| AppError::new("FS_ERROR", format!("创建下载目录失败: {e}")))?;
        let target_file = update_dir.join(&asset_name);

        let client = reqwest::Client::builder()
            .user_agent("TokenScope-Desktop")
            .build()
            .map_err(|e| AppError::new("NETWORK_ERROR", format!("创建客户端失败: {e}")))?;

        let response = client
            .get(&download_url)
            .send()
            .await
            .map_err(|e| AppError::new("DOWNLOAD_ERROR", format!("发起下载失败: {e}")))?;

        if !response.status().is_success() {
            return Err(AppError::new(
                "DOWNLOAD_ERROR",
                format!("下载失败，HTTP 状态码: {}", response.status()),
            ));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut file = std::fs::File::create(&target_file)
            .map_err(|e| AppError::new("FS_ERROR", format!("创建安装包文件失败: {e}")))?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| AppError::new("DOWNLOAD_ERROR", format!("下载传输中断: {e}")))?;
            file.write_all(&chunk)
                .map_err(|e| AppError::new("FS_ERROR", format!("写入文件失败: {e}")))?;
            downloaded += chunk.len() as u64;

            let percentage = if total_size > 0 {
                (downloaded as f64 / total_size as f64 * 100.0).min(100.0)
            } else {
                0.0
            };

            let _ = app.emit(
                "update-download-progress",
                DownloadProgress {
                    percentage,
                    downloaded,
                    total: total_size,
                },
            );
        }
        file.flush()
            .map_err(|e| AppError::new("FS_ERROR", format!("写入磁盘失败: {e}")))?;

        // 3. 启动安装程序（Windows 环境下保留 AppData 用户数据无损更新）
        #[cfg(target_os = "windows")]
        {
            if asset_name.ends_with(".msi") {
                std::process::Command::new("msiexec")
                    .args(["/i", target_file.to_str().unwrap_or_default()])
                    .spawn()
                    .map_err(|e| AppError::new("LAUNCH_FAILED", format!("启动 MSI 安装程序失败: {e}")))?;
            } else {
                std::process::Command::new(&target_file)
                    .spawn()
                    .map_err(|e| AppError::new("LAUNCH_FAILED", format!("启动安装程序失败: {e}")))?;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            return Err(AppError::new(
                "PLATFORM_UNSUPPORTED",
                "仅 Windows 平台支持自动启动安装包，请手动运行下载的文件",
            ));
        }

        Ok(format!(
            "更新包已就绪并启动安装程序！{}\n您的所有历史 Token 数据均已完好保留。",
            backup_info
        ))
    }

    #[tauri::command]
    pub fn backup_database_now(app: tauri::AppHandle) -> AppResult<BackupResult> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::new("PATH_ERROR", e.to_string()))?;
        let backup_path = backup_database_file(&data_dir)?;
        Ok(BackupResult {
            success: true,
            backup_path: backup_path.display().to_string(),
            message: format!("数据库已安全备份至：{}", backup_path.display()),
        })
    }

    #[tauri::command]
    pub fn get_app_info(app: tauri::AppHandle) -> AppResult<AppMetadata> {
        let version = app.package_info().version.to_string();
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::new("PATH_ERROR", e.to_string()))?;
        let db_path = data_dir.join(DATABASE_FILE);
        let backups_dir = data_dir.join("backups");
        let backup_count = if backups_dir.exists() {
            std::fs::read_dir(&backups_dir)
                .map(|entries| entries.filter_map(|e| e.ok()).count())
                .unwrap_or(0)
        } else {
            0
        };

        Ok(AppMetadata {
            version,
            app_data_dir: data_dir.display().to_string(),
            database_path: db_path.display().to_string(),
            backup_count,
        })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            std::fs::create_dir_all(&data_dir)?;
            let database_path = data_dir.join(DATABASE_FILE);
            let database = open_database(&database_path).map_err(|error| {
                Box::new(std::io::Error::other(error.message)) as Box<dyn std::error::Error>
            })?;
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_models,
            commands::create_model,
            commands::update_model,
            commands::set_model_active,
            commands::delete_model,
            commands::save_usage_record,
            commands::save_usage_records_batch,
            commands::list_usage_records,
            commands::delete_usage_record,
            commands::query_stats,
            commands::check_app_update,
            commands::download_and_install_update,
            commands::backup_database_now,
            commands::get_app_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_token_counts() {
        assert!(validate_token_count(0).is_err());
        assert!(validate_token_count(-1).is_err());
        assert!(validate_token_count(MAX_SAFE_INTEGER + 1).is_err());
        assert!(validate_token_count(1).is_ok());
    }

    #[test]
    fn validates_date_ranges() {
        assert!(validate_range("2026-08-01", "2026-08-31").is_ok());
        assert!(validate_range("2026-08-31", "2026-08-01").is_err());
        assert!(validate_date("2026-02-30").is_err());
    }

    #[test]
    fn migration_creates_schema_and_default_models() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        migrate(&connection).expect("migrate database");
        let model_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM models", [], |row| row.get(0))
            .expect("count models");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("count migrations");
        assert_eq!(model_count, 3);
        assert_eq!(migration_count, 1);
    }

    #[test]
    fn model_deletion_respects_usage_history() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("enable foreign keys");
        migrate(&connection).expect("migrate database");
        let timestamp = now();
        connection
            .execute(
                "INSERT INTO models (id,name,created_at,updated_at) VALUES (?1,?2,?3,?3)",
                params!["empty-model", "Empty model", timestamp],
            )
            .expect("insert empty model");
        {
            let transaction = connection
                .unchecked_transaction()
                .expect("start deletion transaction");
            commands::delete_model_in_transaction(&transaction, "empty-model")
                .expect("delete model without records");
            transaction.commit().expect("commit deletion");
        }
        let deleted_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM models WHERE id='empty-model'",
                [],
                |row| row.get(0),
            )
            .expect("count deleted model");
        assert_eq!(deleted_count, 0);

        connection
            .execute(
                "INSERT INTO models (id,name,created_at,updated_at) VALUES (?1,?2,?3,?3)",
                params!["used-model", "Used model", now()],
            )
            .expect("insert used model");
        connection
            .execute(
                "INSERT INTO usage_records (id,usage_date,model_id,token_count,created_at,updated_at) VALUES ('record-1','2026-08-25','used-model',100,?1,?1)",
                params![now()],
            )
            .expect("insert usage record");
        let transaction = connection
            .unchecked_transaction()
            .expect("start protected deletion transaction");
        let error = commands::delete_model_in_transaction(&transaction, "used-model")
            .expect_err("reject model with usage history");
        assert_eq!(error.code, "MODEL_IN_USE");
        drop(transaction);
        let retained_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM models WHERE id='used-model'",
                [],
                |row| row.get(0),
            )
            .expect("count retained model");
        assert_eq!(retained_count, 1);
    }

    #[test]
    fn batch_import_is_atomic_and_supports_overwrite() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("enable foreign keys");
        migrate(&connection).expect("migrate database");
        let records = (1..=3)
            .map(|day| BatchUsageRecordInput {
                usage_date: format!("2026-08-0{day}"),
                model_id: "openai-gpt-4o".to_string(),
                token_count: 1_500_000,
                notes: None,
            })
            .collect();
        {
            let transaction = connection
                .unchecked_transaction()
                .expect("start batch transaction");
            let ids =
                commands::save_usage_records_batch_in_transaction(&transaction, records, false)
                    .expect("insert batch records");
            assert_eq!(ids.len(), 3);
            transaction.commit().expect("commit batch transaction");
        }
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM usage_records WHERE model_id='openai-gpt-4o'",
                [],
                |row| row.get(0),
            )
            .expect("count batch records");
        assert_eq!(count, 3);

        let duplicate = vec![BatchUsageRecordInput {
            usage_date: "2026-08-02".to_string(),
            model_id: "openai-gpt-4o".to_string(),
            token_count: 2_000_000,
            notes: None,
        }];
        let transaction = connection
            .unchecked_transaction()
            .expect("start duplicate transaction");
        let error =
            commands::save_usage_records_batch_in_transaction(&transaction, duplicate, false)
                .expect_err("reject duplicate batch without overwrite");
        assert_eq!(error.code, "DUPLICATE_RECORD");
        drop(transaction);

        let overwrite = vec![BatchUsageRecordInput {
            usage_date: "2026-08-02".to_string(),
            model_id: "openai-gpt-4o".to_string(),
            token_count: 2_000_000,
            notes: Some("覆盖".to_string()),
        }];
        let transaction = connection
            .unchecked_transaction()
            .expect("start overwrite transaction");
        commands::save_usage_records_batch_in_transaction(&transaction, overwrite, true)
            .expect("overwrite duplicate batch");
        transaction.commit().expect("commit overwrite transaction");
        let token_count: i64 = connection
            .query_row(
                "SELECT token_count FROM usage_records WHERE usage_date='2026-08-02' AND model_id='openai-gpt-4o'",
                [],
                |row| row.get(0),
            )
            .expect("read overwritten token count");
        assert_eq!(token_count, 2_000_000);
    }

    #[test]
    fn version_comparison_works() {
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.1.0", "v0.1.1"));
        assert!(is_newer_version("0.1.0", "0.1.0.1"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.2.0", "0.1.9"));
        assert!(!is_newer_version("1.0.0", "v1.0.0"));
        assert!(!is_newer_version("0.1.0", "0.0.9"));
    }

    #[test]
    fn database_backup_works() {
        let temp_dir = std::env::temp_dir().join(format!("test_token_scope_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let db_path = temp_dir.join(DATABASE_FILE);
        std::fs::write(&db_path, b"test sqlite backup content").expect("write test db");

        let backup_path = backup_database_file(&temp_dir).expect("backup database");
        assert!(backup_path.exists());
        let backup_content = std::fs::read(&backup_path).expect("read backup file");
        assert_eq!(backup_content, b"test sqlite backup content");

        let bak_path = temp_dir.join("token-statistics.sqlite.bak");
        assert!(bak_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
