export interface MonitoredPost {
  post_id: string;
  post_url: string;
  author_name?: string | null;
  content_preview?: string | null;
  total_comments_crawled: number;
  total_orders_detected: number;
  total_phones_detected: number;
  status: 'ACTIVE' | 'PAUSED' | 'COMPLETED' | 'ERROR';
  crawl_interval_minutes: number;
  last_cursor?: string | null;
  last_crawled_at?: number | null;
  created_at: number;
}

export interface CommentItem {
  id: string;
  post_id: string;
  parent_comment_id?: string | null;
  author_id?: string | null;
  author_name?: string | null;
  author_url?: string | null;
  content?: string | null;
  phone_numbers?: string | null;
  intent_tag?: '[Chốt đơn]' | '[Hỏi giá]' | '[Khiếu nại]' | '[Spam]' | string | null;
  created_at?: number | null;
}

export interface LicenseStateInfo {
  is_authenticated: boolean;
  license_key?: string | null;
  client_name?: string | null;
  expires_at?: string | null;
  hwid: string;
  server_url: string;
}
