import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Plus,
  Trash2,
  RefreshCw,
  FileSpreadsheet,
  MessageSquare,
  PhoneCall,
  ShoppingBag,
  ExternalLink,
  Search,
  Clock,
  Sparkles,
  ShieldCheck,
  AlertCircle,
  Pencil,
  AlertTriangle,
  X,
  Save
} from 'lucide-react';
import { MonitoredPost } from '../types';

export const SCAN_INTERVAL_OPTIONS = [
  { value: 1, label: '1 phút (Siêu tốc / Thử nghiệm)' },
  { value: 5, label: '5 phút (Rất nhanh)' },
  { value: 10, label: '10 phút (Nhanh)' },
  { value: 15, label: '15 phút (Khuyến nghị)' },
  { value: 30, label: '30 phút (Mặc định)' },
  { value: 60, label: '1 giờ' },
  { value: 120, label: '2 giờ' },
  { value: 360, label: '6 giờ' },
  { value: 720, label: '12 giờ' },
  { value: 1440, label: '24 giờ (1 ngày)' }
];

interface PostTrackerDashboardProps {
  onOpenComments: (postId: string, postTitle?: string, postPreview?: string, postUrl?: string) => void;
  onOpenSessionModal: () => void;
}

export const PostTrackerDashboard: React.FC<PostTrackerDashboardProps> = ({
  onOpenComments,
  onOpenSessionModal
}) => {
  const [posts, setPosts] = useState<MonitoredPost[]>([]);
  const [loading, setLoading] = useState(false);
  const [inputUrl, setInputUrl] = useState('');
  const [intervalMinutes, setIntervalMinutes] = useState<number>(30);
  const [searchFilter, setSearchFilter] = useState('');
  const [scanningId, setScanningId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Edit Modal State
  const [editingPost, setEditingPost] = useState<MonitoredPost | null>(null);
  const [editAuthorName, setEditAuthorName] = useState('');
  const [editContentPreview, setEditContentPreview] = useState('');
  const [editInterval, setEditInterval] = useState<number>(30);
  const [editStatus, setEditStatus] = useState<string>('ACTIVE');
  const [savingEdit, setSavingEdit] = useState(false);

  // Delete Confirmation State (In-app modal to replace blocked window.confirm)
  const [postToDelete, setPostToDelete] = useState<MonitoredPost | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Fetch monitored posts (silently = no loading spinner for background polls)
  const loadPosts = async (silent = false) => {
    try {
      if (!silent) setLoading(true);
      const res = await invoke<MonitoredPost[]>('get_monitored_posts');
      setPosts(res);
      setErrorMessage(null);
    } catch (err: any) {
      console.error('Failed to load posts:', err);
      if (!silent) setErrorMessage(String(err));
    } finally {
      if (!silent) setLoading(false);
    }
  };

  useEffect(() => {
    loadPosts();

    // Listen to real-time events emitted from Tauri background worker
    let unlistenPostUpdated: (() => void) | undefined;
    let unlistenCheckpoint: (() => void) | undefined;
    let unlistenInitialCrawl: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        const u1 = await listen<MonitoredPost>('post_updated', (event) => {
          const updated = event.payload;
          setPosts((prev) => {
            const idx = prev.findIndex((p) => p.post_id === updated.post_id);
            if (idx >= 0) {
              const clone = [...prev];
              clone[idx] = updated;
              return clone;
            }
            return [updated, ...prev];
          });
        });
        unlistenPostUpdated = u1;

        const u2 = await listen<{ post_id: string; message: string }>('facebook_checkpoint_detected', (event) => {
          alert(`CẢNH BÁO FACEBOOK: ${event.payload.message}`);
        });
        unlistenCheckpoint = u2;

        // Listen for initial crawl completion
        const u3 = await listen<{ post_id: string; new_comments: number }>('post_initial_crawl_done', (_event) => {
          // Reload posts to get accurate count after full initial crawl
          loadPosts();
        });
        unlistenInitialCrawl = u3;
      } catch (e) {
        console.warn('Tauri event listen warning (browser mode fallback):', e);
      }
    };

    setupListeners();

    // Auto-poll every 3 seconds to keep post stats up-to-date
    // This is the fallback when Tauri events don't reach the WebView
    const pollInterval = setInterval(() => {
      loadPosts(true); // silent refresh - no loading spinner
    }, 3000);

    return () => {
      clearInterval(pollInterval);
      if (unlistenPostUpdated) unlistenPostUpdated();
      if (unlistenCheckpoint) unlistenCheckpoint();
      if (unlistenInitialCrawl) unlistenInitialCrawl();
    };
  }, []);

  // Handle Add Post
  const handleAddPost = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputUrl.trim()) return;

    try {
      setLoading(true);
      setErrorMessage(null);
      const newPost = await invoke<MonitoredPost>('add_monitored_post', {
        urlOrId: inputUrl.trim(),
        crawlIntervalMinutes: intervalMinutes
      });

      setPosts((prev) => [newPost, ...prev.filter((p) => p.post_id !== newPost.post_id)]);
      setInputUrl('');
    } catch (err: any) {
      setErrorMessage(String(err));
    } finally {
      setLoading(false);
    }
  };

  // Toggle Status
  const handleToggleStatus = async (post: MonitoredPost) => {
    const nextStatus = post.status === 'ACTIVE' ? 'PAUSED' : 'ACTIVE';
    try {
      await invoke('toggle_post_status', { postId: post.post_id, status: nextStatus });
      setPosts((prev) =>
        prev.map((p) => (p.post_id === post.post_id ? { ...p, status: nextStatus as any } : p))
      );
    } catch (err: any) {
      alert(`Lỗi đổi trạng thái: ${err}`);
    }
  };

  // Open Edit Modal
  const handleOpenEdit = (post: MonitoredPost) => {
    setEditingPost(post);
    setEditAuthorName(post.author_name || '');
    setEditContentPreview(post.content_preview || '');
    setEditInterval(post.crawl_interval_minutes || 30);
    setEditStatus(post.status);
  };

  // Save Edit
  const handleSaveEdit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editingPost) return;

    try {
      setSavingEdit(true);
      const updated = await invoke<MonitoredPost>('update_monitored_post', {
        postId: editingPost.post_id,
        authorName: editAuthorName.trim() || undefined,
        contentPreview: editContentPreview.trim() || undefined,
        crawlIntervalMinutes: editInterval,
        status: editStatus
      });

      setPosts((prev) => prev.map((p) => (p.post_id === updated.post_id ? updated : p)));
      setEditingPost(null);
    } catch (err: any) {
      alert(`Lỗi cập nhật bài viết: ${err}`);
    } finally {
      setSavingEdit(false);
    }
  };

  // Confirm Delete
  const handleConfirmDelete = async () => {
    if (!postToDelete) return;
    try {
      setDeleting(true);
      await invoke('delete_monitored_post', { postId: postToDelete.post_id });
      setPosts((prev) => prev.filter((p) => p.post_id !== postToDelete.post_id));
      setPostToDelete(null);
    } catch (err: any) {
      alert(`Lỗi xoá bài viết: ${err}`);
    } finally {
      setDeleting(false);
    }
  };

  // Scan Now
  const handleScanNow = async (postId: string) => {
    try {
      setScanningId(postId);
      const count = await invoke<number>('scan_post_now', { postId });
      await loadPosts();
      alert(`Quét hoàn tất! Đã bóc tách thêm ${count} bình luận mới.`);
    } catch (err: any) {
      alert(`Lỗi khi quét bài viết: ${err}`);
    } finally {
      setScanningId(null);
    }
  };

  // Export CSV
  const handleExportCSV = async (post: MonitoredPost) => {
    try {
      const comments = await invoke<any[]>('get_post_comments', {
        postId: post.post_id,
        intentFilter: null,
        limit: 5000,
        offset: 0
      });

      if (comments.length === 0) {
        alert('Chưa có bình luận nào để xuất Excel/CSV.');
        return;
      }

      const headers = ['Comment_ID', 'Post_ID', 'Nguoi_Binh_Luan', 'Noi_Dung', 'So_Dien_Thoai', 'Phan_Loai_Y_Dinh', 'Thoi_Gian'];
      const rows = comments.map((c) => [
        `"${c.id}"`,
        `"${c.post_id}"`,
        `"${(c.author_name || '').replace(/"/g, '""')}"`,
        `"${(c.content || '').replace(/"/g, '""')}"`,
        `"${c.phone_numbers || ''}"`,
        `"${c.intent_tag || ''}"`,
        `"${c.created_at ? new Date(c.created_at * 1000).toLocaleString('vi-VN') : ''}"`
      ]);

      const csvContent = '\uFEFF' + [headers.join(','), ...rows.map((r) => r.join(','))].join('\n');
      const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.setAttribute('href', url);
      link.setAttribute('download', `C9_Comments_${post.post_id}_${Date.now()}.csv`);
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
    } catch (err: any) {
      alert(`Lỗi xuất CSV: ${err}`);
    }
  };

  // Demo Data Injection
  const handleSeedDemo = async () => {
    try {
      setLoading(true);
      await invoke('seed_demo_data');
      await loadPosts();
      alert('Đã nạp thành công 2 bài viết mẫu cùng hơn 10 bình luận chốt đơn thực tế!');
    } catch (err: any) {
      alert(`Lỗi nạp demo: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const filteredPosts = posts.filter((p) => {
    if (!searchFilter.trim()) return true;
    const q = searchFilter.toLowerCase();
    return (
      p.post_id.toLowerCase().includes(q) ||
      (p.author_name && p.author_name.toLowerCase().includes(q)) ||
      (p.content_preview && p.content_preview.toLowerCase().includes(q))
    );
  });

  return (
    <div className="space-y-6">
      {/* Top Banner / Add Post Card */}
      <div className="p-6 rounded-2xl bg-[#1e293b]/70 border border-slate-700/80 shadow-xl backdrop-blur-md">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 mb-6">
          <div>
            <h1 className="text-xl font-bold text-white tracking-wide flex items-center gap-2.5">
              <span>Theo dõi & Bóc tách Bài viết (Watchlist Tracker)</span>
              <span className="text-xs px-2.5 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-semibold">
                Tự động cào định kỳ
              </span>
            </h1>
            <p className="text-xs text-slate-400 mt-1">
              Nhập link bài viết Facebook bất kỳ. Hệ thống sẽ bóc tách SĐT, đơn hàng và phân loại ý định khách hàng theo chu kỳ.
            </p>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={onOpenSessionModal}
              className="px-3.5 py-2 text-xs font-semibold rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700/80 transition-all flex items-center gap-2 shadow-sm"
            >
              <ShieldCheck className="w-4 h-4 text-emerald-400" />
              Cấu hình FB Session
            </button>
            <button
              onClick={handleSeedDemo}
              className="px-3.5 py-2 text-xs font-semibold rounded-xl bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 transition-all flex items-center gap-2"
            >
              <Sparkles className="w-4 h-4" />
              Dữ liệu Mẫu
            </button>
            <button
              onClick={() => loadPosts(false)}
              disabled={loading}
              className="p-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700/80 transition-all"
              title="Làm mới danh sách"
            >
              <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            </button>
          </div>
        </div>

        {/* Input Form */}
        <form onSubmit={handleAddPost} className="grid grid-cols-1 md:grid-cols-12 gap-3">
          <div className="md:col-span-8">
            <input
              type="text"
              placeholder="Dán link bài viết Facebook (VD: https://www.facebook.com/... hoặc Post ID)..."
              value={inputUrl}
              onChange={(e) => setInputUrl(e.target.value)}
              className="w-full px-4 py-3 rounded-xl bg-slate-900/90 border border-slate-700/80 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 focus:border-emerald-500 transition-all text-sm font-medium"
            />
          </div>
          <div className="md:col-span-2">
            <select
              value={intervalMinutes}
              onChange={(e) => setIntervalMinutes(Number(e.target.value))}
              className="w-full px-3 py-3 rounded-xl bg-slate-900/90 border border-slate-700/80 text-slate-200 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 focus:border-emerald-500 text-sm font-medium"
            >
              {SCAN_INTERVAL_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>
          <div className="md:col-span-2">
            <button
              type="submit"
              disabled={loading || !inputUrl.trim()}
              className="w-full h-full py-3 px-4 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white font-semibold text-sm shadow-lg shadow-emerald-600/20 disabled:opacity-50 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2"
            >
              <Plus className="w-4 h-4" />
              Thêm Theo Dõi
            </button>
          </div>
        </form>

        {errorMessage && (
          <div className="mt-4 p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>{errorMessage}</span>
          </div>
        )}
      </div>

      {/* Posts Table Card */}
      <div className="rounded-2xl bg-[#1e293b]/70 border border-slate-700/80 shadow-xl overflow-hidden backdrop-blur-md">
        {/* Search Toolbar */}
        <div className="p-4 border-b border-slate-800 flex items-center justify-between gap-4">
          <div className="relative w-full max-w-sm">
            <Search className="w-4 h-4 absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-400" />
            <input
              type="text"
              placeholder="Tìm kiếm theo ID, Tác giả, Nội dung..."
              value={searchFilter}
              onChange={(e) => setSearchFilter(e.target.value)}
              className="w-full pl-9 pr-4 py-2 rounded-xl bg-slate-900/90 border border-slate-700/80 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-emerald-500"
            />
          </div>

          <div className="text-xs text-slate-400 font-medium">
            Đang theo dõi: <strong className="text-white">{posts.length}</strong> bài viết
          </div>
        </div>

        {/* Table */}
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm text-slate-300">
            <thead className="bg-slate-900/80 text-xs uppercase tracking-wider text-slate-400 font-semibold border-b border-slate-800">
              <tr>
                <th className="py-4 px-5">Bài viết & Tác giả</th>
                <th className="py-4 px-4 text-center">Thống kê Bóc tách</th>
                <th className="py-4 px-4 text-center">Chu kỳ</th>
                <th className="py-4 px-4 text-center">Lần quét cuối</th>
                <th className="py-4 px-4 text-center">Trạng thái</th>
                <th className="py-4 px-5 text-right">Thao tác</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/60">
              {filteredPosts.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-12 text-center text-slate-500 text-sm">
                    {loading ? (
                      <div className="flex items-center justify-center gap-2">
                        <RefreshCw className="w-5 h-5 animate-spin text-emerald-500" />
                        Đang tải danh sách bài viết...
                      </div>
                    ) : (
                      'Chưa có bài viết nào trong danh sách theo dõi. Hãy dán link hoặc nhấn "Dữ liệu Mẫu" để thử nghiệm.'
                    )}
                  </td>
                </tr>
              ) : (
                filteredPosts.map((post) => (
                  <tr
                    key={post.post_id}
                    className="hover:bg-slate-800/30 transition-colors group"
                  >
                    {/* Post ID & Preview */}
                    <td className="py-4 px-5 max-w-sm">
                      <div className="flex items-start gap-3">
                        <div className="p-2 rounded-lg bg-slate-800/80 border border-slate-700/60 text-slate-300 mt-1">
                          <MessageSquare className="w-4 h-4 text-emerald-400" />
                        </div>
                        <div className="space-y-1 overflow-hidden">
                          <div className="flex items-center gap-2">
                            <span className="font-semibold text-white truncate text-sm">
                              {post.author_name || 'Facebook User'}
                            </span>
                            <a
                              href={post.post_url}
                              target="_blank"
                              rel="noreferrer"
                              className="text-slate-400 hover:text-emerald-400 transition-colors"
                              title="Mở link bài viết gốc"
                            >
                              <ExternalLink className="w-3.5 h-3.5" />
                            </a>
                          </div>
                          <p className="text-xs text-slate-400 line-clamp-2 leading-relaxed">
                            {post.content_preview || 'Không có bản xem trước.'}
                          </p>
                          <div className="text-[11px] font-mono text-slate-500">
                            ID: {post.post_id}
                          </div>
                        </div>
                      </div>
                    </td>

                    {/* Stats Badges */}
                    <td className="py-4 px-4 text-center">
                      <div className="flex items-center justify-center gap-2 flex-wrap">
                        {/* Total Comments */}
                        <span
                          className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-sky-500/10 text-sky-300 border border-sky-500/20"
                          title="Tổng bình luận đã cào"
                        >
                          <MessageSquare className="w-3 h-3" />
                          {post.total_comments_crawled}
                        </span>

                        {/* Total Phones */}
                        <span
                          className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-300 border border-emerald-500/20"
                          title="Số điện thoại phát hiện"
                        >
                          <PhoneCall className="w-3 h-3" />
                          {post.total_phones_detected}
                        </span>

                        {/* Total Orders */}
                        <span
                          className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-amber-500/10 text-amber-300 border border-amber-500/20"
                          title="Bình luận [Chốt đơn] tự động bóc tách"
                        >
                          <ShoppingBag className="w-3 h-3" />
                          {post.total_orders_detected}
                        </span>
                      </div>
                    </td>

                    {/* Interval */}
                    <td className="py-4 px-4 text-center text-xs text-slate-300 font-medium">
                      <div className="inline-flex items-center gap-1 text-slate-400">
                        <Clock className="w-3.5 h-3.5 text-emerald-400/80" />
                        {post.crawl_interval_minutes}m
                      </div>
                    </td>

                    {/* Last Crawl */}
                    <td className="py-4 px-4 text-center text-xs text-slate-400">
                      {post.last_crawled_at
                        ? new Date(post.last_crawled_at * 1000).toLocaleTimeString('vi-VN', {
                            hour: '2-digit',
                            minute: '2-digit',
                            second: '2-digit'
                          })
                        : 'Chưa quét'}
                    </td>

                    {/* Status & Switch */}
                    <td className="py-4 px-4 text-center">
                      <div className="inline-flex items-center gap-2">
                        <button
                          onClick={() => handleToggleStatus(post)}
                          className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                            post.status === 'ACTIVE' ? 'bg-emerald-500' : 'bg-slate-700'
                          }`}
                          title={post.status === 'ACTIVE' ? 'Bấm để tạm dừng quét' : 'Bấm để kích hoạt quét'}
                        >
                          <span
                            className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${
                              post.status === 'ACTIVE' ? 'translate-x-4' : 'translate-x-0'
                            }`}
                          />
                        </button>
                        <span
                          className={`text-xs font-semibold px-2 py-0.5 rounded-md ${
                            post.status === 'ACTIVE'
                              ? 'text-emerald-400 bg-emerald-500/10'
                              : post.status === 'PAUSED'
                              ? 'text-amber-400 bg-amber-500/10'
                              : 'text-rose-400 bg-rose-500/10'
                          }`}
                        >
                          {post.status}
                        </span>
                      </div>
                    </td>

                    {/* Actions */}
                    <td className="py-4 px-5 text-right">
                      <div className="flex items-center justify-end gap-1.5">
                        {/* Scan Now Button */}
                        <button
                          onClick={() => handleScanNow(post.post_id)}
                          disabled={scanningId === post.post_id}
                          className="p-2 rounded-lg bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 transition-all disabled:opacity-50"
                          title="Quét ngay lập tức (Incremental)"
                        >
                          <RefreshCw
                            className={`w-4 h-4 ${scanningId === post.post_id ? 'animate-spin' : ''}`}
                          />
                        </button>

                        {/* View Comments Button */}
                        <button
                          onClick={() => onOpenComments(post.post_id, post.author_name || undefined, post.content_preview || undefined, post.post_url)}
                          className="px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700/80 text-xs font-medium transition-all flex items-center gap-1.5"
                          title="Xem chi tiết danh sách bình luận & đơn hàng"
                        >
                          <MessageSquare className="w-3.5 h-3.5 text-teal-400" />
                          Comments
                        </button>

                        {/* Export Excel Button */}
                        <button
                          onClick={() => handleExportCSV(post)}
                          className="p-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700/80 transition-all"
                          title="Xuất Excel / CSV bình luận"
                        >
                          <FileSpreadsheet className="w-4 h-4 text-emerald-400" />
                        </button>

                        {/* Edit Post Button */}
                        <button
                          onClick={() => handleOpenEdit(post)}
                          className="p-2 rounded-lg bg-indigo-500/10 hover:bg-indigo-500/20 text-indigo-300 border border-indigo-500/30 transition-all"
                          title="Chỉnh sửa thông tin bài viết"
                        >
                          <Pencil className="w-4 h-4" />
                        </button>

                        {/* Delete Post Button */}
                        <button
                          onClick={() => setPostToDelete(post)}
                          className="p-2 rounded-lg bg-rose-500/10 hover:bg-rose-500/20 text-rose-300 border border-rose-500/20 transition-all"
                          title="Xoá bài viết này"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* 1. Edit Post Modal */}
      {editingPost && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-fadeIn">
          <div className="w-full max-w-xl bg-[#0f172a] border border-slate-700/90 rounded-2xl shadow-2xl overflow-hidden flex flex-col">
            <div className="px-6 py-4 border-b border-slate-800 flex items-center justify-between bg-slate-900/80">
              <div className="flex items-center gap-2.5">
                <div className="p-2 rounded-lg bg-indigo-500/10 border border-indigo-500/30 text-indigo-400">
                  <Pencil className="w-4 h-4" />
                </div>
                <div>
                  <h3 className="text-base font-bold text-white">Chỉnh Sửa Bài Viết Theo Dõi</h3>
                  <p className="text-xs font-mono text-slate-400">ID: {editingPost.post_id}</p>
                </div>
              </div>
              <button
                onClick={() => setEditingPost(null)}
                className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleSaveEdit} className="p-6 space-y-4">
              <div>
                <label className="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-1.5">
                  Tên Tác Giả / Gợi nhớ Fanpage
                </label>
                <input
                  type="text"
                  value={editAuthorName}
                  onChange={(e) => setEditAuthorName(e.target.value)}
                  placeholder="VD: Vua Nệm, Shop Thời Trang..."
                  className="w-full px-3.5 py-2.5 rounded-xl bg-slate-900 border border-slate-700 text-sm text-white placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-1.5">
                  Tóm Tắt Nội Dung Bài Viết
                </label>
                <textarea
                  rows={3}
                  value={editContentPreview}
                  onChange={(e) => setEditContentPreview(e.target.value)}
                  placeholder="Nội dung bài viết..."
                  className="w-full px-3.5 py-2.5 rounded-xl bg-slate-900 border border-slate-700 text-sm text-white placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 leading-relaxed"
                />
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-1.5">
                    Chu Kỳ Quét Tự Động
                  </label>
                  <select
                    value={editInterval}
                    onChange={(e) => setEditInterval(Number(e.target.value))}
                    className="w-full px-3.5 py-2.5 rounded-xl bg-slate-900 border border-slate-700 text-sm text-white focus:outline-none focus:border-indigo-500"
                  >
                    {SCAN_INTERVAL_OPTIONS.map((opt) => (
                      <option key={opt.value} value={opt.value}>
                        {opt.label}
                      </option>
                    ))}
                  </select>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-1.5">
                    Trạng Thái Hoạt Động
                  </label>
                  <select
                    value={editStatus}
                    onChange={(e) => setEditStatus(e.target.value)}
                    className="w-full px-3.5 py-2.5 rounded-xl bg-slate-900 border border-slate-700 text-sm text-white focus:outline-none focus:border-indigo-500"
                  >
                    <option value="ACTIVE">ACTIVE (Đang tự động quét)</option>
                    <option value="PAUSED">PAUSED (Tạm dừng quét)</option>
                  </select>
                </div>
              </div>

              <div className="pt-4 border-t border-slate-800 flex items-center justify-end gap-2.5">
                <button
                  type="button"
                  onClick={() => setEditingPost(null)}
                  className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-300 font-semibold text-xs transition-colors"
                >
                  Hủy bỏ
                </button>
                <button
                  type="submit"
                  disabled={savingEdit}
                  className="px-4 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white font-semibold text-xs transition-all flex items-center gap-1.5 shadow-lg shadow-indigo-600/20 disabled:opacity-50"
                >
                  <Save className="w-4 h-4" />
                  {savingEdit ? 'Đang lưu...' : 'Lưu Thay Đổi'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* 2. In-App Delete Confirmation Modal (Reliable on macOS WebKit) */}
      {postToDelete && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-fadeIn">
          <div className="w-full max-w-md bg-[#0f172a] border border-rose-500/30 rounded-2xl shadow-2xl p-6 space-y-4">
            <div className="flex items-start gap-3.5">
              <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-400 shrink-0">
                <AlertTriangle className="w-6 h-6" />
              </div>
              <div className="space-y-1">
                <h3 className="text-base font-bold text-white">Xác Nhận Ngừng Theo Dõi & Xoá</h3>
                <p className="text-xs text-slate-400 leading-relaxed">
                  Bạn có chắc chắn muốn ngừng theo dõi và xoá toàn bộ bình luận đã bóc tách của bài viết này không? Thao tác này không thể hoàn tác.
                </p>
              </div>
            </div>

            <div className="p-3 rounded-xl bg-slate-900 border border-slate-800 text-xs space-y-1 text-slate-300">
              <div><strong>Tác giả:</strong> {postToDelete.author_name || 'Facebook User'}</div>
              <div className="truncate"><strong>Nội dung:</strong> {postToDelete.content_preview || 'Không có bản xem trước.'}</div>
              <div className="font-mono text-slate-500 text-[11px]">ID: {postToDelete.post_id}</div>
            </div>

            <div className="flex items-center justify-end gap-2.5 pt-2">
              <button
                type="button"
                onClick={() => setPostToDelete(null)}
                disabled={deleting}
                className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-300 font-semibold text-xs transition-colors"
              >
                Hủy bỏ
              </button>
              <button
                type="button"
                onClick={handleConfirmDelete}
                disabled={deleting}
                className="px-4 py-2 rounded-xl bg-rose-600 hover:bg-rose-500 text-white font-semibold text-xs transition-all flex items-center gap-1.5 shadow-lg shadow-rose-600/20 disabled:opacity-50"
              >
                <Trash2 className="w-4 h-4" />
                {deleting ? 'Đang xoá...' : 'Xác Nhận Xoá'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
