use chrono::Utc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::crawler::FacebookCrawler;
use crate::db::Database;
use crate::license::LicenseManager;

pub struct WatchlistTracker {
    db: Arc<Database>,
    crawler: Arc<FacebookCrawler>,
    license: Arc<LicenseManager>,
}

impl WatchlistTracker {
    pub fn new(
        db: Arc<Database>,
        crawler: Arc<FacebookCrawler>,
        license: Arc<LicenseManager>,
    ) -> Self {
        Self {
            db,
            crawler,
            license,
        }
    }

    /// Performs incremental crawling for a specific monitored post
    /// Stops immediately if old comments are encountered to conserve resources
    pub async fn crawl_post_incremental(
        &self,
        post_id: &str,
        app_handle: Option<&AppHandle>,
    ) -> Result<usize, String> {
        // Strict license check before crawling
        if !self.license.is_authenticated.load(Ordering::SeqCst) {
            return Err("Ứng dụng chưa được kích hoạt bản quyền trực tuyến.".to_string());
        }

        let post = self.db.get_post(post_id)
            .map_err(|e| format!("DB Error: {}", e))?
            .ok_or_else(|| "Không tìm thấy bài viết theo dõi.".to_string())?;

        let existing_comments_count = self.db.get_post_comment_count(post_id).unwrap_or(0);
        let mut current_cursor: Option<String> = None;
        let mut total_new_comments = 0;
        let mut target_id = post_id.to_string();

        let mut known_total_count: Option<i64> = None;

        // 1. Fetch fresh post details (author, preview, resolved target ID, embedded comments)
        if let Ok(details) = self.crawler.fetch_post_details(&post.post_url, post_id).await {
            if details.author_name.is_some() || details.content_preview.is_some() {
                let _ = self.db.update_post_details(
                    post_id,
                    details.author_name.as_deref(),
                    details.content_preview.as_deref(),
                );
            }

            if let Some(total) = details.total_comment_count {
                known_total_count = Some(total);
            }

            if let Some(res_id) = details.resolved_id {
                target_id = res_id;
            }

            if let Some(dtsg) = details.fb_dtsg {
                if let Some(mut session) = self.crawler.get_session().await {
                    if session.fb_dtsg == "NA" || session.fb_dtsg.is_empty() {
                        session.fb_dtsg = dtsg;
                        self.crawler.set_session(session).await;
                    }
                }
            }

            if !details.initial_comments.is_empty() {
                if let Ok(inserted) = self.db.batch_insert_comments(&details.initial_comments) {
                    total_new_comments += inserted;
                }
            }

            // Fetch replies for initial parent comments that report replies
            if !details.reply_targets.is_empty() {
                eprintln!("[Tracker] Found {} parent comments with replies in post {}", details.reply_targets.len(), post_id);
                for target in &details.reply_targets {
                    let existing_sub = self.db.get_subcomments_count(&target.parent_id).unwrap_or(0);
                    if existing_sub < target.replies_count as usize {
                        FacebookCrawler::jitter_sleep().await;
                        match self.crawler.fetch_comment_replies(post_id, &target.parent_id, &target.feedback_id, &target.expansion_token).await {
                            Ok(replies) => {
                                if !replies.is_empty() {
                                    if let Ok(inserted) = self.db.batch_insert_comments(&replies) {
                                        total_new_comments += inserted;
                                        eprintln!("[Tracker] Parent {}: collected {}/{} sub-comments", target.parent_id, replies.len(), target.replies_count);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("[Tracker Error] Fetching replies for parent {}: {}", target.parent_id, e);
                            }
                        }
                    }
                }
            }
        }

        // 2. Incremental GraphQL comments crawl - driven by Facebook's reported total_count
        // If we know total_count, crawl until we have that many. Otherwise safety cap at 500 pages.
        let mut consecutive_existing_pages = 0;
        // For brand new posts (no existing comments), never early-stop
        let is_fresh_post = existing_comments_count == 0;
        // Safety page cap: 500 pages × up to 50 comments/page = 25,000 comments max
        let safety_max_pages = 500usize;

        for page in 0..safety_max_pages {
            // Jitter sleep between pages to avoid rate limiting
            if page > 0 {
                FacebookCrawler::jitter_sleep().await;
            }

            let res = match self.crawler.fetch_comments_page(&target_id, current_cursor.as_deref()).await {
                Ok(r) => r,
                Err(err) => {
                    eprintln!("[Tracker Error] Target {}: {}", target_id, err);
                    if total_new_comments == 0 && existing_comments_count == 0 {
                        let _ = self.db.update_post_status(post_id, "ERROR");
                    }
                    break;
                }
            };

            if res.is_rate_limited {
                eprintln!("[Tracker Warning] Facebook 429 Rate Limit. Tạm dừng cào post {}", post_id);
                break;
            }

            if res.is_checkpoint {
                eprintln!("[Tracker Warning] Facebook Session Checkpoint! Cần đăng nhập lại.");
                let _ = self.db.update_post_status(post_id, "PAUSED");
                if let Some(h) = app_handle {
                    let _ = h.emit("facebook_checkpoint_detected", serde_json::json!({
                        "post_id": post_id,
                        "message": "Phiên Facebook đã hết hạn hoặc bị checkpoint. Vui lòng cập nhật cookies."
                    }));
                }
                break;
            }

            // Update our known total from latest response (Facebook reports it on each page)
            if let Some(tc) = res.total_count {
                if tc > 0 {
                    known_total_count = Some(tc);
                }
            }

            if res.comments.is_empty() {
                break;
            }

            // Check how many comments in this batch are newly discovered
            let mut page_new_items = 0;
            let mut batch = Vec::new();
            for mut c in res.comments {
                c.post_id = post_id.to_string(); // Ensure post_id matches parent monitored post
                if !self.db.has_comment(&c.id) {
                    page_new_items += 1;
                }
                batch.push(c);
            }

            if !batch.is_empty() {
                let inserted = self.db.batch_insert_comments(&batch)
                    .map_err(|e| format!("Lỗi lưu comments vào SQLite: {}", e))?;
                total_new_comments += inserted;
            }

            // Fetch replies for any parent comments found in this page
            if !res.reply_targets.is_empty() {
                for target in &res.reply_targets {
                    let existing_sub = self.db.get_subcomments_count(&target.parent_id).unwrap_or(0);
                    if existing_sub < target.replies_count as usize {
                        FacebookCrawler::jitter_sleep().await;
                        match self.crawler.fetch_comment_replies(post_id, &target.parent_id, &target.feedback_id, &target.expansion_token).await {
                            Ok(replies) => {
                                if !replies.is_empty() {
                                    if let Ok(inserted) = self.db.batch_insert_comments(&replies) {
                                        total_new_comments += inserted;
                                        eprintln!("[Tracker] Parent {}: collected {}/{} sub-comments", target.parent_id, replies.len(), target.replies_count);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("[Tracker Error] Fetching replies for parent {}: {}", target.parent_id, e);
                            }
                        }
                    }
                }
            }

            // Live UI update for real-time progress
            let _ = self.db.update_post_statistics(post_id, res.next_cursor.clone());
            if let Ok(Some(updated_post)) = self.db.get_post(post_id) {
                if let Some(h) = app_handle {
                    let _ = h.emit("post_updated", &updated_post);
                }
            }

            // Check if we've reached the target total_count
            let current_db_count = self.db.get_post_comment_count(post_id).unwrap_or(0);
            if let Some(total) = known_total_count {
                eprintln!("[Tracker] Page {}: {}/{} comments collected (post_id={})", page + 1, current_db_count, total, post_id);
                if current_db_count >= total {
                    eprintln!("[Tracker] ✓ Reached total_count={}, stopping.", total);
                    break;
                }
            }

            // Early stop logic: only stop early if we have collected enough or hit repeated empty pages without a target total
            if !is_fresh_post {
                let can_early_stop = match known_total_count {
                    Some(total) => current_db_count >= total,
                    None => true,
                };
                if can_early_stop {
                    if page_new_items == 0 {
                        consecutive_existing_pages += 1;
                        if consecutive_existing_pages >= 5 {
                            eprintln!("[Tracker] Early stop: 5 consecutive existing pages (post has {} existing, total={})",
                                current_db_count, known_total_count.unwrap_or(-1));
                            break;
                        }
                    } else {
                        consecutive_existing_pages = 0;
                    }
                }
            }

            if res.next_cursor.is_none() || res.next_cursor == current_cursor || !res.has_next_page {
                break;
            }

            current_cursor = res.next_cursor;
        }

        // Update post statistics and latest cursor
        let _ = self.db.update_post_statistics(post_id, current_cursor);
        let _ = self.db.update_post_status(post_id, "ACTIVE");

        // Fetch updated post and emit event to UI
        if let Ok(Some(updated_post)) = self.db.get_post(post_id) {
            if let Some(h) = app_handle {
                let _ = h.emit("post_updated", &updated_post);
            }
        }

        Ok(total_new_comments)
    }
}

/// Background worker running every 30 seconds to check active watchlist items due for incremental crawl
pub fn start_watchlist_worker(
    app_handle: AppHandle,
    tracker: Arc<WatchlistTracker>,
    db: Arc<Database>,
    license: Arc<LicenseManager>,
) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;

            // Only crawl if app is strictly authenticated
            if !license.is_authenticated.load(Ordering::SeqCst) {
                continue;
            }

            let posts = match db.get_monitored_posts() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let now = Utc::now().timestamp();

            for post in posts {
                if post.status != "ACTIVE" {
                    continue;
                }

                let interval_secs = (post.crawl_interval_minutes.max(1)) * 60;
                let last_crawl = post.last_crawled_at.unwrap_or(0);

                if now - last_crawl >= interval_secs {
                    let tracker_ref = tracker.clone();
                    let post_id = post.post_id.clone();
                    let handle_clone = app_handle.clone();

                    tauri::async_runtime::spawn(async move {
                        let _ = tracker_ref.crawl_post_incremental(&post_id, Some(&handle_clone)).await;
                    });
                }
            }
        }
    });
}
