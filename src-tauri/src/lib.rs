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
    pub is_local_update: bool,
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
    pub current_exe_path: String,
    pub local_project_path: Option<String>,
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

    fn format_system_time(time: std::time::SystemTime) -> String {
        let dt: chrono::DateTime<chrono::Local> = time.into();
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn find_local_project_dir() -> Option<std::path::PathBuf> {
        // 1. 优先检查本地固定路径
        let fixed = std::path::PathBuf::from(r"D:\CHATGPT\token统计器");
        if fixed.join("src-tauri").join("Cargo.toml").exists() {
            return Some(fixed);
        }

        // 2. 检查环境变量
        if let Ok(env_path) = std::env::var("TOKEN_SCOPE_PROJECT_DIR") {
            let p = std::path::PathBuf::from(env_path);
            if p.join("src-tauri").join("Cargo.toml").exists() {
                return Some(p);
            }
        }

        // 3. 从当前运行的 exe 向上追溯查找
        if let Ok(current_exe) = std::env::current_exe() {
            let mut cur = current_exe.parent();
            while let Some(dir) = cur {
                if dir.join("src-tauri").join("Cargo.toml").exists() {
                    return Some(dir.to_path_buf());
                }
                let sibling = dir.join("token统计器");
                if sibling.join("src-tauri").join("Cargo.toml").exists() {
                    return Some(sibling);
                }
                cur = dir.parent();
            }
        }

        None
    }

    fn get_latest_source_mtime(project_dir: &std::path::Path) -> Option<std::time::SystemTime> {
        let mut latest: Option<std::time::SystemTime> = None;
        let targets = [
            project_dir.join("src"),
            project_dir.join("src-tauri").join("src"),
            project_dir.join("src-tauri").join("Cargo.toml"),
            project_dir.join("package.json"),
        ];

        for target in &targets {
            if target.is_file() {
                if let Ok(meta) = std::fs::metadata(target) {
                    if let Ok(mtime) = meta.modified() {
                        latest = Some(latest.map_or(mtime, |prev| prev.max(mtime)));
                    }
                }
            } else if target.is_dir() {
                let mut stack = vec![target.clone()];
                while let Some(dir) = stack.pop() {
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            let path = entry.path();
                            if path.is_dir() {
                                let name = entry.file_name();
                                let s = name.to_string_lossy();
                                if s != "target" && s != "node_modules" && s != ".git" {
                                    stack.push(path);
                                }
                            } else if path.is_file() {
                                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                                if matches!(ext, "rs" | "tsx" | "ts" | "css" | "json" | "html" | "toml") {
                                    if let Ok(meta) = entry.metadata() {
                                        if let Ok(mtime) = meta.modified() {
                                            latest = Some(latest.map_or(mtime, |prev| prev.max(mtime)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        latest
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
                is_local_update: false,
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
            is_local_update: false,
        })
    }

    #[tauri::command]
    pub async fn check_app_update(app: tauri::AppHandle) -> AppResult<UpdateInfo> {
        let current_version = app.package_info().version.to_string();

        // 1. 本地更新优先检查（完全脱机，免联网，秒级响应）
        if let Some(project_dir) = find_local_project_dir() {
            if let Ok(current_exe) = std::env::current_exe() {
                let current_mtime = std::fs::metadata(&current_exe)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let current_mtime_str = format_system_time(current_mtime);

                let local_release_exe = project_dir
                    .join("src-tauri")
                    .join("target")
                    .join("release")
                    .join("token-scope.exe");

                let is_same_file = match (current_exe.canonicalize(), local_release_exe.canonicalize()) {
                    (Ok(a), Ok(b)) => a == b,
                    _ => false,
                };

                // 检查已构建的新程序
                let (has_compiled_update, local_mtime_str, local_size) = if local_release_exe.exists() {
                    let meta = std::fs::metadata(&local_release_exe).ok();
                    let mtime = meta.as_ref().and_then(|m| m.modified().ok());
                    let size = meta.as_ref().map(|m| m.len());
                    let is_newer = if is_same_file {
                        false
                    } else if let (Some(l_time), c_time) = (mtime, current_mtime) {
                        l_time > c_time + std::time::Duration::from_secs(2)
                    } else {
                        false
                    };
                    let m_str = mtime.map(format_system_time).unwrap_or_default();
                    (is_newer, m_str, size)
                } else {
                    (false, String::new(), None)
                };

                // 检查源码是否有新修改
                let source_mtime = get_latest_source_mtime(&project_dir);
                let is_source_newer = if let (Some(s_time), c_time) = (source_mtime, current_mtime) {
                    s_time > c_time + std::time::Duration::from_secs(2)
                } else {
                    false
                };
                let source_mtime_str = source_mtime.map(format_system_time).unwrap_or_default();

                if has_compiled_update {
                    let size_mb = local_size.unwrap_or(0) as f64 / (1024.0 * 1024.0);
                    return Ok(UpdateInfo {
                        has_update: true,
                        current_version: format!("v{current_version} ({current_mtime_str})"),
                        latest_version: format!("本地编译完成 ({local_mtime_str})"),
                        release_name: "检测到本地工程已构建最新版本".to_string(),
                        release_notes: format!(
                            "本地工程路径：{}\n最新程序构建时间：{}\n文件大小：{:.2} MB\n\n点击【一键应用本地更新】，系统将自动备份数据库，并无损热替换为本地最新版本重新启动，无需联网。",
                            project_dir.display(),
                            local_mtime_str,
                            size_mb
                        ),
                        release_date: local_mtime_str,
                        download_url: Some(local_release_exe.to_string_lossy().to_string()),
                        asset_name: Some("TokenScope.exe".to_string()),
                        asset_size: local_size,
                        release_url: project_dir.display().to_string(),
                        is_local_update: true,
                    });
                }

                if is_source_newer {
                    return Ok(UpdateInfo {
                        has_update: true,
                        current_version: format!("v{current_version} ({current_mtime_str})"),
                        latest_version: format!("源码有新修改 ({source_mtime_str})"),
                        release_name: "检测到本地源码有改动（待编译）".to_string(),
                        release_notes: format!(
                            "本地工程路径：{}\n代码最新修改时间：{}\n\n本地源码已发生变化，尚未编译为可执行文件。点击【一键本地编译并更新】，将自动备份数据并在后台完成增量编译与无损热重启。",
                            project_dir.display(),
                            source_mtime_str
                        ),
                        release_date: source_mtime_str,
                        download_url: Some("local://build_and_sync".to_string()),
                        asset_name: Some("build_and_sync".to_string()),
                        asset_size: None,
                        release_url: project_dir.display().to_string(),
                        is_local_update: true,
                    });
                }

                // 已是本地最新
                return Ok(UpdateInfo {
                    has_update: false,
                    current_version: format!("v{current_version} ({current_mtime_str})"),
                    latest_version: format!("v{current_version} ({current_mtime_str})"),
                    release_name: "当前已是本地最新版本".to_string(),
                    release_notes: format!(
                        "当前运行程序已与本地工程源码及编译产物完全一致，无需更新。\n本地工程：{}\n运行程序：{}",
                        project_dir.display(),
                        current_exe.display()
                    ),
                    release_date: current_mtime_str,
                    download_url: None,
                    asset_name: None,
                    asset_size: None,
                    release_url: project_dir.display().to_string(),
                    is_local_update: true,
                });
            }
        }

        // 2. 若未检测到本地工程，降级回退到网络检查
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .map_err(|e| AppError::new("NETWORK_ERROR", format!("创建请求客户端失败: {e}")))?;

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
                    is_local_update: false,
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
                        is_local_update: false,
                    });
                }
            }
        }

        if let Some(mut info) = check_update_via_atom(&client, &current_version).await {
            info.is_local_update = false;
            return Ok(info);
        }

        Err(AppError::new(
            "NETWORK_ERROR",
            "未能定位本地工程，且无法连接 GitHub 检查更新",
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

        // 1. 保障数据安全：所有更新前自动创建数据库完整快照
        let backup_path = backup_database_file(&data_dir)?;
        let backup_info = if backup_path.exists() {
            format!("已自动创建数据快照备份至：{}", backup_path.display())
        } else {
            "未检测到需要备份的历史数据".to_string()
        };

        // 2. 本地编译并同步更新模式 (download_url == "local://build_and_sync")
        if download_url == "local://build_and_sync" {
            if let Some(project_dir) = find_local_project_dir() {
                let script_path = project_dir.join("scripts").join("update-local.bat");
                if !script_path.exists() {
                    let _ = std::fs::create_dir_all(project_dir.join("scripts"));
                    let content = r#"@echo off
chcp 65001 >nul
title TokenScope 本地一键编译与热更新
echo ====================================================
echo  TokenScope 本地编译与无损热更新
echo ====================================================
echo 正在执行前端构建...
cd /d "%~dp0\.."
call npm run build
if %ERRORLEVEL% NEQ 0 (
    echo 前端构建失败，按任意键退出
    pause
    exit /b 1
)
echo 正在执行 Rust 核心编译...
call cargo build --release --manifest-path "src-tauri/Cargo.toml"
if %ERRORLEVEL% NEQ 0 (
    echo Rust 核心构建失败，按任意键退出
    pause
    exit /b 1
)
echo 正在安全更新可执行文件...
copy /y "src-tauri\target\release\token-scope.exe" "..\TokenScope-绿色免安装版\TokenScope.exe" >nul
echo 正在重新启动 TokenScope...
start "" "..\TokenScope-绿色免安装版\TokenScope.exe"
exit 0
"#;
                    let _ = std::fs::write(&script_path, content);
                }

                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "", script_path.to_str().unwrap_or_default()])
                        .spawn();
                    app.exit(0);
                }

                return Ok("已启动本地编译更新程序，请稍候...".to_string());
            }
        }

        // 3. 本地已编译可执行文件直接热替换 (download_url 为本地路径)
        let is_local_file = !download_url.starts_with("http://") && !download_url.starts_with("https://");
        if is_local_file {
            let source_exe = std::path::PathBuf::from(&download_url);
            if !source_exe.exists() {
                return Err(AppError::new(
                    "FILE_NOT_FOUND",
                    format!("本地源文件不存在: {}", source_exe.display()),
                ));
            }

            let current_exe = std::env::current_exe()
                .map_err(|e| AppError::new("PATH_ERROR", format!("获取当前程序路径失败: {e}")))?;

            let is_same = match (current_exe.canonicalize(), source_exe.canonicalize()) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            };

            if is_same {
                return Ok("当前运行的文件已是最新构建文件，无需重复替换。".to_string());
            }

            let _ = app.emit(
                "update-download-progress",
                DownloadProgress {
                    percentage: 50.0,
                    downloaded: 1,
                    total: 2,
                },
            );

            // Windows 热替换：重命名当前 running exe -> 复制新 exe -> 唤起新 exe -> 退出当前进程
            let old_exe = current_exe.with_extension("exe.old");
            if old_exe.exists() {
                let _ = std::fs::remove_file(&old_exe);
            }

            std::fs::rename(&current_exe, &old_exe)
                .map_err(|e| AppError::new("RENAME_ERROR", format!("重命名原程序文件失败: {e}")))?;

            if let Err(e) = std::fs::copy(&source_exe, &current_exe) {
                let _ = std::fs::rename(&old_exe, &current_exe);
                return Err(AppError::new("COPY_ERROR", format!("复制新版本文件失败: {e}")));
            }

            let _ = app.emit(
                "update-download-progress",
                DownloadProgress {
                    percentage: 100.0,
                    downloaded: 2,
                    total: 2,
                },
            );

            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new(&current_exe).spawn();
                app.exit(0);
            }

            return Ok(format!(
                "本地更新已就绪并重启！{}\n您的所有历史 Token 数据均已完好保留。",
                backup_info
            ));
        }

        // 4. 远程网络下载回退逻辑
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

        let current_exe_path = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let local_project_path = find_local_project_dir().map(|p| p.display().to_string());

        Ok(AppMetadata {
            version,
            app_data_dir: data_dir.display().to_string(),
            database_path: db_path.display().to_string(),
            backup_count,
            current_exe_path,
            local_project_path,
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

            // 清理可能遗留的历史更新旧文件
            if let Ok(current_exe) = std::env::current_exe() {
                let old_exe = current_exe.with_extension("exe.old");
                if old_exe.exists() {
                    let _ = std::fs::remove_file(old_exe);
                }
            }

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
