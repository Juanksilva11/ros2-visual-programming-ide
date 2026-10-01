import { memo } from 'react';
import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import { Hourglass } from 'lucide-react';
import { NodeNumberField } from './NodeNumberField';

export type WaitNodeData = {
    duration: number;
    isExecuting?: boolean;
    hasError?: boolean;
    onChange?: (key: string, value: string) => void;
};

export type WaitNodeType = Node<WaitNodeData, 'waitNode'>;

export const WaitNode = memo(({ data, isConnectable }: NodeProps<WaitNodeType>) => {
    return (
        <div className={`glass-panel min-w-[150px] overflow-hidden transition-all duration-300 hover:border-slate-400 group/node ${data.isExecuting ? 'neon-pulse-executing' : data.hasError ? 'node-error' : ''}`}>
            <div className="bg-surface/80 px-4 py-2 border-b border-white/5 flex items-center gap-2">
                <div className="p-1 bg-slate-700/50 rounded text-slate-300">
                    <Hourglass size={14} />
                </div>
                <span className="text-[11px] font-bold text-white uppercase tracking-widest font-mono">
                    Wait
                </span>
            </div>

            <div className="p-3">
                <NodeNumberField
                    label="Duration (Seconds)"
                    accent="slate"
                    value={data.duration}
                    min={0}
                    step={0.1}
                    placeholder="e.g. 1.0"
                    onValueChange={v => data.onChange?.('duration', v)}
                />
            </div>

            <Handle type="target" position={Position.Top} isConnectable={isConnectable}
                className="!bg-slate-500 !w-3 !h-3 !border-2 !border-background transition-transform hover:scale-110" />
            <Handle type="source" position={Position.Bottom} isConnectable={isConnectable}
                className="!bg-slate-400 !w-3 !h-3 !border-2 !border-background transition-transform hover:scale-110" />
        </div>
    );
});
