import React, { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  X,
  PhoneCall,
  Copy,
  Check,
  Search,
  ExternalLink,
  RefreshCw,
  Quote,
  ShoppingBag,
  MessageCircle,
  AlertTriangle,
  Ban,
  Sparkles,
  ChevronDown,
  ChevronRight,
  MessageSquare,
  CornerDownRight,
  Maximize2,
  Minimize2,
  Users
} from 'lucide-react';
import { CommentItem } from '../types';

interface CommentsModalProps {
  postId: string;
  postTitle?: string;
  postPreview?: string;
  postUrl?: string;
  onClose: () => void;
  onPostUpdated?: () => void;
}

interface CommentThread {
  parent: CommentItem;
  replies: CommentItem[];
  matchingReplyCount?: number;
}

export const CommentsModal: React.FC<CommentsModalProps> = ({
  postId,
  postTitle,
  postPreview,
  postUrl,
  onClose,
  onPostUpdated
}) => {
  const [comments, setComments] = useState<CommentItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [selectedIntent, setSelectedIntent] = useState<string>('ALL');
  const [searchTerm, setSearchTerm] = useState('');
  const [phoneOnlyFilter, setPhoneOnlyFilter] = useState(false);
  const [copiedPhone, setCopiedPhone] = useState<string | null>(null);
  const [copiedCommentId, setCopiedCommentId] = useState<string | null>(null);
  const [expandedThreads, setExpandedThreads] = useState<Record<string, boolean>>({});

  const fetchComments = async () => {
    try {
      setLoading(true);
      const res = await invoke<CommentItem[]>('get_post_comments', {
        postId,
        intentFilter: null, // Fetch all so tree relationships remain complete
        limit: 5000,
        offset: 0
      });
      setComments(res);
    } catch (err: any) {
      console.error('Failed to load comments:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchComments();
  }, [postId]);

  const handleScanNow = async () => {
    try {
      setScanning(true);
      const count = await invoke<number>('scan_post_now', { postId });
      await fetchComments();
      if (onPostUpdated) onPostUpdated();
      alert(`Quét hoàn tất! Đã bóc tách được ${count} bình luận mới.`);
    } catch (err: any) {
      alert(`Lỗi quét bài viết: ${err}`);
    } finally {
      setScanning(false);
    }
  };

  const copyToClipboard = (text: string, type: 'phone' | 'comment', id?: string) => {
    navigator.clipboard.writeText(text);
    if (type === 'phone') {
      setCopiedPhone(text);
      setTimeout(() => setCopiedPhone(null), 2000);
    } else if (id) {
      setCopiedCommentId(id);
      setTimeout(() => setCopiedCommentId(null), 2000);
    }
  };

  const getInitials = (name?: string | null) => {
    if (!name) return 'FB';
    const words = name.trim().split(/\s+/);
    if (words.length === 1) return words[0].substring(0, 2).toUpperCase();
    return (words[0][0] + words[words.length - 1][0]).toUpperCase();
  };

  const renderIntentBadge = (tag?: string | null) => {
    switch (tag) {
      case '[Chốt đơn]':
        return (
          <span className="inline-flex items-center gap-1 text-[11px] font-bold px-2.5 py-0.5 rounded-full bg-amber-500/20 text-amber-300 border border-amber-500/40">
            <ShoppingBag className="w-3 h-3 text-amber-400" />
            Chốt đơn
          </span>
        );
      case '[Hỏi giá]':
        return (
          <span className="inline-flex items-center gap-1 text-[11px] font-bold px-2.5 py-0.5 rounded-full bg-sky-500/20 text-sky-300 border border-sky-500/40">
            <MessageCircle className="w-3 h-3 text-sky-400" />
            Hỏi giá
          </span>
        );
      case '[Khiếu nại]':
        return (
          <span className="inline-flex items-center gap-1 text-[11px] font-bold px-2.5 py-0.5 rounded-full bg-rose-500/20 text-rose-300 border border-rose-500/40">
            <AlertTriangle className="w-3 h-3 text-rose-400" />
            Khiếu nại
          </span>
        );
      case '[Spam]':
        return (
          <span className="inline-flex items-center gap-1 text-[11px] font-bold px-2.5 py-0.5 rounded-full bg-purple-500/20 text-purple-300 border border-purple-500/40">
            <Ban className="w-3 h-3 text-purple-400" />
            Spam
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center gap-1 text-[11px] font-medium px-2 py-0.5 rounded-full bg-slate-700/60 text-slate-300 border border-slate-600/40">
            Khác
          </span>
        );
    }
  };

  // Build hierarchical threads (parent -> replies)
  const { threads, topLevelCount, repliesCount, totalOrders, totalPhones } = useMemo(() => {
    const commentMap = new Map<string, CommentItem>();
    comments.forEach(c => commentMap.set(c.id, c));

    const repliesByParent = new Map<string, CommentItem[]>();
    const topLevelList: CommentItem[] = [];

    comments.forEach(c => {
      let pid = c.parent_comment_id;
      if (pid && pid.includes('_')) {
        pid = pid.split('_').pop() || pid;
      }
      if (pid && pid !== c.id && commentMap.has(pid)) {
        const list = repliesByParent.get(pid) || [];
        list.push(c);
        repliesByParent.set(pid, list);
      } else {
        topLevelList.push(c);
      }
    });

    // Sort top level by created_at DESC (newest first)
    topLevelList.sort((a, b) => (b.created_at || 0) - (a.created_at || 0));

    // Sort replies inside each thread chronologically (ASC)
    repliesByParent.forEach((list) => {
      list.sort((a, b) => (a.created_at || 0) - (b.created_at || 0));
    });

    const threadList: CommentThread[] = topLevelList.map(parent => ({
      parent,
      replies: repliesByParent.get(parent.id) || []
    }));

    let repliesTotal = 0;
    repliesByParent.forEach(list => { repliesTotal += list.length; });

    const orders = comments.filter(c => c.intent_tag === '[Chốt đơn]').length;
    const phones = comments.filter(c => !!c.phone_numbers).length;

    return {
      threads: threadList,
      topLevelCount: topLevelList.length,
      repliesCount: repliesTotal,
      totalOrders: orders,
      totalPhones: phones
    };
  }, [comments]);

  // Matcher for individual comment
  const matchesComment = (c: CommentItem) => {
    if (selectedIntent !== 'ALL' && c.intent_tag !== selectedIntent) {
      return false;
    }
    if (phoneOnlyFilter && !c.phone_numbers) {
      return false;
    }
    if (searchTerm.trim()) {
      const q = searchTerm.toLowerCase();
      const inAuthor = c.author_name && c.author_name.toLowerCase().includes(q);
      const inContent = c.content && c.content.toLowerCase().includes(q);
      const inPhone = c.phone_numbers && c.phone_numbers.includes(q);
      return inAuthor || inContent || inPhone;
    }
    return true;
  };

  // Filter threads: a thread appears if parent matches OR any reply matches
  const filteredThreads = useMemo(() => {
    return threads
      .map(t => {
        const parentMatches = matchesComment(t.parent);
        const matchingReplies = t.replies.filter(matchesComment);
        const hasMatch = parentMatches || matchingReplies.length > 0;

        return {
          ...t,
          hasMatch,
          parentMatches,
          matchingReplyCount: matchingReplies.length
        };
      })
      .filter(t => t.hasMatch);
  }, [threads, selectedIntent, searchTerm, phoneOnlyFilter]);

  // Auto-expand threads with matching replies when search/filter is active
  useEffect(() => {
    if (selectedIntent !== 'ALL' || phoneOnlyFilter || searchTerm.trim()) {
      const newExpanded: Record<string, boolean> = {};
      filteredThreads.forEach(t => {
        if (t.replies.length > 0) {
          newExpanded[t.parent.id] = true;
        }
      });
      setExpandedThreads(prev => ({ ...prev, ...newExpanded }));
    }
  }, [selectedIntent, phoneOnlyFilter, searchTerm, filteredThreads]);

  const toggleThread = (parentId: string) => {
    setExpandedThreads(prev => ({
      ...prev,
      [parentId]: !prev[parentId]
    }));
  };

  const handleExpandAll = () => {
    const next: Record<string, boolean> = {};
    threads.forEach(t => {
      if (t.replies.length > 0) next[t.parent.id] = true;
    });
    setExpandedThreads(next);
  };

  const handleCollapseAll = () => {
    setExpandedThreads({});
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-2 md:p-6 bg-black/80 backdrop-blur-md animate-fadeIn">
      <div className="w-full max-w-5xl h-[92vh] bg-[#0f172a] border border-slate-700/80 rounded-2xl shadow-2xl flex flex-col overflow-hidden">
        {/* Header */}
        <div className="px-6 py-4 border-b border-slate-800 flex items-center justify-between bg-slate-900/90 backdrop-blur-sm shrink-0">
          <div className="space-y-1">
            <div className="flex items-center gap-2.5">
              <span className="text-xs font-mono px-2.5 py-0.5 rounded-md bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 font-semibold">
                ID: {postId}
              </span>
              <h2 className="text-lg font-bold text-white tracking-wide flex items-center gap-2">
                <Sparkles className="w-4 h-4 text-emerald-400" />
                Bóc tách bình luận & Phân cấp câu trả lời
              </h2>
            </div>
            {postTitle && (
              <div className="flex items-center gap-2 text-xs text-slate-300 font-medium">
                <span>Tác giả: <strong>{postTitle}</strong></span>
                {postUrl && (
                  <a
                    href={postUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="text-emerald-400 hover:underline flex items-center gap-1 ml-1"
                  >
                    Xem bài viết <ExternalLink className="w-3 h-3" />
                  </a>
                )}
              </div>
            )}
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={handleScanNow}
              disabled={scanning}
              className="px-3 py-1.5 rounded-xl bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300 border border-emerald-500/40 text-xs font-semibold transition-all flex items-center gap-1.5 shadow-sm"
              title="Quét lại bài viết này ngay lập tức"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${scanning ? 'animate-spin' : ''}`} />
              {scanning ? 'Đang quét...' : 'Quét Ngay'}
            </button>
            <button
              onClick={fetchComments}
              disabled={loading}
              className="p-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-all"
              title="Tải lại danh sách"
            >
              <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            </button>
            <button
              onClick={onClose}
              className="p-2 rounded-xl bg-slate-800 hover:bg-rose-600 text-slate-300 hover:text-white border border-slate-700 transition-all"
              title="Đóng cửa sổ"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Post Preview Callout */}
        {postPreview && (
          <div className="px-6 py-2.5 bg-slate-900/50 border-b border-slate-800/80 flex items-start gap-2.5 shrink-0">
            <Quote className="w-4 h-4 text-emerald-400/80 shrink-0 mt-0.5" />
            <div className="flex-1 overflow-hidden">
              <span className="text-[11px] font-semibold text-emerald-400 uppercase tracking-wider block mb-0.5">
                Nội dung bài viết đang theo dõi:
              </span>
              <p className="text-xs text-slate-300 line-clamp-2 leading-relaxed italic">
                "{postPreview}"
              </p>
            </div>
          </div>
        )}

        {/* Quick Stats Bar */}
        <div className="px-6 py-2 bg-slate-950/60 border-b border-slate-800/60 flex items-center justify-between text-xs text-slate-400 shrink-0">
          <div className="flex items-center gap-4 flex-wrap">
            <div className="flex items-center gap-1.5 text-slate-200 font-semibold">
              <Users className="w-3.5 h-3.5 text-emerald-400" />
              <span>Tổng: {comments.length} bình luận</span>
              <span className="text-slate-500 font-normal">
                ({topLevelCount} bình luận gốc + {repliesCount} câu trả lời)
              </span>
            </div>
            <div className="h-3 w-[1px] bg-slate-700" />
            <div className="flex items-center gap-1.5 text-amber-300 font-medium">
              <ShoppingBag className="w-3.5 h-3.5" />
              <span>Chốt đơn: {totalOrders}</span>
            </div>
            <div className="h-3 w-[1px] bg-slate-700" />
            <div className="flex items-center gap-1.5 text-emerald-300 font-medium">
              <PhoneCall className="w-3.5 h-3.5" />
              <span>Có SĐT: {totalPhones}</span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={handleExpandAll}
              className="text-[11px] text-slate-400 hover:text-emerald-400 transition-colors flex items-center gap-1"
              title="Mở tất cả câu trả lời"
            >
              <Maximize2 className="w-3 h-3" /> Mở tất cả
            </button>
            <span className="text-slate-600">•</span>
            <button
              onClick={handleCollapseAll}
              className="text-[11px] text-slate-400 hover:text-emerald-400 transition-colors flex items-center gap-1"
              title="Thu gọn tất cả câu trả lời"
            >
              <Minimize2 className="w-3 h-3" /> Thu gọn
            </button>
          </div>
        </div>

        {/* Filters Toolbar */}
        <div className="px-6 py-3 border-b border-slate-800/80 bg-slate-900/40 flex flex-wrap items-center justify-between gap-3 shrink-0">
          {/* Intent Tabs */}
          <div className="flex items-center gap-1.5 flex-wrap">
            {[
              { id: 'ALL', label: `Tất cả (${comments.length})` },
              { id: '[Chốt đơn]', label: `🛒 Chốt đơn (${comments.filter(c => c.intent_tag === '[Chốt đơn]').length})` },
              { id: '[Hỏi giá]', label: `💬 Hỏi giá (${comments.filter(c => c.intent_tag === '[Hỏi giá]').length})` },
              { id: '[Khiếu nại]', label: `⚠️ Khiếu nại (${comments.filter(c => c.intent_tag === '[Khiếu nại]').length})` },
              { id: '[Spam]', label: `🚫 Spam (${comments.filter(c => c.intent_tag === '[Spam]').length})` }
            ].map((tab) => (
              <button
                key={tab.id}
                onClick={() => setSelectedIntent(tab.id)}
                className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-all border ${
                  selectedIntent === tab.id
                    ? 'bg-emerald-500/20 border-emerald-500/50 text-emerald-300 shadow-sm'
                    : 'bg-slate-800/60 border-slate-700/60 text-slate-400 hover:text-slate-200'
                }`}
              >
                {tab.label}
              </button>
            ))}

            <button
              onClick={() => setPhoneOnlyFilter(!phoneOnlyFilter)}
              className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-all border flex items-center gap-1.5 ${
                phoneOnlyFilter
                  ? 'bg-emerald-500/25 border-emerald-500 text-emerald-200 shadow-sm'
                  : 'bg-slate-800/60 border-slate-700/60 text-slate-400 hover:text-slate-200'
              }`}
            >
              <PhoneCall className="w-3 h-3 text-emerald-400" />
              Chỉ SĐT ({totalPhones})
            </button>
          </div>

          {/* Search Box */}
          <div className="relative w-full sm:w-64">
            <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
            <input
              type="text"
              placeholder="Lọc tên, SĐT, nội dung..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full pl-8 pr-3 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-emerald-500"
            />
          </div>
        </div>

        {/* Threaded Comments List */}
        <div className="flex-1 overflow-y-auto p-4 md:p-6 space-y-4">
          {loading ? (
            <div className="h-full flex flex-col items-center justify-center text-slate-400 text-sm gap-3">
              <RefreshCw className="w-6 h-6 animate-spin text-emerald-400" />
              <span>Đang tải danh sách bình luận...</span>
            </div>
          ) : filteredThreads.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center text-slate-400 text-sm gap-3 py-16">
              <div className="p-4 rounded-full bg-slate-800/80 border border-slate-700 text-slate-500">
                <Search className="w-8 h-8" />
              </div>
              <p className="font-semibold text-slate-300">
                {comments.length === 0 ? 'Chưa có bình luận nào được lưu cho bài viết này.' : 'Không có bình luận nào phù hợp bộ lọc hiện tại.'}
              </p>
              {comments.length === 0 && (
                <button
                  onClick={handleScanNow}
                  disabled={scanning}
                  className="mt-2 px-4 py-2 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white font-semibold text-xs transition-all flex items-center gap-2 shadow-lg shadow-emerald-600/20"
                >
                  <RefreshCw className={`w-4 h-4 ${scanning ? 'animate-spin' : ''}`} />
                  Quét Bình Luận Ngay
                </button>
              )}
            </div>
          ) : (
            filteredThreads.map(({ parent: c, replies }) => {
              const isExpanded = expandedThreads[c.id] || false;
              const hasReplies = replies.length > 0;

              return (
                <div
                  key={c.id}
                  className="rounded-2xl bg-slate-900/80 border border-slate-800/90 hover:border-slate-700 transition-all p-4 shadow-sm"
                >
                  {/* Top-Level Comment Card */}
                  <div className="flex flex-col md:flex-row items-start justify-between gap-4">
                    <div className="space-y-2.5 flex-1 w-full">
                      {/* Author Header */}
                      <div className="flex items-center justify-between flex-wrap gap-2">
                        <div className="flex items-center gap-2.5">
                          {/* Avatar initials badge */}
                          <div className="w-8 h-8 rounded-full bg-gradient-to-tr from-emerald-600 to-teal-500 flex items-center justify-center text-white text-xs font-bold shadow-sm shrink-0">
                            {getInitials(c.author_name)}
                          </div>
                          <div>
                            <div className="flex items-center gap-1.5">
                              <span className="font-bold text-white text-sm">
                                {c.author_name || 'Người dùng Facebook'}
                              </span>
                              {c.author_url && (
                                <a
                                  href={c.author_url}
                                  target="_blank"
                                  rel="noreferrer"
                                  className="text-slate-400 hover:text-emerald-400 transition-colors"
                                  title="Mở trang cá nhân Facebook"
                                >
                                  <ExternalLink className="w-3 h-3" />
                                </a>
                              )}
                            </div>
                            <div className="text-[11px] text-slate-500 font-medium">
                              {c.created_at
                                ? new Date(c.created_at * 1000).toLocaleString('vi-VN')
                                : 'Không rõ thời gian'}
                            </div>
                          </div>
                        </div>

                        <div className="flex items-center gap-2">
                          {renderIntentBadge(c.intent_tag)}
                          <button
                            onClick={() => copyToClipboard(c.content || '', 'comment', c.id)}
                            className="p-1 rounded-md text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors"
                            title="Sao chép nội dung bình luận"
                          >
                            {copiedCommentId === c.id ? (
                              <Check className="w-3.5 h-3.5 text-emerald-400" />
                            ) : (
                              <Copy className="w-3.5 h-3.5" />
                            )}
                          </button>
                        </div>
                      </div>

                      {/* Comment Content Box */}
                      <div className="p-3.5 rounded-xl bg-slate-950/90 border border-slate-800/90 text-slate-100 text-sm leading-relaxed whitespace-pre-wrap select-text font-normal shadow-inner flex items-start gap-2.5">
                        <Quote className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5 opacity-60" />
                        <div className="flex-1 break-words">
                          {c.content || <span className="text-slate-500 italic">Không có nội dung văn bản</span>}
                        </div>
                      </div>
                    </div>

                    {/* Phone detection badge (Right Side) */}
                    {c.phone_numbers && (
                      <div className="shrink-0 w-full md:w-auto flex items-center justify-between md:justify-start gap-3 bg-emerald-500/15 border border-emerald-500/40 rounded-xl px-3.5 py-2.5 shadow-sm">
                        <div className="flex items-center gap-2">
                          <PhoneCall className="w-4 h-4 text-emerald-400 animate-pulse" />
                          <div>
                            <div className="text-[10px] uppercase font-bold text-emerald-400 tracking-wider">
                              Phát hiện SĐT
                            </div>
                            <div className="text-sm font-mono font-bold text-emerald-200">
                              {c.phone_numbers}
                            </div>
                          </div>
                        </div>
                        <button
                          onClick={() => copyToClipboard(c.phone_numbers!, 'phone')}
                          className="p-2 rounded-lg bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300 transition-colors"
                          title="Sao chép số điện thoại"
                        >
                          {copiedPhone === c.phone_numbers ? (
                            <Check className="w-4 h-4" />
                          ) : (
                            <Copy className="w-4 h-4" />
                          )}
                        </button>
                      </div>
                    )}
                  </div>

                  {/* Replies Toggle / Expand Bar */}
                  {hasReplies && (
                    <div className="mt-3 pt-2.5 border-t border-slate-800/60 flex items-center justify-between">
                      <button
                        onClick={() => toggleThread(c.id)}
                        className="inline-flex items-center gap-1.5 text-xs font-semibold px-3 py-1 rounded-lg bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-300 border border-emerald-500/25 transition-all shadow-xs"
                      >
                        <MessageSquare className="w-3.5 h-3.5 text-emerald-400" />
                        <span>{replies.length} câu trả lời</span>
                        {isExpanded ? (
                          <ChevronDown className="w-3.5 h-3.5 text-emerald-400 ml-0.5" />
                        ) : (
                          <ChevronRight className="w-3.5 h-3.5 text-emerald-400 ml-0.5" />
                        )}
                      </button>

                      <span className="text-[11px] text-slate-500">
                        {isExpanded ? 'Nhấn để thu gọn' : 'Nhấn để xem câu trả lời'}
                      </span>
                    </div>
                  )}

                  {/* Nested Sub-Comments (Replies) Container */}
                  {hasReplies && isExpanded && (
                    <div className="mt-3 pl-3 md:pl-6 border-l-2 border-emerald-500/40 space-y-2.5 ml-2 md:ml-4">
                      {replies.map((reply) => {
                        const isMatch = matchesComment(reply);

                        return (
                          <div
                            key={reply.id}
                            className={`p-3 rounded-xl bg-slate-950/70 border transition-all flex flex-col md:flex-row items-start justify-between gap-3 ${
                              isMatch && (selectedIntent !== 'ALL' || searchTerm || phoneOnlyFilter)
                                ? 'border-emerald-500/60 ring-1 ring-emerald-500/30'
                                : 'border-slate-800/80 hover:border-slate-700/80'
                            }`}
                          >
                            <div className="space-y-2 flex-1 w-full">
                              {/* Reply Author Header */}
                              <div className="flex items-center justify-between flex-wrap gap-2">
                                <div className="flex items-center gap-2">
                                  <CornerDownRight className="w-3.5 h-3.5 text-emerald-400/80 shrink-0" />
                                  <div className="w-6 h-6 rounded-full bg-gradient-to-tr from-sky-600 to-indigo-500 flex items-center justify-center text-white text-[10px] font-bold shrink-0">
                                    {getInitials(reply.author_name)}
                                  </div>
                                  <div className="flex items-center gap-1.5">
                                    <span className="font-semibold text-slate-200 text-xs">
                                      {reply.author_name || 'Người dùng Facebook'}
                                    </span>
                                    {reply.author_url && (
                                      <a
                                        href={reply.author_url}
                                        target="_blank"
                                        rel="noreferrer"
                                        className="text-slate-400 hover:text-emerald-400 transition-colors"
                                      >
                                        <ExternalLink className="w-2.5 h-2.5" />
                                      </a>
                                    )}
                                    <span className="text-[10px] text-slate-500">
                                      • {reply.created_at ? new Date(reply.created_at * 1000).toLocaleString('vi-VN') : ''}
                                    </span>
                                  </div>
                                </div>

                                <div className="flex items-center gap-1.5">
                                  {renderIntentBadge(reply.intent_tag)}
                                  <button
                                    onClick={() => copyToClipboard(reply.content || '', 'comment', reply.id)}
                                    className="p-1 rounded text-slate-500 hover:text-slate-300 transition-colors"
                                    title="Sao chép câu trả lời"
                                  >
                                    {copiedCommentId === reply.id ? (
                                      <Check className="w-3 h-3 text-emerald-400" />
                                    ) : (
                                      <Copy className="w-3 h-3" />
                                    )}
                                  </button>
                                </div>
                              </div>

                              {/* Reply Content Box */}
                              <div className="p-2.5 rounded-lg bg-slate-900/90 border border-slate-800 text-slate-200 text-xs leading-relaxed whitespace-pre-wrap select-text break-words">
                                {reply.content || <span className="text-slate-500 italic">Không có văn bản</span>}
                              </div>
                            </div>

                            {/* Reply Phone Detection */}
                            {reply.phone_numbers && (
                              <div className="shrink-0 flex items-center gap-2 bg-emerald-500/15 border border-emerald-500/40 rounded-lg px-2.5 py-1.5">
                                <PhoneCall className="w-3.5 h-3.5 text-emerald-400" />
                                <span className="text-xs font-mono font-bold text-emerald-200">
                                  {reply.phone_numbers}
                                </span>
                                <button
                                  onClick={() => copyToClipboard(reply.phone_numbers!, 'phone')}
                                  className="p-1 rounded bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300"
                                >
                                  {copiedPhone === reply.phone_numbers ? (
                                    <Check className="w-3 h-3" />
                                  ) : (
                                    <Copy className="w-3 h-3" />
                                  )}
                                </button>
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-3 border-t border-slate-800 bg-slate-900/80 flex items-center justify-between text-xs text-slate-400 shrink-0">
          <div>
            Hiển thị <strong>{filteredThreads.length}</strong> luồng bình luận (Tổng số: <strong>{comments.length}</strong>)
          </div>
          <div className="flex items-center gap-4">
            <span>
              Chốt đơn: <strong className="text-amber-400">{totalOrders}</strong>
            </span>
            <span>
              SĐT: <strong className="text-emerald-400">{totalPhones}</strong>
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};
