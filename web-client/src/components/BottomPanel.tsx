import React, { useState, useCallback } from 'react';
import { Terminal as TerminalIcon, Search, Loader2 } from 'lucide-react';
import { Terminal } from './Terminal';
import { RosInfoPanel } from './RosInfoPanel';
import type { SystemEvent } from '../types';

type BottomTab = 'terminal' | 'rosInfo';

interface BottomPanelProps {
    logs: SystemEvent[];
    height: number;
}

/**
 * IDE-style bottom panel with tab navigation.
 * Contains Terminal and ROS Info Panel as switchable tabs.
 */
export const BottomPanel: React.FC<BottomPanelProps> = ({ logs, height }) => {
    const [activeTab, setActiveTab] = useState<BottomTab>('terminal');
    const [isScanning, setIsScanning] = useState(false);
    // Incrementing signal consumed by RosInfoPanel to trigger a graph scan
    const [scanSignal, setScanSignal] = useState(0);

    const handleScan = useCallback(() => {
        setIsScanning(true);
        setScanSignal(s => s + 1);
        // Brief delay to show spinner, the panel handles actual loading
        setTimeout(() => setIsScanning(false), 800);
    }, []);

    return (
        <div style={{ height: `${height}px` }} className="shrink-0 flex flex-col border-t border-white/5 bg-background">
            {/* Tab Bar */}
            <div className="h-9 shrink-0 flex items-center bg-surface/50 border-b border-white/5 backdrop-blur-md px-1">
                {/* Tabs */}
                <button
                    onClick={() => setActiveTab('terminal')}
                    className={`flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-mono tracking-wide transition-all rounded-t ${activeTab === 'terminal'
                            ? 'text-primary_glow border-b-2 border-primary_glow bg-primary/5'
                            : 'text-slate-500 hover:text-slate-300 hover:bg-white/5'
                        }`}
                >
                    <TerminalIcon size={12} />
                    Terminal
                </button>
                <button
                    onClick={() => setActiveTab('rosInfo')}
                    className={`flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-mono tracking-wide transition-all rounded-t ${activeTab === 'rosInfo'
                            ? 'text-accent_glow border-b-2 border-accent_glow bg-accent/5'
                            : 'text-slate-500 hover:text-slate-300 hover:bg-white/5'
                        }`}
                >
                    <Search size={12} />
                    ROS Info
                </button>

                {/* Tab-bar actions (right side) */}
                <div className="ml-auto flex items-center gap-1 pr-2">
                    {activeTab === 'rosInfo' && (
                        <button
                            onClick={handleScan}
                            disabled={isScanning}
                            className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-mono btn-cyber btn-cyber-accent"
                        >
                            {isScanning ? <Loader2 size={10} className="animate-spin" /> : <Search size={10} />}
                            SCAN
                        </button>
                    )}
                </div>
            </div>

            {/* Tab Content */}
            <div className="flex-1 overflow-hidden">
                {activeTab === 'terminal' ? (
                    <Terminal logs={logs} />
                ) : (
                    <RosInfoPanel scanSignal={scanSignal} />
                )}
            </div>
        </div>
    );
};
