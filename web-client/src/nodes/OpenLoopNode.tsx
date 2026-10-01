import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { Node, NodeProps } from '@xyflow/react';
import { Joystick } from 'lucide-react';
import { NodeNumberField } from './NodeNumberField';

export type OpenLoopNodeData = {
    linear_x: number;
    angular_z: number;
    duration: number;
    isExecuting?: boolean;
    hasError?: boolean;
    onChange?: (key: string, value: string) => void;
};

export type OpenLoopNodeType = Node<OpenLoopNodeData, 'openLoopNode'>;

export const OpenLoopNode = memo(({ data, isConnectable }: NodeProps<OpenLoopNodeType>) => {
    return (
        <div className={`glass-panel min-w-[200px] overflow-hidden transition-all duration-300 hover:border-amber-400 hover:shadow-[0_0_15px_rgba(251,191,36,0.3)] group/node ${data.isExecuting ? 'neon-pulse-executing' : data.hasError ? 'node-error' : ''}`}>
            <div className="bg-surface/80 px-4 py-2 border-b border-white/5 flex items-center gap-2">
                <div className="p-1 bg-amber-900/30 rounded text-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.3)]">
                    <Joystick size={14} />
                </div>
                <span className="text-[11px] font-bold text-white uppercase tracking-widest font-mono">
                    Open-Loop
                </span>
            </div>

            <div className="p-3 space-y-3">
                <NodeNumberField
                    label="Linear Vel. (m/s)"
                    accent="amber"
                    value={data.linear_x}
                    step={0.1}
                    title="Negative velocity drives in reverse"
                    onValueChange={v => data.onChange?.('linear_x', v)}
                />
                <NodeNumberField
                    label="Angular Vel. (rad/s)"
                    accent="amber"
                    value={data.angular_z}
                    step={0.1}
                    title="Negative velocity rotates clockwise"
                    onValueChange={v => data.onChange?.('angular_z', v)}
                />
                <NodeNumberField
                    label="Duration (s)"
                    accent="amber"
                    value={data.duration}
                    min={0.1}
                    step={0.5}
                    onValueChange={v => data.onChange?.('duration', v)}
                />
            </div>

            <Handle type="target" position={Position.Top} isConnectable={isConnectable}
                className="!bg-slate-500 !w-3 !h-3 !border-2 !border-background transition-transform hover:scale-110" />
            <Handle type="source" position={Position.Bottom} isConnectable={isConnectable}
                className="!bg-amber-400 !w-3 !h-3 !border-2 !border-background shadow-[0_0_8px_rgba(251,191,36,0.4)] transition-transform hover:scale-110" />
        </div>
    );
});
