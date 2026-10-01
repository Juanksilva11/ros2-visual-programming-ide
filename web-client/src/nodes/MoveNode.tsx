import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { Node, NodeProps } from '@xyflow/react';
import { Move } from 'lucide-react';
import { NodeNumberField } from './NodeNumberField';

export type MoveNodeData = {
    distance: number;
    speed: number;
    isExecuting?: boolean;
    hasError?: boolean;
    onChange?: (key: string, value: string) => void;
};

export type MoveNodeType = Node<MoveNodeData, 'moveNode'>;

export const MoveNode = memo(({ data, isConnectable }: NodeProps<MoveNodeType>) => {
    return (
        <div className={`glass-panel min-w-[200px] overflow-hidden transition-all duration-300 hover:border-primary_glow hover:shadow-neon-primary group/node ${data.isExecuting ? 'neon-pulse-executing' : data.hasError ? 'node-error' : ''}`}>
            <div className="bg-surface/80 px-4 py-2 border-b border-white/5 flex items-center gap-2">
                <div className="p-1 bg-primary/20 rounded text-primary_glow shadow-neon-primary">
                    <Move size={14} />
                </div>
                <span className="text-[11px] font-bold text-white uppercase tracking-widest font-mono">
                    Linear Move (Odom)
                </span>
            </div>

            <div className="p-3 space-y-3">
                <NodeNumberField
                    label="Distance (m)"
                    accent="primary"
                    value={data.distance}
                    step={0.1}
                    placeholder="e.g. 2.0"
                    title="Negative distance drives in reverse"
                    onValueChange={v => data.onChange?.('distance', v)}
                />
                <NodeNumberField
                    label="Speed (m/s)"
                    accent="primary"
                    value={data.speed}
                    min={0.01}
                    step={0.1}
                    onValueChange={v => data.onChange?.('speed', v)}
                />
            </div>

            <Handle type="target" position={Position.Top} isConnectable={isConnectable}
                className="!bg-slate-500 !w-3 !h-3 !border-2 !border-background transition-transform hover:scale-110" />
            <Handle type="source" position={Position.Bottom} isConnectable={isConnectable}
                className="!bg-primary_glow !w-3 !h-3 !border-2 !border-background shadow-neon-primary transition-transform hover:scale-110" />
        </div>
    );
});
