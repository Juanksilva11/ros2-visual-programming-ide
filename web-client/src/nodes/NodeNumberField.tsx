import { useEffect, useRef, useState } from 'react';
import { ChevronUp, ChevronDown } from 'lucide-react';

// Static class lookups so Tailwind can see every class at build time
const ACCENTS = {
    primary: {
        label: 'group-hover:text-primary_glow',
        box: 'focus-within:border-primary_glow focus-within:shadow-neon-primary',
        btn: 'hover:text-primary_glow hover:bg-primary/10',
    },
    accent: {
        label: 'group-hover:text-accent_glow',
        box: 'focus-within:border-accent_glow focus-within:shadow-neon-accent',
        btn: 'hover:text-accent_glow hover:bg-accent/10',
    },
    amber: {
        label: 'group-hover:text-amber-400',
        box: 'focus-within:border-amber-400 focus-within:shadow-[0_0_8px_rgba(251,191,36,0.2)]',
        btn: 'hover:text-amber-400 hover:bg-amber-400/10',
    },
    slate: {
        label: 'group-hover:text-slate-300',
        box: 'focus-within:border-slate-400',
        btn: 'hover:text-slate-200 hover:bg-white/10',
    },
    violet: {
        label: 'group-hover:text-violet-400',
        box: 'focus-within:border-violet-400 focus-within:shadow-[0_0_8px_rgba(167,139,250,0.25)]',
        btn: 'hover:text-violet-400 hover:bg-violet-400/10',
    },
} as const;

export type NodeNumberFieldProps = {
    label: string;
    value: number;
    onValueChange: (value: string) => void;
    accent?: keyof typeof ACCENTS;
    min?: number;
    max?: number;
    step?: number;
    placeholder?: string;
    title?: string;
};

const decimalsOf = (n: number): number => (String(n).split('.')[1] ?? '').length;

// Hold-to-repeat timing
const HOLD_DELAY_MS = 400;
const REPEAT_MS = 80;

/**
 * Themed numeric field for canvas nodes: hides the native browser
 * spinners (which get stuck auto-repeating inside React Flow nodes when
 * the canvas swallows the mouseup) and replaces them with custom ▲/▼
 * steppers whose auto-repeat is owned by this component — a global
 * pointerup listener guarantees it always stops.
 */
export const NodeNumberField = ({
    label,
    value,
    onValueChange,
    accent = 'primary',
    min,
    max,
    step = 0.1,
    placeholder,
    title,
}: NodeNumberFieldProps) => {
    const [text, setText] = useState(String(value));
    // Latest value for the hold-to-repeat handlers, which live outside render
    const textRef = useRef(text);
    useEffect(() => {
        textRef.current = text;
    }, [text]);

    const holdTimeout = useRef<number | null>(null);
    const holdInterval = useRef<number | null>(null);

    const commit = (next: string) => {
        textRef.current = next;
        setText(next);
        onValueChange(next);
    };

    const stepBy = (dir: 1 | -1) => {
        const current = Number(textRef.current);
        const base = Number.isFinite(current) ? current : (min ?? 0);
        let next = base + dir * step;
        if (min !== undefined && next < min) next = min;
        if (max !== undefined && next > max) next = max;
        // Round to the precision implied by step/min to avoid float noise
        const decimals = Math.max(decimalsOf(step), min !== undefined ? decimalsOf(min) : 0);
        commit(String(Number(next.toFixed(decimals))));
    };

    const endHold = () => {
        if (holdTimeout.current !== null) window.clearTimeout(holdTimeout.current);
        if (holdInterval.current !== null) window.clearInterval(holdInterval.current);
        holdTimeout.current = null;
        holdInterval.current = null;
    };

    const startHold = (dir: 1 | -1) => (e: React.PointerEvent) => {
        e.preventDefault();
        e.stopPropagation();
        stepBy(dir);
        holdTimeout.current = window.setTimeout(() => {
            holdInterval.current = window.setInterval(() => stepBy(dir), REPEAT_MS);
        }, HOLD_DELAY_MS);
        // The repeat must die with the press, wherever the pointer ends up
        window.addEventListener('pointerup', endHold, { once: true });
    };

    useEffect(() => endHold, []);

    const a = ACCENTS[accent];

    return (
        <div className="group">
            <label className={`block text-[10px] text-slate-500 uppercase font-mono mb-1 transition-colors tracking-wide ${a.label}`}>
                {label}
            </label>
            <div className={`flex items-stretch bg-background/50 border border-white/10 rounded overflow-hidden transition-all ${a.box}`}>
                <input
                    type="number"
                    step={step}
                    min={min}
                    max={max}
                    className="nodrag no-spinner flex-1 min-w-0 bg-transparent px-2 py-1.5 text-xs text-slate-200 focus:outline-none font-mono"
                    value={text}
                    onChange={e => commit(e.target.value)}
                    placeholder={placeholder}
                    title={title}
                />
                <div className="flex flex-col border-l border-white/10 shrink-0">
                    <button
                        type="button"
                        tabIndex={-1}
                        aria-label={`Increase ${label}`}
                        className={`nodrag flex-1 flex items-center px-1 text-slate-500 transition-colors ${a.btn}`}
                        onPointerDown={startHold(1)}
                    >
                        <ChevronUp size={10} />
                    </button>
                    <button
                        type="button"
                        tabIndex={-1}
                        aria-label={`Decrease ${label}`}
                        className={`nodrag flex-1 flex items-center px-1 text-slate-500 transition-colors border-t border-white/10 ${a.btn}`}
                        onPointerDown={startHold(-1)}
                    >
                        <ChevronDown size={10} />
                    </button>
                </div>
            </div>
        </div>
    );
};
