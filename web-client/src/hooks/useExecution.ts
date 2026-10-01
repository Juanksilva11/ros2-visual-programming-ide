import { useState, useCallback } from 'react';
import type { Node, Edge } from '@xyflow/react';
import { API_BASE } from '../config';
import { compileGraphToSequence, GraphValidationError } from '../features/editor/compiler';

/**
 * Program execution lifecycle: compiles the visual graph, submits it to
 * the backend, and tracks run state plus compile/backend errors.
 *
 * `executionOrder` holds the compiled node IDs so STEP events from the
 * WebSocket can be mapped back to canvas nodes.
 */
export function useExecution() {
    const [isRunning, setIsRunning] = useState(false);
    const [executionOrder, setExecutionOrder] = useState<string[]>([]);
    const [compileError, setCompileError] = useState<string | null>(null);
    const [errorNodeIds, setErrorNodeIds] = useState<string[]>([]);

    const clearCompileError = useCallback(() => {
        setCompileError(null);
        setErrorNodeIds([]);
    }, []);

    const execute = useCallback(async (nodes: Node[], edges: Edge[]) => {
        clearCompileError();

        let compiled;
        try {
            compiled = compileGraphToSequence(nodes, edges);
        } catch (error) {
            if (error instanceof GraphValidationError) {
                setCompileError(error.message);
                setErrorNodeIds(error.nodeIds);
            } else {
                console.error(error);
                setCompileError('Unexpected error while compiling the program.');
            }
            return;
        }

        setExecutionOrder(compiled.order);
        setIsRunning(true);
        try {
            const res = await fetch(`${API_BASE}/api/execute`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(compiled.payload),
            });
            if (!res.ok) {
                // Backend rejection (e.g. semantic validation against the HAL
                // profile limits) — surface the reason in the canvas banner.
                const body = await res.json().catch(() => null);
                setCompileError(body?.message ?? `Backend rejected the program (HTTP ${res.status}).`);
            }
        } catch (error) {
            console.error(error);
            setCompileError('Failed to reach the backend. Is the server running?');
        } finally {
            setIsRunning(false);
        }
    }, [clearCompileError]);

    const abort = useCallback(async () => {
        try {
            await fetch(`${API_BASE}/api/abort`, { method: 'POST' });
        } catch (error) {
            console.error('Abort failed:', error);
        } finally {
            setIsRunning(false);
        }
    }, []);

    return { isRunning, executionOrder, compileError, errorNodeIds, execute, abort, clearCompileError };
}
