import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  X,
  ShieldCheck,
  Save,
  CheckCircle2,
  Sparkles,
  ExternalLink,
  Download,
  AlertCircle,
  Globe,
  Edit3
} from 'lucide-react';

interface FbSessionModalProps {
  onClose: () => void;
  onSuccess: () => void;
}

export const FbSessionModal: React.FC<FbSessionModalProps> = ({
  onClose,
  onSuccess
}) => {
  const [activeTab, setActiveTab] = useState<'webview' | 'manual'>('webview');
  const [cUser, setCUser] = useState('');
  const [xs, setXs] = useState('');
  const [datr, setDatr] = useState('');
  const [fr, setFr] = useState('');
  const [fbDtsg, setFbDtsg] = useState('');
  const [loading, setLoading] = useState(false);
  const [extracting, setExtracting] = useState(false);
  const [savedSuccess, setSavedSuccess] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // 1. In-App WebView: Open Facebook login window
  const handleOpenLoginWindow = async () => {
    try {
      setErrorMsg(null);
      setStatusMsg('Đã mở cửa sổ đăng nhập Facebook. Vui lòng đăng nhập và vượt 2FA trên cửa sổ đó.');
      await invoke('open_facebook_login_window');
    } catch (err: any) {
      setErrorMsg(`Không thể mở WebView Facebook: ${err}`);
    }
  };

  // 2. In-App WebView: Extract session cookies from login window
  const handleExtractSession = async () => {
    try {
      setExtracting(true);
      setErrorMsg(null);
      const res = await invoke<string>('extract_facebook_session_from_window');
      setStatusMsg(res);
      setSavedSuccess(true);
      setTimeout(() => {
        onSuccess();
        onClose();
      }, 1200);
    } catch (err: any) {
      setErrorMsg(String(err));
    } finally {
      setExtracting(false);
    }
  };

  // 3. Manual Input Save
  const handleSaveManual = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      setLoading(true);
      setErrorMsg(null);
      await invoke('set_facebook_session', {
        cUser: cUser.trim(),
        xs: xs.trim(),
        datr: datr.trim() || undefined,
        fr: fr.trim() || undefined,
        fbDtsg: fbDtsg.trim()
      });
      setSavedSuccess(true);
      setTimeout(() => {
        onSuccess();
        onClose();
      }, 800);
    } catch (err: any) {
      setErrorMsg(`Lỗi cấu hình Session: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleFillDemo = () => {
    setCUser('100088991234567');
    setXs('28%3AX-demo-session-token%3A2%3A1711122334%3A-1%3A-1');
    setDatr('X9Y8Z7demoDatrToken1234');
    setFr('0demoFrTokenSecure9988');
    setFbDtsg('NAcPaDemoFbDtsgToken7788:1711122334');
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md animate-fadeIn">
      <div className="w-full max-w-lg bg-[#111827] border border-slate-700/80 rounded-3xl shadow-2xl p-6 relative overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between pb-4 border-b border-slate-800">
          <div className="flex items-center gap-2.5">
            <div className="p-2.5 rounded-2xl bg-gradient-to-tr from-emerald-500/20 to-teal-500/20 border border-emerald-500/30 text-emerald-400">
              <ShieldCheck className="w-5 h-5" />
            </div>
            <div>
              <h3 className="text-base font-bold text-white">
                Đăng Nhập Tài Khoản Facebook
              </h3>
              <p className="text-xs text-slate-400">
                In-App WebView hỗ trợ 2FA & Trích xuất session GraphQL tự động.
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-white transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Tab Selection */}
        <div className="flex items-center gap-2 mt-4 p-1 rounded-xl bg-slate-900 border border-slate-800">
          <button
            type="button"
            onClick={() => setActiveTab('webview')}
            className={`flex-1 py-2 rounded-lg text-xs font-semibold transition-all flex items-center justify-center gap-1.5 ${
              activeTab === 'webview'
                ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 shadow-sm'
                : 'text-slate-400 hover:text-slate-200'
            }`}
          >
            <Globe className="w-3.5 h-3.5" />
            In-App WebView (Khuyên Dùng)
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('manual')}
            className={`flex-1 py-2 rounded-lg text-xs font-semibold transition-all flex items-center justify-center gap-1.5 ${
              activeTab === 'manual'
                ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 shadow-sm'
                : 'text-slate-400 hover:text-slate-200'
            }`}
          >
            <Edit3 className="w-3.5 h-3.5" />
            Nhập Cookies Thủ Công
          </button>
        </div>

        {/* Status & Error Alerts */}
        {errorMsg && (
          <div className="mt-4 p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>{errorMsg}</span>
          </div>
        )}

        {statusMsg && !errorMsg && (
          <div className="mt-4 p-3 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-300 text-xs flex items-center gap-2">
            <CheckCircle2 className="w-4 h-4 shrink-0" />
            <span>{statusMsg}</span>
          </div>
        )}

        {/* TAB 1: In-App WebView Login Flow */}
        {activeTab === 'webview' && (
          <div className="mt-5 space-y-4">
            <div className="p-4 rounded-2xl bg-slate-900/80 border border-slate-800 space-y-3">
              <div className="flex items-start gap-3">
                <span className="w-6 h-6 rounded-full bg-emerald-500/20 text-emerald-400 font-bold text-xs flex items-center justify-center shrink-0 mt-0.5">
                  1
                </span>
                <div className="space-y-1">
                  <h4 className="text-xs font-semibold text-white">
                    Mở Cửa Sổ WebView Facebook
                  </h4>
                  <p className="text-[11px] text-slate-400 leading-relaxed">
                    Đăng nhập tài khoản Facebook của bạn và thực hiện xác minh 2 lớp (2FA) an toàn trên cửa sổ chính thức của Facebook.
                  </p>
                  <button
                    type="button"
                    onClick={handleOpenLoginWindow}
                    className="mt-2 px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 text-xs font-semibold transition-all flex items-center gap-2"
                  >
                    <ExternalLink className="w-3.5 h-3.5 text-emerald-400" />
                    Mở Cửa Sổ Đăng Nhập Facebook
                  </button>
                </div>
              </div>

              <div className="border-t border-slate-800 pt-3 flex items-start gap-3">
                <span className="w-6 h-6 rounded-full bg-emerald-500/20 text-emerald-400 font-bold text-xs flex items-center justify-center shrink-0 mt-0.5">
                  2
                </span>
                <div className="space-y-1 w-full">
                  <h4 className="text-xs font-semibold text-white">
                    Trích Xuất Session & Đồng Bộ Về App
                  </h4>
                  <p className="text-[11px] text-slate-400 leading-relaxed">
                    Sau khi đã đăng nhập thành công vào trang chủ Facebook trên cửa sổ vừa mở, bấm nút bên dưới để Rust tự động bóc tách <code>c_user</code>, <code>xs</code>, <code>fb_dtsg</code>.
                  </p>
                  <button
                    type="button"
                    onClick={handleExtractSession}
                    disabled={extracting}
                    className="mt-2 w-full py-2.5 px-4 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white font-bold text-xs shadow-lg shadow-emerald-600/20 transition-all flex items-center justify-center gap-2 disabled:opacity-50"
                  >
                    <Download className={`w-3.5 h-3.5 ${extracting ? 'animate-bounce' : ''}`} />
                    {extracting ? 'Đang trích xuất Session...' : 'Trích Xuất Session Tự Động & Đóng Cửa Sổ'}
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* TAB 2: Manual Cookie Inputs */}
        {activeTab === 'manual' && (
          <form onSubmit={handleSaveManual} className="space-y-3 mt-5">
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <label className="text-xs font-semibold text-slate-300">c_user (UID)</label>
                <input
                  type="text"
                  required
                  placeholder="1000..."
                  value={cUser}
                  onChange={(e) => setCUser(e.target.value)}
                  className="w-full px-3 py-2 rounded-xl bg-slate-900 border border-slate-700 text-xs text-white font-mono placeholder-slate-500 focus:outline-none focus:border-emerald-500"
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-semibold text-slate-300">xs (Token)</label>
                <input
                  type="text"
                  required
                  placeholder="28%3A..."
                  value={xs}
                  onChange={(e) => setXs(e.target.value)}
                  className="w-full px-3 py-2 rounded-xl bg-slate-900 border border-slate-700 text-xs text-white font-mono placeholder-slate-500 focus:outline-none focus:border-emerald-500"
                />
              </div>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-semibold text-slate-300">fb_dtsg (CSRF Token)</label>
              <input
                type="text"
                required
                placeholder="NAc..."
                value={fbDtsg}
                onChange={(e) => setFbDtsg(e.target.value)}
                className="w-full px-3 py-2 rounded-xl bg-slate-900 border border-slate-700 text-xs text-white font-mono placeholder-slate-500 focus:outline-none focus:border-emerald-500"
              />
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <label className="text-xs font-semibold text-slate-400">datr (Tùy chọn)</label>
                <input
                  type="text"
                  placeholder="datr cookie..."
                  value={datr}
                  onChange={(e) => setDatr(e.target.value)}
                  className="w-full px-3 py-2 rounded-xl bg-slate-900/60 border border-slate-800 text-xs text-slate-300 font-mono placeholder-slate-600 focus:outline-none focus:border-emerald-500"
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-semibold text-slate-400">fr (Tùy chọn)</label>
                <input
                  type="text"
                  placeholder="fr cookie..."
                  value={fr}
                  onChange={(e) => setFr(e.target.value)}
                  className="w-full px-3 py-2 rounded-xl bg-slate-900/60 border border-slate-800 text-xs text-slate-300 font-mono placeholder-slate-600 focus:outline-none focus:border-emerald-500"
                />
              </div>
            </div>

            <div className="pt-2 flex items-center justify-between gap-3">
              <button
                type="button"
                onClick={handleFillDemo}
                className="px-3 py-2 rounded-xl bg-slate-800/80 hover:bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-700/60 text-xs transition-colors flex items-center gap-1.5"
              >
                <Sparkles className="w-3.5 h-3.5 text-emerald-400" />
                Điền Session Demo
              </button>

              <button
                type="submit"
                disabled={loading}
                className="px-5 py-2.5 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white font-semibold text-xs shadow-lg shadow-emerald-600/20 transition-all flex items-center gap-2"
              >
                {savedSuccess ? (
                  <>
                    <CheckCircle2 className="w-4 h-4" />
                    Đã Lưu
                  </>
                ) : (
                  <>
                    <Save className="w-4 h-4" />
                    Lưu Thủ Công
                  </>
                )}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
};
