import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Camera, VideoOff, Power, Loader2 } from 'lucide-react';
import { API_BASE, resolveHostUrl } from '../config';

interface CameraStatus {
    has_camera: boolean;
    topic: string | null;
    server_running: boolean;
    stream_url: string | null;
}

/**
 * Live camera viewer.
 *
 * Fetches camera status, manages web_video_server toggle, and embeds
 * the MJPEG stream. Includes robust retry logic for server startup delay.
 */
interface CameraPanelProps {
    label?: string;
}

export const CameraPanel: React.FC<CameraPanelProps> = ({ label = 'Camera Feed' }) => {
    const [status, setStatus] = useState<CameraStatus | null>(null);
    const [loading, setLoading] = useState(false);
    const [streamReady, setStreamReady] = useState(false);
    const [streamFailed, setStreamFailed] = useState(false);
    // activeSrc is the actual URL loaded in <img>. Managed separately from
    // status.stream_url so that status polling doesn't reset it mid-retry.
    const [activeSrc, setActiveSrc] = useState<string | null>(null);
    const retryCountRef = useRef(0);
    const retryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    // ─── Fetch status ───────────────────────────────────────────────────
    const fetchStatus = useCallback(async () => {
        try {
            const res = await fetch(`${API_BASE}/api/camera/status`);
            const data: CameraStatus = await res.json();
            setStatus(data);
        } catch (e) {
            console.error('Camera status fetch failed:', e);
        }
    }, []);

    useEffect(() => {
        fetchStatus();
        const interval = setInterval(fetchStatus, 3000);
        return () => clearInterval(interval);
    }, [fetchStatus]);

    // ─── Toggle web_video_server ────────────────────────────────────────
    const toggleServer = useCallback(async () => {
        if (!status) return;
        setLoading(true);
        setStreamReady(false);
        setStreamFailed(false);
        setActiveSrc(null);
        retryCountRef.current = 0;
        if (retryTimerRef.current) clearTimeout(retryTimerRef.current);
        try {
            const endpoint = status.server_running ? 'stop' : 'start';
            await fetch(`${API_BASE}/api/camera/${endpoint}`, { method: 'POST' });
            // Wait for the process to actually start before polling status
            setTimeout(fetchStatus, 1000);
        } catch (e) {
            console.error('Camera toggle failed:', e);
        } finally {
            setLoading(false);
        }
    }, [status, fetchStatus]);

    // ─── When server becomes running, kick off the stream connection ────
    useEffect(() => {
        if (!status?.server_running || !status.stream_url) {
            setStreamReady(false);
            setStreamFailed(false);
            setActiveSrc(null);
            retryCountRef.current = 0;
            if (retryTimerRef.current) {
                clearTimeout(retryTimerRef.current);
                retryTimerRef.current = null;
            }
            return;
        }

        // If we don't already have an active src, set one after a brief
        // delay to give web_video_server time to start listening.
        if (!activeSrc) {
            const url = resolveHostUrl(status.stream_url);
            retryTimerRef.current = setTimeout(() => {
                setActiveSrc(`${url}${url.includes('?') ? '&' : '?'}_t=${Date.now()}`);
            }, 1500);
        }
    }, [status?.server_running, status?.stream_url, activeSrc]);

    // ─── Stream load handlers ───────────────────────────────────────────
    const handleStreamLoad = useCallback(() => {
        setStreamReady(true);
        setStreamFailed(false);
        retryCountRef.current = 0;
    }, []);

    const handleStreamError = useCallback(() => {
        if (retryCountRef.current >= 10) {
            setStreamFailed(true);
            return;
        }
        retryCountRef.current++;
        const baseUrl = status?.stream_url ? resolveHostUrl(status.stream_url) : undefined;
        if (!baseUrl) return;
        retryTimerRef.current = setTimeout(() => {
            setActiveSrc(`${baseUrl}${baseUrl.includes('?') ? '&' : '?'}_t=${Date.now()}`);
        }, 1500);
    }, [status?.stream_url]);

    // Cleanup
    useEffect(() => {
        return () => {
            if (retryTimerRef.current) clearTimeout(retryTimerRef.current);
        };
    }, []);

    return (
        <div className="flex flex-col h-full bg-background">
            {/* Header */}
            <div className="h-9 shrink-0 border-b border-white/5 bg-surface/50 flex items-center justify-between px-4 backdrop-blur-md">
                <div className="flex items-center gap-2">
                    <Camera className="w-3.5 h-3.5 text-primary_glow" />
                    <span className="text-[10px] font-mono text-primary_glow tracking-wider uppercase">{label}</span>
                </div>

                {status?.has_camera && (
                    <button
                        onClick={toggleServer}
                        disabled={loading}
                        className={`flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-mono transition-all ${status.server_running
                                ? 'bg-accent/20 text-accent_glow border border-accent/50 hover:bg-accent/30'
                                : 'bg-surface border border-white/10 text-slate-400 hover:text-white hover:border-white/20'
                            }`}
                    >
                        {loading ? (
                            <Loader2 size={10} className="animate-spin" />
                        ) : (
                            <Power size={10} />
                        )}
                        {status.server_running ? 'ON' : 'OFF'}
                    </button>
                )}
            </div>

            {/* Content */}
            <div className="flex-1 flex flex-col items-center justify-center overflow-hidden">
                {!status ? (
                    <div className="flex flex-col items-center gap-2">
                        <Loader2 className="w-6 h-6 text-slate-600 animate-spin" />
                        <p className="text-[10px] text-slate-600 font-mono">Checking camera...</p>
                    </div>
                ) : !status.has_camera ? (
                    <div className="flex flex-col items-center gap-3 p-6 text-center">
                        <div className="p-4 rounded-xl bg-surface/40 border border-white/5">
                            <VideoOff className="w-8 h-8 text-slate-600" />
                        </div>
                        <p className="text-xs font-mono text-slate-500">
                            This robot does not have a camera configured
                        </p>
                        <p className="text-[10px] text-slate-600">
                            Switch to a camera-equipped profile to enable this feature
                        </p>
                    </div>
                ) : !status.server_running ? (
                    <div className="flex flex-col items-center gap-3 p-6 text-center">
                        <div className="p-4 rounded-xl bg-surface/40 border border-white/5">
                            <Camera className="w-8 h-8 text-slate-500" />
                        </div>
                        <p className="text-xs font-mono text-slate-400">
                            Camera available on <span className="text-primary_glow">{status.topic}</span>
                        </p>
                        <p className="text-[10px] text-slate-600">
                            Click the power button above to start streaming
                        </p>
                    </div>
                ) : (
                    <div className="w-full h-full relative">
                        {/* Loading overlay */}
                        {!streamReady && !streamFailed && (
                            <div className="absolute inset-0 flex flex-col items-center justify-center bg-background z-10">
                                <Loader2 className="w-6 h-6 text-primary_glow animate-spin" />
                                <p className="text-[10px] text-slate-500 font-mono mt-2">
                                    Connecting to camera stream...
                                </p>
                            </div>
                        )}
                        {/* Failure */}
                        {streamFailed && (
                            <div className="absolute inset-0 flex flex-col items-center justify-center bg-background z-10">
                                <VideoOff className="w-8 h-8 text-slate-600" />
                                <p className="text-xs font-mono text-slate-500 mt-2">
                                    Could not connect to camera stream
                                </p>
                                <p className="text-[10px] text-slate-600 mt-1">
                                    Try toggling the power button
                                </p>
                            </div>
                        )}
                        {/* MJPEG stream — activeSrc is managed via state, never reset by polls */}
                        {activeSrc && (
                            <img
                                key={activeSrc}
                                src={activeSrc}
                                alt=""
                                onLoad={handleStreamLoad}
                                onError={handleStreamError}
                                className={`w-full h-full object-contain bg-black ${streamReady ? 'opacity-100' : 'opacity-0'
                                    }`}
                            />
                        )}
                    </div>
                )}
            </div>
        </div>
    );
};
