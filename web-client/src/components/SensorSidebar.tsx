import React, { useState, useRef, useEffect, useCallback } from 'react';
import { CameraPanel } from './CameraPanel';
import { LidarPanel } from './LidarPanel';

/**
 * Right sidebar displaying sensor feeds.
 *
 * Contains a Camera panel on top and a LiDAR panel below, separated
 * by a resizable vertical splitter.
 */
interface SensorSidebarProps {
    width: number;
}

export const SensorSidebar: React.FC<SensorSidebarProps> = ({ width }) => {
    const [split, setSplit] = useState(50);
    const dragging = useRef(false);
    const containerRef = useRef<HTMLDivElement>(null);

    const onSplitterDown = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        dragging.current = true;
    }, []);

    useEffect(() => {
        const onMove = (e: MouseEvent) => {
            if (!dragging.current || !containerRef.current) return;
            const rect = containerRef.current.getBoundingClientRect();
            const y = e.clientY - rect.top;
            setSplit(Math.min(Math.max((y / rect.height) * 100, 15), 85));
        };
        const onUp = () => { dragging.current = false; };
        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
        return () => {
            window.removeEventListener('mousemove', onMove);
            window.removeEventListener('mouseup', onUp);
        };
    }, []);

    return (
        <div
            ref={containerRef}
            style={{ width: `${width}px` }}
            className="shrink-0 flex flex-col border-l border-white/5 bg-background"
        >
            {/* Camera */}
            <div style={{ height: `${split}%` }} className="overflow-hidden flex flex-col">
                <CameraPanel />
            </div>

            {/* Splitter */}
            <div
                onMouseDown={onSplitterDown}
                className="h-1.5 shrink-0 bg-surface cursor-row-resize hover:bg-primary/30 active:bg-primary_glow/50 transition-colors border-y border-white/5 flex items-center justify-center"
            >
                <div className="w-8 h-0.5 rounded-full bg-white/20" />
            </div>

            {/* LiDAR */}
            <div style={{ height: `${100 - split}%` }} className="overflow-hidden flex flex-col">
                <LidarPanel />
            </div>
        </div>
    );
};
