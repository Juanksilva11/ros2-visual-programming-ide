import React, { useState, useCallback, useEffect } from 'react';
import { Radio, Settings, Zap, ChevronRight, Loader2, X } from 'lucide-react';
import { API_BASE } from '../config';

type TabKind = 'topics' | 'services' | 'actions';

interface IntrospectData {
    topics: string[];
    services: string[];
    actions: string[];
}

interface DetailData {
    info: string;
    interface: string;
}

interface RosInfoPanelProps {
    /** Increment to trigger a graph scan (SCAN button in the tab bar). */
    scanSignal?: number;
}

/** ROS 2 graph introspection panel. Rendered inside the BottomPanel. */
export const RosInfoPanel: React.FC<RosInfoPanelProps> = ({ scanSignal = 0 }) => {
    const [data, setData] = useState<IntrospectData | null>(null);
    const [activeTab, setActiveTab] = useState<TabKind>('topics');
    const [isLoading, setIsLoading] = useState(false);
    const [selectedItem, setSelectedItem] = useState<string | null>(null);
    const [detail, setDetail] = useState<DetailData | null>(null);
    const [isDetailLoading, setIsDetailLoading] = useState(false);

    const fetchGraph = useCallback(async () => {
        setIsLoading(true);
        setSelectedItem(null);
        setDetail(null);
        try {
            const res = await fetch(`${API_BASE}/api/introspect`);
            const json: IntrospectData = await res.json();
            setData(json);
        } catch (e) {
            console.error('Introspection failed:', e);
        } finally {
            setIsLoading(false);
        }
    }, []);

    // Scan on demand: the BottomPanel's SCAN button bumps scanSignal
    useEffect(() => {
        if (scanSignal > 0) fetchGraph();
    }, [scanSignal, fetchGraph]);

    const fetchDetail = useCallback(async (kind: TabKind, name: string) => {
        setSelectedItem(name);
        setIsDetailLoading(true);
        setDetail(null);
        try {
            const cleanName = name.startsWith('/') ? name.slice(1) : name;
            const kindSingular = kind === 'topics' ? 'topic' : kind === 'services' ? 'service' : 'action';
            const res = await fetch(`${API_BASE}/api/introspect/${kindSingular}/${cleanName}`);
            const json: DetailData = await res.json();
            setDetail(json);
        } catch (e) {
            console.error('Detail fetch failed:', e);
        } finally {
            setIsDetailLoading(false);
        }
    }, []);

    const currentList = data ? data[activeTab] : [];

    const tabConfig: { key: TabKind; label: string; icon: React.ReactNode }[] = [
        { key: 'topics', label: 'Topics', icon: <Radio size={12} /> },
        { key: 'services', label: 'Services', icon: <Settings size={12} /> },
        { key: 'actions', label: 'Actions', icon: <Zap size={12} /> },
    ];

    return (
        <div className="flex flex-col h-full bg-background">
            {/* Sub-tabs for Topics / Services / Actions */}
            <div className="flex border-b border-white/5 shrink-0">
                {tabConfig.map((tab) => (
                    <button
                        key={tab.key}
                        onClick={() => { setActiveTab(tab.key); setSelectedItem(null); setDetail(null); }}
                        className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 text-[10px] font-mono uppercase tracking-wider transition-all
                            ${activeTab === tab.key
                                ? 'text-accent_glow border-b-2 border-accent_glow bg-accent/5'
                                : 'text-slate-500 hover:text-slate-300 hover:bg-white/5'}`}
                    >
                        {tab.icon}
                        {tab.label}
                        {data && <span className="text-slate-600 ml-1">({data[tab.key].length})</span>}
                    </button>
                ))}
            </div>

            {/* Content — horizontal split: list left, detail right */}
            <div className="flex-1 overflow-hidden flex min-h-0">
                {/* List */}
                <div className={`overflow-y-auto ${selectedItem ? 'w-[40%] shrink-0 border-r border-white/5' : 'flex-1'} transition-all`}>
                    {!data ? (
                        <div className="p-4 text-center text-slate-500 text-xs font-mono">
                            {isLoading ? (
                                <div className="flex items-center justify-center gap-2">
                                    <Loader2 className="animate-spin" size={14} />
                                    Scanning ROS 2 graph...
                                </div>
                            ) : (
                                'Click SCAN to discover the ROS 2 graph.'
                            )}
                        </div>
                    ) : currentList.length === 0 ? (
                        <div className="p-4 text-center text-slate-500 text-xs font-mono">
                            No {activeTab} found.
                        </div>
                    ) : (
                        currentList.map((item, i) => (
                            <button
                                key={i}
                                onClick={() => fetchDetail(activeTab, item)}
                                className={`w-full text-left px-4 py-2 text-xs font-mono flex items-center gap-2 transition-all hover:bg-white/5 group
                                    ${selectedItem === item ? 'bg-accent/10 text-accent_glow' : 'text-slate-400'}`}
                            >
                                <ChevronRight size={10} className={`transition-transform ${selectedItem === item ? 'rotate-90 text-accent_glow' : 'text-slate-600 group-hover:text-slate-400'}`} />
                                <span className="truncate">{item}</span>
                            </button>
                        ))
                    )}
                </div>

                {/* Detail pane — right side */}
                {selectedItem && (
                    <div className="flex-1 overflow-y-auto p-3 space-y-3 animate-fade-in">
                        <div className="flex items-center justify-between">
                            <span className="text-[10px] font-mono text-accent_glow uppercase tracking-wider truncate">{selectedItem}</span>
                            <button onClick={() => { setSelectedItem(null); setDetail(null); }} className="text-slate-500 hover:text-white transition-colors shrink-0 ml-2">
                                <X size={12} />
                            </button>
                        </div>
                        {isDetailLoading ? (
                            <div className="flex items-center justify-center py-8">
                                <Loader2 className="animate-spin text-accent_glow" size={20} />
                            </div>
                        ) : detail ? (
                            <>
                                <div>
                                    <p className="text-[10px] text-slate-500 font-mono uppercase mb-1">Info</p>
                                    <pre className="text-[11px] text-slate-300 bg-surface/80 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap border border-white/5">{detail.info}</pre>
                                </div>
                                <div>
                                    <p className="text-[10px] text-slate-500 font-mono uppercase mb-1">Interface Definition</p>
                                    <pre className="text-[11px] text-primary_glow bg-surface/80 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap border border-white/5">{detail.interface}</pre>
                                </div>
                            </>
                        ) : null}
                    </div>
                )}
            </div>
        </div>
    );
};
