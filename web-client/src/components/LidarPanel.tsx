import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Radar, RadarIcon, Power, Loader2 } from 'lucide-react';
import * as ROSLIB from 'roslib';
import { API_BASE, resolveHostUrl } from '../config';

// ─── Types ──────────────────────────────────────────────────────────────────

interface LidarStatus {
    has_lidar: boolean;
    topic: string | null;
    server_running: boolean;
    rosbridge_url: string | null;
}

interface LaserScanMsg {
    angle_min: number;
    angle_max: number;
    angle_increment: number;
    range_min: number;
    range_max: number;
    ranges: number[];
    intensities: number[];
}

// ─── Radar Renderer ─────────────────────────────────────────────────────────

/**
 * Draws the radar visualization on a Canvas2D context.
 *
 * Orientation: ROS X-forward (angle 0) is rendered pointing UP.
 * This is achieved by rotating all angles by +90° (π/2).
 */
function drawRadar(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    scan: LaserScanMsg | null,
    dpr: number,
) {
    const cx = width / 2;
    const cy = height / 2;
    const maxRadius = Math.min(cx, cy) - 16;

    ctx.clearRect(0, 0, width, height);

    // ─── Background ─────────────────────────────────────────────────────
    ctx.fillStyle = 'rgb(4, 18, 18)';
    ctx.fillRect(0, 0, width, height);

    // ─── Concentric range rings ─────────────────────────────────────────
    const ringCount = 5;
    const maxRange = scan ? scan.range_max : 3.5;
    ctx.lineWidth = 1 * dpr;
    ctx.font = `${9 * dpr}px "Fira Code", monospace`;
    ctx.textAlign = 'center';

    for (let i = 1; i <= ringCount; i++) {
        const r = (i / ringCount) * maxRadius;
        const rangeVal = ((i / ringCount) * maxRange).toFixed(1);

        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.strokeStyle = i === ringCount
            ? 'rgba(0, 255, 200, 0.15)'
            : 'rgba(0, 255, 200, 0.06)';
        ctx.stroke();

        // Range label at the top of each ring
        ctx.fillStyle = 'rgba(0, 255, 200, 0.3)';
        ctx.fillText(`${rangeVal}m`, cx, cy - r + 12 * dpr);
    }

    // ─── Angular grid lines (every 45°) ─────────────────────────────────
    ctx.strokeStyle = 'rgba(0, 255, 200, 0.04)';
    for (let deg = 0; deg < 360; deg += 45) {
        const rad = (deg * Math.PI) / 180;
        ctx.beginPath();
        ctx.moveTo(cx, cy);
        ctx.lineTo(
            cx + Math.cos(rad) * maxRadius,
            cy - Math.sin(rad) * maxRadius,
        );
        ctx.stroke();
    }

    // ─── Center (robot) dot ─────────────────────────────────────────────
    ctx.beginPath();
    ctx.arc(cx, cy, 3.5 * dpr, 0, Math.PI * 2);
    ctx.fillStyle = 'rgba(0, 255, 200, 0.8)';
    ctx.fill();

    // Directional arrow (forward = UP)
    ctx.beginPath();
    ctx.moveTo(cx, cy - 10 * dpr);
    ctx.lineTo(cx - 4 * dpr, cy - 3 * dpr);
    ctx.lineTo(cx + 4 * dpr, cy - 3 * dpr);
    ctx.closePath();
    ctx.fillStyle = 'rgba(0, 255, 200, 0.5)';
    ctx.fill();

    if (!scan || scan.ranges.length === 0) return;

    // ─── Compute scan points ────────────────────────────────────────────
    const { angle_min, angle_increment, range_min, range_max, ranges } = scan;
    const scale = maxRadius / maxRange;

    const points: { x: number; y: number; dist: number }[] = [];
    for (let i = 0; i < ranges.length; i++) {
        const r = ranges[i];
        if (r < range_min || r > range_max || !isFinite(r)) continue;

        const angle = angle_min + i * angle_increment;
        // Rotate +90° so ROS forward (angle 0) points UP on canvas
        // cos(a + π/2) = -sin(a),  sin(a + π/2) = cos(a)
        const px = cx + (-Math.sin(angle)) * r * scale;
        const py = cy - Math.cos(angle) * r * scale;
        points.push({ x: px, y: py, dist: r });
    }

    if (points.length < 2) return;

    // ─── Filled scan polygon ────────────────────────────────────────────
    ctx.beginPath();
    ctx.moveTo(cx, cy);
    for (const pt of points) ctx.lineTo(pt.x, pt.y);
    ctx.closePath();
    ctx.fillStyle = 'rgba(0, 255, 180, 0.04)';
    ctx.fill();

    // ─── Scan outline ───────────────────────────────────────────────────
    ctx.beginPath();
    ctx.moveTo(points[0].x, points[0].y);
    for (let i = 1; i < points.length; i++) ctx.lineTo(points[i].x, points[i].y);
    ctx.strokeStyle = 'rgba(0, 255, 200, 0.35)';
    ctx.lineWidth = 1.5 * dpr;
    ctx.stroke();

    // ─── Scan points (distance-colored) ─────────────────────────────────
    for (const pt of points) {
        const t = Math.min(pt.dist / maxRange, 1);
        const g = 255 - Math.floor(t * 55);
        const b = 180 + Math.floor(t * 75);
        const a = 1 - t * 0.4;
        const radius = (2.5 - t) * dpr;

        ctx.beginPath();
        ctx.arc(pt.x, pt.y, radius, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(0,${g},${b},${a})`;
        ctx.fill();
    }
}

// ─── Component ──────────────────────────────────────────────────────────────

/**
 * Real-time LiDAR radar viewer.
 *
 * Connects to rosbridge_server via roslibjs, subscribes to LaserScan,
 * and renders a radar-style visualization.
 *
 * Performance: renders ONLY when a new scan message arrives (data-driven),
 * not on every animation frame. This prevents RAM/CPU overload.
 */
export const LidarPanel: React.FC = () => {
    const [status, setStatus] = useState<LidarStatus | null>(null);
    const [loading, setLoading] = useState(false);
    const [connected, setConnected] = useState(false);
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const rosRef = useRef<ROSLIB.Ros | null>(null);

    // ─── Fetch LiDAR status ─────────────────────────────────────────────
    const fetchStatus = useCallback(async () => {
        try {
            const res = await fetch(`${API_BASE}/api/lidar/status`);
            const data: LidarStatus = await res.json();
            setStatus(data);
        } catch (e) {
            console.error('LiDAR status fetch failed:', e);
        }
    }, []);

    useEffect(() => {
        fetchStatus();
        const interval = setInterval(fetchStatus, 3000);
        return () => clearInterval(interval);
    }, [fetchStatus]);

    // ─── Toggle rosbridge server ────────────────────────────────────────
    const toggleServer = useCallback(async () => {
        if (!status) return;
        setLoading(true);
        try {
            const endpoint = status.server_running ? 'stop' : 'start';
            await fetch(`${API_BASE}/api/lidar/${endpoint}`, { method: 'POST' });
            setTimeout(fetchStatus, 800);
        } catch (e) {
            console.error('LiDAR toggle failed:', e);
        } finally {
            setLoading(false);
        }
    }, [status, fetchStatus]);

    // ─── Render a single scan to the canvas (called on each new message) ─
    const renderScan = useCallback((scan: LaserScanMsg) => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const dpr = window.devicePixelRatio || 1;
        const rect = canvas.getBoundingClientRect();
        const w = rect.width * dpr;
        const h = rect.height * dpr;

        if (canvas.width !== w || canvas.height !== h) {
            canvas.width = w;
            canvas.height = h;
        }

        drawRadar(ctx, w, h, scan, dpr);
    }, []);

    // Draw empty radar grid when connected but no data yet
    const renderEmptyRadar = useCallback(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const dpr = window.devicePixelRatio || 1;
        const rect = canvas.getBoundingClientRect();
        canvas.width = rect.width * dpr;
        canvas.height = rect.height * dpr;
        drawRadar(ctx, canvas.width, canvas.height, null, dpr);
    }, []);

    // ─── roslibjs connection + LaserScan subscription ───────────────────
    useEffect(() => {
        if (!status?.server_running || !status.rosbridge_url || !status.topic) {
            if (rosRef.current) {
                rosRef.current.close();
                rosRef.current = null;
                setConnected(false);
            }
            return;
        }

        const url = resolveHostUrl(status.rosbridge_url);
        const topicName = status.topic;
        let retryCount = 0;
        const MAX_RETRIES = 8;
        const RETRY_DELAY = 2000;
        let retryTimer: ReturnType<typeof setTimeout> | null = null;
        let cancelled = false;

        function connect() {
            if (cancelled) return;
            const ros = new ROSLIB.Ros({ url });
            rosRef.current = ros;

            ros.on('connection', () => {
                setConnected(true);
                retryCount = 0;
                renderEmptyRadar();

                const listener = new ROSLIB.Topic({
                    ros,
                    name: topicName!,
                    messageType: 'sensor_msgs/msg/LaserScan',
                    throttle_rate: 200,
                });

                listener.subscribe((msg: unknown) => {
                    renderScan(msg as LaserScanMsg);
                });
            });

            ros.on('close', () => {
                setConnected(false);
                if (!cancelled && retryCount < MAX_RETRIES) {
                    retryCount++;
                    retryTimer = setTimeout(connect, RETRY_DELAY);
                }
            });

            ros.on('error', () => {
                // error is followed by close; close handler does the retry
            });
        }

        // Initial delay to let rosbridge_server start up
        retryTimer = setTimeout(connect, 2000);

        return () => {
            cancelled = true;
            if (retryTimer) clearTimeout(retryTimer);
            if (rosRef.current) {
                rosRef.current.close();
                rosRef.current = null;
            }
            setConnected(false);
        };
    }, [status?.server_running, status?.rosbridge_url, status?.topic, renderScan, renderEmptyRadar]);

    return (
        <div className="flex flex-col h-full bg-background">
            {/* Header */}
            <div className="h-9 shrink-0 border-b border-white/5 bg-surface/50 flex items-center justify-between px-4 backdrop-blur-md">
                <div className="flex items-center gap-2">
                    <Radar className="w-3.5 h-3.5 text-accent_glow" />
                    <span className="text-[10px] font-mono text-accent_glow tracking-wider uppercase">LiDAR</span>
                    {connected && (
                        <span className="ml-1 w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                    )}
                </div>

                {status?.has_lidar && (
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
            <div className="flex-1 flex flex-col items-center justify-center overflow-hidden relative">
                {!status ? (
                    <div className="flex flex-col items-center gap-2">
                        <Loader2 className="w-6 h-6 text-slate-600 animate-spin" />
                        <p className="text-[10px] text-slate-600 font-mono">Checking LiDAR...</p>
                    </div>
                ) : !status.has_lidar ? (
                    <div className="flex flex-col items-center gap-3 p-6 text-center">
                        <div className="p-4 rounded-xl bg-surface/40 border border-white/5">
                            <RadarIcon className="w-8 h-8 text-slate-600" />
                        </div>
                        <p className="text-xs font-mono text-slate-500">
                            This robot does not have a LiDAR configured
                        </p>
                        <p className="text-[10px] text-slate-600">
                            Switch to a LiDAR-equipped profile to enable this feature
                        </p>
                    </div>
                ) : !status.server_running ? (
                    <div className="flex flex-col items-center gap-3 p-6 text-center">
                        <div className="p-4 rounded-xl bg-surface/40 border border-white/5">
                            <Radar className="w-8 h-8 text-slate-500" />
                        </div>
                        <p className="text-xs font-mono text-slate-400">
                            LiDAR available on <span className="text-accent_glow">{status.topic}</span>
                        </p>
                        <p className="text-[10px] text-slate-600">
                            Click the power button above to start streaming
                        </p>
                    </div>
                ) : (
                    <canvas
                        ref={canvasRef}
                        className="w-full h-full"
                        style={{ display: 'block' }}
                    />
                )}
            </div>
        </div>
    );
};
