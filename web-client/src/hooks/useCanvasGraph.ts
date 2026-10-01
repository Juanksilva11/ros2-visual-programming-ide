import { useEffect, useCallback } from 'react';
import {
    useNodesState,
    useEdgesState,
    addEdge,
    useReactFlow,
    type Connection,
    type Edge,
} from '@xyflow/react';
import { createNode, type AppNode } from '../nodes';
import type { RobotProfile } from '../types';

interface UseCanvasGraphOptions {
    /** Node currently executing (from STEP events) — gets the neon pulse. */
    executingNodeId: string | null;
    /** Nodes flagged by the compiler — get the red error highlight. */
    errorNodeIds: string[];
    /** Active HAL profile — consulted by node factories (e.g. joint metadata). */
    activeProfile: RobotProfile | null;
    /** Called when the user edits the graph (e.g. to clear stale errors). */
    onGraphEdit?: () => void;
}

/**
 * Canvas state: React Flow nodes/edges, drag-and-drop block creation,
 * connection rules for the linear-DAG program model, and visual flag
 * synchronization (executing / error highlights).
 */
export function useCanvasGraph({ executingNodeId, errorNodeIds, activeProfile, onGraphEdit }: UseCanvasGraphOptions) {
    const [nodes, setNodes, onNodesChange] = useNodesState<AppNode>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
    const { screenToFlowPosition } = useReactFlow();

    // ─── Visual flag synchronization ──────────────────────────────────────

    useEffect(() => {
        setNodes(nds =>
            nds.map(node => {
                const shouldExecute = node.id === executingNodeId;
                if (node.data.isExecuting === shouldExecute) return node;
                return { ...node, data: { ...node.data, isExecuting: shouldExecute } } as AppNode;
            })
        );
    }, [executingNodeId, setNodes]);

    useEffect(() => {
        setNodes(nds =>
            nds.map(node => {
                const shouldError = errorNodeIds.includes(node.id);
                if ((node.data.hasError ?? false) === shouldError) return node;
                return { ...node, data: { ...node.data, hasError: shouldError } } as AppNode;
            })
        );
    }, [errorNodeIds, setNodes]);

    // ─── Node data editing ────────────────────────────────────────────────

    const onNodeDataChange = useCallback(
        (id: string) => (key: string, value: string) => {
            setNodes(nds =>
                nds.map(node => {
                    if (node.id !== id) return node;
                    return { ...node, data: { ...node.data, [key]: parseFloat(value) || value } } as AppNode;
                })
            );
        },
        [setNodes],
    );

    // ─── Drag-and-drop block creation ─────────────────────────────────────

    const onDragOver = useCallback((event: React.DragEvent) => {
        event.preventDefault();
        event.dataTransfer.dropEffect = 'move';
    }, []);

    const onDrop = useCallback(
        (event: React.DragEvent) => {
            event.preventDefault();
            const type = event.dataTransfer.getData('application/reactflow');
            if (!type) return;

            const position = screenToFlowPosition({ x: event.clientX, y: event.clientY });
            const node = createNode(type, position, onNodeDataChange, activeProfile);
            if (!node) return;
            setNodes(nds => nds.concat(node));
        },
        [screenToFlowPosition, onNodeDataChange, setNodes, activeProfile],
    );

    // ─── Connection rules ─────────────────────────────────────────────────

    const onConnect = useCallback(
        (params: Connection) => {
            onGraphEdit?.();
            setEdges(eds => addEdge(params, eds));
        },
        [setEdges, onGraphEdit],
    );

    // UX guard: block connections that would violate the linear-sequence
    // invariants (self-loops, fan-out/fan-in, cycles) before they are drawn.
    // The compiler re-validates everything on RUN as the source of truth.
    const isValidConnection = useCallback(
        (conn: Edge | Connection) => {
            if (!conn.source || !conn.target || conn.source === conn.target) return false;
            if (edges.some(e => e.source === conn.source)) return false; // one outgoing per block
            if (edges.some(e => e.target === conn.target)) return false; // one incoming per block
            // Walk forward from the target; reaching the source would close a cycle.
            const next = new Map(edges.map(e => [e.source, e.target]));
            let cursor: string | undefined = conn.target;
            while (cursor) {
                if (cursor === conn.source) return false;
                cursor = next.get(cursor);
            }
            return true;
        },
        [edges],
    );

    return { nodes, edges, onNodesChange, onEdgesChange, onConnect, isValidConnection, onDrop, onDragOver };
}
