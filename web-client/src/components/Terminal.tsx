import React, { useEffect, useRef } from 'react';
import type { SystemEvent } from '../types';

interface TerminalProps {
    logs: SystemEvent[];
}

/** Real-time event log. Rendered inside the BottomPanel. */
export const Terminal: React.FC<TerminalProps> = ({ logs }) => {
    const endRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        endRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [logs]);

    return (
        <div className="w-full h-full flex flex-col bg-background">
            <div className="flex-1 p-4 overflow-y-auto font-mono text-[13px] leading-relaxed space-y-2">
                {logs.map((log, i) => (
                    <div key={i} className="flex gap-3 hover:bg-white/5 hover:rounded p-0.5 transition-colors">
                        <span className="text-slate-500 shrink-0 select-none">
                            [{new Date(log.timestamp).toLocaleTimeString()}]
                        </span>
                        <span className={`break-words ${log.event_type === 'ERROR' ? 'text-danger drop-shadow-[0_0_5px_rgba(225,29,72,0.5)]' :
                            log.event_type === 'PROGRAM_ABORT' ? 'text-danger_glow drop-shadow-[0_0_5px_rgba(251,113,133,0.5)]' :
                                log.event_type === 'STEP_START' ? 'text-primary_glow' :
                                    log.event_type === 'PROGRAM_FINISH' ? 'text-accent_glow' :
                                        'text-slate-300'
                            }`}>
                            {log.message}
                        </span>
                    </div>
                ))}
                <div ref={endRef} />
            </div>
        </div>
    );
};