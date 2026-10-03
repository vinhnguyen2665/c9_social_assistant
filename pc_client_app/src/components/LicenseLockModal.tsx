import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Lock,
  Key,
  ShieldAlert,
  Server,
  Cpu,
  Copy,
  Check,
  ArrowRight,
  AlertCircle
} from 'lucide-react';
import { LicenseStateInfo } from '../types';

interface LicenseLockModalProps {
  licenseInfo: LicenseStateInfo | null;
  onSuccess: () => void;
}

export const LicenseLockModal: React.FC<LicenseLockModalProps> = ({
  licenseInfo,
  onSuccess
}) => {
  const [licenseKey, setLicenseKey] = useState('C9-PRO-2026-VIP');
  const [serverUrl, setServerUrl] = useState('http://127.0.0.1:8000');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copiedHwid, setCopiedHwid] = useState(false);

  const handleCopyHwid = () => {
    if (licenseInfo?.hwid) {
      navigator.clipboard.writeText(licenseInfo.hwid);
      setCopiedHwid(true);
      setTimeout(() => setCopiedHwid(false), 2000);
    }
  };

  const handleActivate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!licenseKey.trim()) return;

    try {
      setLoading(true);
      setError(null);
      await invoke('login_license', {
        licenseKey: licenseKey.trim(),
        serverUrl: serverUrl.trim() || undefined
      });
      onSuccess();
    } catch (err: any) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-[#080B12]/95 backdrop-blur-xl animate-fadeIn">
      <div className="w-full max-w-lg bg-[#111827] border border-slate-700/80 rounded-3xl shadow-2xl p-8 relative overflow-hidden">
        {/* Glow decoration */}
        <div className="absolute -top-24 -left-24 w-48 h-48 bg-emerald-500/20 rounded-full blur-3xl pointer-events-none" />
        <div className="absolute -bottom-24 -right-24 w-48 h-48 bg-teal-500/20 rounded-full blur-3xl pointer-events-none" />

        {/* Lock Icon & Title */}
        <div className="text-center space-y-2 mb-6">
          <div className="inline-flex p-4 rounded-2xl bg-gradient-to-tr from-emerald-500/20 to-teal-500/10 border border-emerald-500/30 text-emerald-400 shadow-inner">
            <Lock className="w-8 h-8" />
          </div>
          <h2 className="text-2xl font-extrabold text-white tracking-tight">
            Kích hoạt Bản quyền Trực tuyến
          </h2>
          <p className="text-xs text-slate-400 max-w-sm mx-auto leading-relaxed">
            Hệ thống hoạt động ở chế độ <strong>Strict Online-Only</strong> được bảo vệ bởi chữ ký mã hóa bất đối xứng Ed25519.
          </p>
        </div>

        {/* HWID Card */}
        <div className="mb-6 p-3.5 rounded-2xl bg-slate-900/80 border border-slate-800 space-y-1.5">
          <div className="flex items-center justify-between text-xs text-slate-400">
            <span className="flex items-center gap-1.5 font-medium">
              <Cpu className="w-3.5 h-3.5 text-emerald-400" />
              Mã phần cứng thiết bị (HWID):
            </span>
            <button
              type="button"
              onClick={handleCopyHwid}
              className="text-emerald-400 hover:text-emerald-300 font-medium flex items-center gap-1 transition-colors"
            >
              {copiedHwid ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
              {copiedHwid ? 'Đã sao chép' : 'Sao chép'}
            </button>
          </div>
          <div className="font-mono text-[11px] text-slate-300 bg-slate-950/70 p-2 rounded-lg break-all select-all border border-slate-800">
            {licenseInfo?.hwid || 'Đang nhận diện phần cứng...'}
          </div>
        </div>

        {/* Activation Form */}
        <form onSubmit={handleActivate} className="space-y-4">
          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-slate-300 flex items-center gap-1.5">
              <Key className="w-3.5 h-3.5 text-emerald-400" />
              License Key
            </label>
            <input
              type="text"
              required
              placeholder="VD: C9-PRO-2026-VIP"
              value={licenseKey}
              onChange={(e) => setLicenseKey(e.target.value)}
              className="w-full px-4 py-2.5 rounded-xl bg-slate-900/90 border border-slate-700 text-white font-mono text-sm placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 focus:border-emerald-500 transition-all"
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-slate-300 flex items-center gap-1.5">
              <Server className="w-3.5 h-3.5 text-teal-400" />
              Máy chủ xác thực (License Server)
            </label>
            <input
              type="text"
              required
              placeholder="http://127.0.0.1:8000"
              value={serverUrl}
              onChange={(e) => setServerUrl(e.target.value)}
              className="w-full px-4 py-2.5 rounded-xl bg-slate-900/90 border border-slate-700 text-white text-xs font-mono placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 focus:border-emerald-500 transition-all"
            />
          </div>

          {error && (
            <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
              <AlertCircle className="w-4 h-4 shrink-0" />
              <span>{error}</span>
            </div>
          )}

          <button
            type="submit"
            disabled={loading}
            className="w-full py-3 px-4 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white font-bold text-sm shadow-xl shadow-emerald-600/20 transition-all flex items-center justify-center gap-2 disabled:opacity-50"
          >
            {loading ? (
              'Đang xác minh chữ ký Ed25519...'
            ) : (
              <>
                Kích hoạt Bản quyền
                <ArrowRight className="w-4 h-4" />
              </>
            )}
          </button>
        </form>

        <div className="mt-6 text-center text-[11px] text-slate-500 flex items-center justify-center gap-2">
          <ShieldAlert className="w-3.5 h-3.5 text-slate-400" />
          Heartbeat tự động mỗi 5 phút. Mất mạng quá 10 phút sẽ tự khóa.
        </div>
      </div>
    </div>
  );
};
