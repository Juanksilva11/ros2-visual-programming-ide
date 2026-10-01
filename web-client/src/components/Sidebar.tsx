import React from 'react';
import { Move, Hourglass, RotateCw, Joystick, Bot, GripVertical } from 'lucide-react';
import type { ControlMode } from '../types';

interface SidebarItemProps {
    type: string;
    label: string;
    icon: React.ElementType;
    colorClass: string;
    borderClass: string;
}

/** Draggable block item in the sidebar. */
const SidebarItem: React.FC<SidebarItemProps> = ({ type, label, icon: Icon, colorClass, borderClass }) => {
    const onDragStart = (event: React.DragEvent, nodeType: string) => {
        event.dataTransfer.setData('application/reactflow', nodeType);
        event.dataTransfer.effectAllowed = 'move';
        document.body.style.cursor = 'grabbing';
    };

    const onDragEnd = () => {
        document.body.style.cursor = 'default';
    };

    return (
        <div
            className={`
        bg-surface/50 p-3 rounded-lg border border-white/5 
        cursor-pointer
        flex items-center gap-3 select-none transition-all duration-300
        hover:bg-surface_light hover:border-${borderClass} hover:shadow-neon-${borderClass === 'primary' ? 'primary' : 'accent'}
        hover:translate-x-1
      `}
            onDragStart={(event) => onDragStart(event, type)}
            onDragEnd={onDragEnd}
            draggable
        >
            <GripVertical size={16} className="text-slate-500 shrink-0" />
            <div className={`p-1.5 rounded ${colorClass} shrink-0`}>
                <Icon size={16} />
            </div>
            <span className="text-sm font-medium text-slate-200">{label}</span>
        </div>
    );
};

interface PaletteEntry extends SidebarItemProps {
    /** Which control modes this block applies to */
    modes: ControlMode[];
}

/**
 * Full block palette. Each entry declares which control modes it supports,
 * so the sidebar only offers blocks the active robot can execute
 * (odometry moves make no sense on a manipulator, and vice versa).
 */
const PALETTE: PaletteEntry[] = [
    {
        type: 'moveNode', label: 'Linear Move', icon: Move,
        colorClass: 'bg-primary/20 text-primary_glow', borderClass: 'primary',
        modes: ['Velocity'],
    },
    {
        type: 'rotateNode', label: 'Rotation', icon: RotateCw,
        colorClass: 'bg-accent/20 text-accent_glow', borderClass: 'accent',
        modes: ['Velocity'],
    },
    {
        type: 'openLoopNode', label: 'Open-Loop', icon: Joystick,
        colorClass: 'bg-amber-900/30 text-amber-400', borderClass: 'amber-400',
        modes: ['Velocity'],
    },
    {
        type: 'jointMoveNode', label: 'Joint Move', icon: Bot,
        colorClass: 'bg-violet-900/30 text-violet-400', borderClass: 'accent',
        modes: ['JointPosition'],
    },
    {
        type: 'waitNode', label: 'Wait / Timer', icon: Hourglass,
        colorClass: 'bg-slate-700/50 text-slate-300', borderClass: 'slate-500',
        modes: ['Velocity', 'JointPosition'],
    },
];

interface SidebarProps {
    /** Active robot's control mode; null while profiles are loading. */
    controlMode: ControlMode | null;
}

/** Left sidebar — block palette filtered by the active robot's control mode. */
export const Sidebar: React.FC<SidebarProps> = ({ controlMode }) => {
    const visibleBlocks = PALETTE.filter(
        entry => controlMode === null || entry.modes.includes(controlMode),
    );

    return (
        <div className="w-64 bg-surface/90 border-r border-white/5 p-4 flex flex-col gap-4 shadow-glass z-20 shrink-0 backdrop-blur-md">
            <div className="mb-2">
                <h2 className="text-sm font-bold text-white uppercase tracking-wider mb-1 font-mono">Control Blocks</h2>
                <p className="text-xs text-slate-400">Drag nodes onto the editor</p>
            </div>

            <div className="space-y-3">
                {visibleBlocks.map(entry => (
                    <SidebarItem
                        key={entry.type}
                        type={entry.type}
                        label={entry.label}
                        icon={entry.icon}
                        colorClass={entry.colorClass}
                        borderClass={entry.borderClass}
                    />
                ))}
            </div>
        </div>
    );
};