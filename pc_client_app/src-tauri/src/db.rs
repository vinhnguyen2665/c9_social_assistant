use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredPost {
    pub post_id: String,
    pub post_url: String,
    pub author_name: Option<String>,
    pub content_preview: Option<String>,
    pub total_comments_crawled: i64,
    pub total_orders_detected: i64,
    pub total_phones_detected: i64,
    pub status: String, // 'ACTIVE', 'PAUSED', 'COMPLETED', 'ERROR'
    pub crawl_interval_minutes: i64,
    pub last_cursor: Option<String>,
    pub last_crawled_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentItem {
    pub id: String,
    pub post_id: String,
    pub parent_comment_id: Option<String>,
    pub author_id: Option<String>,
    pub author_name: Option<String>,
    pub author_url: Option<String>,
    pub content: Option<String>,
    pub phone_numbers: Option<String>,
    pub intent_tag: Option<String>,
    pub created_at: Option<i64>,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(&db_path)?;

        // WAL Mode and Performance Tuning
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA cache_size = -64000; -- 64MB memory cache",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.init_schema()?;
        Ok(db)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS monitored_posts (
                post_id TEXT PRIMARY KEY,
                post_url TEXT NOT NULL,
                author_name TEXT,
                content_preview TEXT,
                total_comments_crawled INTEGER DEFAULT 0,
                total_orders_detected INTEGER DEFAULT 0,
                total_phones_detected INTEGER DEFAULT 0,
                status TEXT DEFAULT 'ACTIVE',
                crawl_interval_minutes INTEGER DEFAULT 30,
                last_cursor TEXT,
                last_crawled_at INTEGER,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS comments (
                id TEXT PRIMARY KEY,
                post_id TEXT NOT NULL,
                parent_comment_id TEXT,
                author_id TEXT,
                author_name TEXT,
                author_url TEXT,
                content TEXT,
                phone_numbers TEXT,
                intent_tag TEXT,
                created_at INTEGER,
                FOREIGN KEY (post_id) REFERENCES monitored_posts(post_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_comments_post_id ON comments(post_id);
            CREATE INDEX IF NOT EXISTS idx_comments_parent ON comments(parent_comment_id);
            CREATE INDEX IF NOT EXISTS idx_comments_created_at ON comments(created_at);
            CREATE INDEX IF NOT EXISTS idx_comments_intent ON comments(intent_tag);
            CREATE INDEX IF NOT EXISTS idx_posts_status ON monitored_posts(status);

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;

        // Auto-migrate any posts that were inserted with full URLs as post_id
        let mut corrupted_posts = Vec::new();
        if let Ok(mut stmt) = conn.prepare("SELECT post_id, post_url, crawl_interval_minutes, created_at FROM monitored_posts WHERE post_id LIKE 'http%'") {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
            }) {
                for r in rows.flatten() {
                    corrupted_posts.push(r);
                }
            }
        }
        for (old_id, url, interval, created) in corrupted_posts {
            let clean_id = crate::crawler::FacebookCrawler::extract_post_id(&url);
            if !clean_id.is_empty() && !clean_id.starts_with("http") {
                let _ = conn.execute("DELETE FROM monitored_posts WHERE post_id = ?1", params![old_id]);
                let _ = conn.execute(
                    "INSERT INTO monitored_posts (
                        post_id, post_url, author_name, content_preview,
                        total_comments_crawled, total_orders_detected, total_phones_detected,
                        status, crawl_interval_minutes, last_cursor, last_crawled_at, created_at
                    ) VALUES (?1, ?2, 'Bài viết Facebook', 'Đang quét nội dung...', 0, 0, 0, 'ACTIVE', ?3, NULL, NULL, ?4)
                    ON CONFLICT(post_id) DO NOTHING",
                    params![clean_id, url, interval, created],
                );
            }
        }

        Ok(())
    }

    pub fn add_or_update_post(&self, post: &MonitoredPost) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO monitored_posts (
                post_id, post_url, author_name, content_preview,
                total_comments_crawled, total_orders_detected, total_phones_detected,
                status, crawl_interval_minutes, last_cursor, last_crawled_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(post_id) DO UPDATE SET
                post_url = excluded.post_url,
                author_name = COALESCE(excluded.author_name, author_name),
                content_preview = COALESCE(excluded.content_preview, content_preview),
                crawl_interval_minutes = excluded.crawl_interval_minutes,
                status = excluded.status,
                last_crawled_at = excluded.last_crawled_at",
            params![
                post.post_id,
                post.post_url,
                post.author_name,
                post.content_preview,
                post.total_comments_crawled,
                post.total_orders_detected,
                post.total_phones_detected,
                post.status,
                post.crawl_interval_minutes,
                post.last_cursor,
                post.last_crawled_at,
                post.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_monitored_posts(&self) -> Result<Vec<MonitoredPost>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT post_id, post_url, author_name, content_preview,
                    total_comments_crawled, total_orders_detected, total_phones_detected,
                    status, crawl_interval_minutes, last_cursor, last_crawled_at, created_at
             FROM monitored_posts
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(MonitoredPost {
                post_id: row.get(0)?,
                post_url: row.get(1)?,
                author_name: row.get(2)?,
                content_preview: row.get(3)?,
                total_comments_crawled: row.get(4)?,
                total_orders_detected: row.get(5)?,
                total_phones_detected: row.get(6)?,
                status: row.get(7)?,
                crawl_interval_minutes: row.get(8)?,
                last_cursor: row.get(9)?,
                last_crawled_at: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;

        let mut posts = Vec::new();
        for r in rows {
            posts.push(r?);
        }
        Ok(posts)
    }

    pub fn get_post(&self, post_id: &str) -> Result<Option<MonitoredPost>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT post_id, post_url, author_name, content_preview,
                    total_comments_crawled, total_orders_detected, total_phones_detected,
                    status, crawl_interval_minutes, last_cursor, last_crawled_at, created_at
             FROM monitored_posts WHERE post_id = ?1",
        )?;

        let mut rows = stmt.query_map(params![post_id], |row| {
            Ok(MonitoredPost {
                post_id: row.get(0)?,
                post_url: row.get(1)?,
                author_name: row.get(2)?,
                content_preview: row.get(3)?,
                total_comments_crawled: row.get(4)?,
                total_orders_detected: row.get(5)?,
                total_phones_detected: row.get(6)?,
                status: row.get(7)?,
                crawl_interval_minutes: row.get(8)?,
                last_cursor: row.get(9)?,
                last_crawled_at: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;

        if let Some(post) = rows.next() {
            Ok(Some(post?))
        } else {
            Ok(None)
        }
    }

    pub fn update_post_status(&self, post_id: &str, status: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE monitored_posts SET status = ?1 WHERE post_id = ?2",
            params![status, post_id],
        )?;
        Ok(())
    }

    pub fn update_post_details(
        &self,
        post_id: &str,
        author_name: Option<&str>,
        content_preview: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE monitored_posts SET
                author_name = COALESCE(?1, author_name),
                content_preview = COALESCE(?2, content_preview)
             WHERE post_id = ?3",
            params![author_name, content_preview, post_id],
        )?;
        Ok(())
    }

    pub fn update_monitored_post(
        &self,
        post_id: &str,
        author_name: Option<&str>,
        content_preview: Option<&str>,
        crawl_interval_minutes: Option<i64>,
        status: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE monitored_posts SET
                author_name = COALESCE(?1, author_name),
                content_preview = COALESCE(?2, content_preview),
                crawl_interval_minutes = COALESCE(?3, crawl_interval_minutes),
                status = COALESCE(?4, status)
             WHERE post_id = ?5",
            params![author_name, content_preview, crawl_interval_minutes, status, post_id],
        )?;
        Ok(())
    }

    pub fn delete_post(&self, post_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM comments WHERE post_id = ?1", params![post_id])?;
        conn.execute("DELETE FROM monitored_posts WHERE post_id = ?1", params![post_id])?;
        Ok(())
    }

    pub fn get_post_comment_count(&self, post_id: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM comments WHERE post_id = ?1",
            params![post_id],
            |r| r.get(0),
        )
    }

    pub fn get_subcomments_count(&self, parent_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM comments WHERE parent_comment_id = ?1",
            params![parent_id],
            |r| r.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn has_comment(&self, comment_id: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT 1 FROM comments WHERE id = ?1",
            params![comment_id],
            |_| Ok(()),
        ).is_ok()
    }

    /// High performance batch insert for crawled comments using transactions.
    /// Chunks into 500-item transactions as required by spec to maintain UI fluidity.
    /// Uses INSERT OR IGNORE to safely skip duplicate IDs without failing.
    pub fn batch_insert_comments(&self, comments: &[CommentItem]) -> Result<usize> {
        if comments.is_empty() {
            return Ok(0);
        }

        // Validate that all comments reference an existing post_id before inserting
        // This prevents FOREIGN KEY constraint failures from unexpected post_ids
        let valid_comments: Vec<&CommentItem> = {
            let conn = self.conn.lock().unwrap();
            comments.iter().filter(|c| {
                conn.query_row(
                    "SELECT 1 FROM monitored_posts WHERE post_id = ?1",
                    params![c.post_id],
                    |_| Ok(()),
                ).is_ok()
            }).collect()
        };

        if valid_comments.is_empty() {
            return Ok(0);
        }

        let mut conn = self.conn.lock().unwrap();
        let chunk_size = 500;
        let mut total_inserted = 0;

        for chunk in valid_comments.chunks(chunk_size) {
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO comments (
                        id, post_id, parent_comment_id, author_id, author_name,
                        author_url, content, phone_numbers, intent_tag, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    ON CONFLICT(id) DO UPDATE SET
                        parent_comment_id = COALESCE(excluded.parent_comment_id, comments.parent_comment_id),
                        content = COALESCE(excluded.content, comments.content),
                        author_name = COALESCE(excluded.author_name, comments.author_name),
                        author_url = COALESCE(excluded.author_url, comments.author_url),
                        phone_numbers = COALESCE(excluded.phone_numbers, comments.phone_numbers),
                        intent_tag = COALESCE(excluded.intent_tag, comments.intent_tag),
                        created_at = COALESCE(excluded.created_at, comments.created_at)",
                )?;

                for c in chunk {
                    match stmt.execute(params![
                        c.id,
                        c.post_id,
                        c.parent_comment_id,
                        c.author_id,
                        c.author_name,
                        c.author_url,
                        c.content,
                        c.phone_numbers,
                        c.intent_tag,
                        c.created_at
                    ]) {
                        Ok(rows) => total_inserted += rows,
                        Err(e) => eprintln!("[DB] Skip comment {}: {}", c.id, e),
                    }
                }
            }
            tx.commit()?;
        }

        Ok(total_inserted)
    }

    /// Recalculates and updates the statistical counters on the post record
    pub fn update_post_statistics(&self, post_id: &str, last_cursor: Option<String>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();

        let total_comments: i64 = conn.query_row(
            "SELECT COUNT(*) FROM comments WHERE post_id = ?1",
            params![post_id],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_orders: i64 = conn.query_row(
            "SELECT COUNT(*) FROM comments WHERE post_id = ?1 AND intent_tag = '[Chốt đơn]'",
            params![post_id],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_phones: i64 = conn.query_row(
            "SELECT COUNT(*) FROM comments WHERE post_id = ?1 AND phone_numbers IS NOT NULL AND phone_numbers != ''",
            params![post_id],
            |r| r.get(0),
        ).unwrap_or(0);

        conn.execute(
            "UPDATE monitored_posts SET
                total_comments_crawled = ?1,
                total_orders_detected = ?2,
                total_phones_detected = ?3,
                last_cursor = COALESCE(?4, last_cursor),
                last_crawled_at = ?5
             WHERE post_id = ?6",
            params![
                total_comments,
                total_orders,
                total_phones,
                last_cursor,
                now,
                post_id
            ],
        )?;

        Ok(())
    }

    pub fn get_comments(
        &self,
        post_id: &str,
        intent_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CommentItem>> {
        let conn = self.conn.lock().unwrap();

        let mut query = "SELECT id, post_id, parent_comment_id, author_id, author_name,
                                author_url, content, phone_numbers, intent_tag, created_at
                         FROM comments WHERE post_id = ?1".to_string();

        if let Some(intent) = intent_filter {
            if !intent.is_empty() && intent != "ALL" {
                query.push_str(" AND intent_tag = '");
                query.push_str(&intent.replace('\'', "''"));
                query.push('\'');
            }
        }

        query.push_str(" ORDER BY created_at DESC LIMIT ?2 OFFSET ?3");

        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(params![post_id, limit, offset], |row| {
            Ok(CommentItem {
                id: row.get(0)?,
                post_id: row.get(1)?,
                parent_comment_id: row.get(2)?,
                author_id: row.get(3)?,
                author_name: row.get(4)?,
                author_url: row.get(5)?,
                content: row.get(6)?,
                phone_numbers: row.get(7)?,
                intent_tag: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |r| r.get(0))?;
        if let Some(val) = rows.next() {
            Ok(Some(val?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn close_database(&self) {
        // Drop cache and optimize
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute_batch("PRAGMA optimize;");
        }
    }
}
