import React from 'react';
import {
    Loader2, Play, Activity, Wifi, WifiOff, ChevronDown, OctagonX,
    PanelLeft, PanelBottom, PanelRight,
} from 'lucide-react';
import type { RobotProfile } from '../types';

interface HeaderBarProps {
    isConnected: boolean;
    isRunning: boolean;
    onRun: () => void;
    onAbort: () => void;

    profiles: RobotProfile[];
    activeProfileId: string;
    onProfileChange: (id: string) => void;

    showLeftSidebar: boolean;
    showBottomPanel: boolean;
    showRightSidebar: boolean;
    onToggleLeftSidebar: () => void;
    onToggleBottomPanel: () => void;
    onToggleRightSidebar: () => void;
}

/** Top toolbar: branding, RUN / E-STOP, profile selector, connection badge, panel toggles. */
export const HeaderBar: React.FC<HeaderBarProps> = ({
    isConnected,
    isRunning,
    onRun,
    onAbort,
    profiles,
    activeProfileId,
    onProfileChange,
    showLeftSidebar,
    showBottomPanel,
    showRightSidebar,
    onToggleLeftSidebar,
    onToggleBottomPanel,
    onToggleRightSidebar,
}) => (
    <header className="h-12 shrink-0 border-b border-white/5 flex items-center justify-between px-4 bg-surface/80 backdrop-blur-sm z-20">
        {/* Left: Logo + Actions */}
        <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
                <Activity className="w-4 h-4 text-primary_glow" />
                <h1 className="text-sm font-bold tracking-tight text-white font-mono">
                    ROS 2 <span className="text-primary_glow">Low-Code</span>
                </h1>
            </div>

            <div className="w-px h-6 bg-white/10" />

            <button
                onClick={onRun}
                disabled={!isConnected || isRunning}
                className={`flex items-center gap-1.5 px-4 py-1 rounded-md text-xs font-bold transition-all ${isConnected && !isRunning ? 'btn-cyber btn-cyber-primary' : 'bg-surface_light text-slate-500 cursor-not-allowed'}`}
            >
                {isRunning ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} fill="currentColor" />}
                {isRunning ? 'RUNNING' : 'RUN'}
            </button>
            <button
                onClick={onAbort}
                disabled={!isRunning}
                className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-bold transition-all ${isRunning ? 'btn-cyber btn-cyber-danger' : 'bg-surface_light text-slate-500 cursor-not-allowed'}`}
            >
                <OctagonX size={13} />
                E-STOP
            </button>
        </div>

        {/* Right: Profile, Status, Panel Toggles */}
        <div className="flex items-center gap-2">
            {/* Profile selector */}
            <div className="relative">
                <select
                    value={activeProfileId}
                    onChange={e => onProfileChange(e.target.value)}
                    className="appearance-none bg-surface border border-white/10 text-slate-200 text-[11px] font-mono rounded-md pl-2.5 pr-7 py-1 focus:outline-none focus:border-primary_glow cursor-pointer hover:border-white/20 transition-all"
                >
                    {profiles.map(p => (
                        <option key={p.id} value={p.id}>{p.name}</option>
                    ))}
                </select>
                <ChevronDown size={10} className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 pointer-events-none" />
            </div>

            {/* Connection status */}
            <div className={`flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-medium border ${isConnected ? 'bg-emerald-950/30 text-emerald-400 border-emerald-800/40' : 'bg-red-950/30 text-red-400 border-red-800/40'}`}>
                {isConnected ? <Wifi className="w-3 h-3" /> : <WifiOff className="w-3 h-3" />}
                {isConnected ? 'ONLINE' : 'OFFLINE'}
            </div>

            <div className="w-px h-6 bg-white/10" />

            {/* Panel toggle buttons */}
            <button
                onClick={onToggleLeftSidebar}
                className={`p-1.5 rounded-md transition-all ${showLeftSidebar ? 'text-primary_glow bg-primary/10' : 'text-slate-500 hover:text-slate-300 hover:bg-white/5'}`}
                title="Toggle Blocks Panel"
            >
                <PanelLeft size={15} />
            </button>
            <button
                onClick={onToggleBottomPanel}
                className={`p-1.5 rounded-md transition-all ${showBottomPanel ? 'text-primary_glow bg-primary/10' : 'text-slate-500 hover:text-slate-300 hover:bg-white/5'}`}
                title="Toggle Terminal Panel"
            >
                <PanelBottom size={15} />
            </button>
            <button
                onClick={onToggleRightSidebar}
                className={`p-1.5 rounded-md transition-all ${showRightSidebar ? 'text-primary_glow bg-primary/10' : 'text-slate-500 hover:text-slate-300 hover:bg-white/5'}`}
                title="Toggle Sensors Panel"
            >
                <PanelRight size={15} />
            </button>
        </div>
    </header>
);
