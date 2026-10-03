import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  ShieldCheck,
  ShieldAlert,
  CloudUpload,
  Cpu,
  Check,
  Copy,
  Radio,
  Layers
} from 'lucide-react';
import { PostTrackerDashboard } from './components/PostTrackerDashboard';
import { CommentsModal } from './components/CommentsModal';
import { LicenseLockModal } from './components/LicenseLockModal';
import { FbSessionModal } from './components/FbSessionModal';
import { LicenseStateInfo } from './types';

export const App: React.FC = () => {
  const [licenseInfo, setLicenseInfo] = useState<LicenseStateInfo | null>(null);
  const [hasFbSession, setHasFbSession] = useState(false);
  const [activeCommentsPostId, setActiveCommentsPostId] = useState<string | null>(null);
  const [activeCommentsPostTitle, setActiveCommentsPostTitle] = useState<string | undefined>();
  const [activeCommentsPostPreview, setActiveCommentsPostPreview] = useState<string | undefined>();
  const [activeCommentsPostUrl, setActiveCommentsPostUrl] = useState<string | undefined>();
  const [showSessionModal, setShowSessionModal] = useState(false);
  const [syncingCloud, setSyncingCloud] = useState(false);
  const [copiedHwid, setCopiedHwid] = useState(false);

  // Load initial states
  const checkStatus = async () => {
    try {
      const lic = await invoke<LicenseStateInfo>('get_license_status');
      setLicenseInfo(lic);

      const fb = await invoke<boolean>('get_facebook_session_status');
      setHasFbSession(fb);
    } catch (e) {
      console.warn('Status check warning:', e);
    }
  };

  useEffect(() => {
    checkStatus();

    // Listen to license locked events from Tauri
    let unlistenLocked: (() => void) | undefined;
    let unlistenWarning: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        const u1 = await listen('license_locked', () => {
          setLicenseInfo((prev) => (prev ? { ...prev, is_authenticated: false } : null));
          alert('BẢN QUYỀN ĐÃ BỊ KHÓA: Mất kết nối xác thực trực tuyến quá thời hạn quy định.');
        });
        unlistenLocked = u1;

        const u2 = await listen<{ failures: number; error: string }>('license_heartbeat_warning', (ev) => {
          console.warn('Heartbeat warning:', ev.payload);
        });
        unlistenWarning = u2;
      } catch (e) {
        console.warn('Tauri event listen warning:', e);
      }
    };

    setupListeners();

    return () => {
      if (unlistenLocked) unlistenLocked();
      if (unlistenWarning) unlistenWarning();
    };
  }, []);

  const handleCopyHwid = () => {
    if (licenseInfo?.hwid) {
      navigator.clipboard.writeText(licenseInfo.hwid);
      setCopiedHwid(true);
      setTimeout(() => setCopiedHwid(false), 2000);
    }
  };

  const handleSyncCloud = async () => {
    try {
      setSyncingCloud(true);
      const msg = await invoke<string>('sync_all_to_cloud');
      alert(`ĐỒNG BỘ CLOUD THÀNH CÔNG:\n${msg}`);
    } catch (err: any) {
      alert(`Lỗi đồng bộ lên máy chủ: ${err}`);
    } finally {
      setSyncingCloud(false);
    }
  };

  return (
    <div className="min-h-screen bg-[#0B0F19] text-slate-100 flex flex-col">
      {/* Top Navbar */}
      <header className="sticky top-0 z-40 bg-[#0F172A]/80 backdrop-blur-xl border-b border-slate-800/80 px-6 py-3.5 flex items-center justify-between">
        {/* Brand */}
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-emerald-500 to-teal-400 p-0.5 shadow-lg shadow-emerald-500/20 flex items-center justify-center text-white">
            <Layers className="w-5 h-5" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="font-extrabold text-base tracking-tight text-white">
                C9 Social Assistant
              </span>
              <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                PRO v1.0
              </span>
            </div>
            <p className="text-[11px] text-slate-400 font-medium">
              Facebook Data Mining & Order Extraction Platform
            </p>
          </div>
        </div>

        {/* Status Indicators & Actions */}
        <div className="flex items-center gap-3">
          {/* HWID Badge */}
          <button
            onClick={handleCopyHwid}
            className="hidden lg:flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-slate-900/90 border border-slate-800 text-xs text-slate-400 hover:text-slate-200 transition-colors"
            title="Nhấn để sao chép HWID máy"
          >
            <Cpu className="w-3.5 h-3.5 text-emerald-400" />
            <span className="font-mono text-[11px] truncate max-w-[120px]">
              {licenseInfo?.hwid ? `${licenseInfo.hwid.slice(0, 10)}...` : 'HWID'}
            </span>
            {copiedHwid ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
          </button>

          {/* FB Session Indicator */}
          <button
            onClick={() => setShowSessionModal(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-slate-900/90 border border-slate-800 hover:border-slate-700 text-xs transition-colors"
          >
            <Radio className={`w-3.5 h-3.5 ${hasFbSession ? 'text-emerald-400' : 'text-amber-400'}`} />
            <span className="font-medium text-slate-300">
              {hasFbSession ? 'FB Session: Đã kết nối' : 'FB Session: Chưa có'}
            </span>
          </button>

          {/* Cloud Sync Button */}
          <button
            onClick={handleSyncCloud}
            disabled={syncingCloud || !licenseInfo?.is_authenticated}
            className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-teal-500/10 hover:bg-teal-500/20 text-teal-300 border border-teal-500/30 text-xs font-semibold shadow-sm transition-all disabled:opacity-50"
            title="Đồng bộ danh sách bài viết và bình luận lên PostgreSQL Cloud"
          >
            <CloudUpload className={`w-3.5 h-3.5 ${syncingCloud ? 'animate-bounce' : ''}`} />
            <span>{syncingCloud ? 'Đang sync...' : 'Đồng bộ Cloud'}</span>
          </button>

          {/* License Badge */}
          <div className="flex items-center gap-2 pl-2 border-l border-slate-800">
            {licenseInfo?.is_authenticated ? (
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 text-xs font-semibold">
                <ShieldCheck className="w-4 h-4" />
                <span className="truncate max-w-[150px]">
                  {licenseInfo.client_name || 'Bản quyền Online'}
                </span>
              </div>
            ) : (
              <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-400 text-xs font-semibold animate-pulse">
                <ShieldAlert className="w-4 h-4" />
                <span>Chưa kích hoạt</span>
              </div>
            )}
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1 max-w-7xl w-full mx-auto p-6 md:p-8">
        <PostTrackerDashboard
          onOpenComments={(id, title, preview, url) => {
            setActiveCommentsPostId(id);
            setActiveCommentsPostTitle(title);
            setActiveCommentsPostPreview(preview);
            setActiveCommentsPostUrl(url);
          }}
          onOpenSessionModal={() => setShowSessionModal(true)}
        />
      </main>

      {/* Modals */}
      {/* 1. Strict Online-Only Lock Modal */}
      {(!licenseInfo || !licenseInfo.is_authenticated) && (
        <LicenseLockModal
          licenseInfo={licenseInfo}
          onSuccess={checkStatus}
        />
      )}

      {/* 2. Comments Modal */}
      {activeCommentsPostId && (
        <CommentsModal
          postId={activeCommentsPostId}
          postTitle={activeCommentsPostTitle}
          postPreview={activeCommentsPostPreview}
          postUrl={activeCommentsPostUrl}
          onClose={() => setActiveCommentsPostId(null)}
          onPostUpdated={checkStatus}
        />
      )}

      {/* 3. Facebook Session Modal */}
      {showSessionModal && (
        <FbSessionModal
          onClose={() => setShowSessionModal(false)}
          onSuccess={checkStatus}
        />
      )}
    </div>
  );
};
export default App;
