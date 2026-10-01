import { useState, useEffect, useRef, useCallback } from 'react';
import { WS_URL } from '../config';
import type { SystemEvent } from '../types';

const MAX_LOG_ENTRIES = 50;
const RECONNECT_DELAY_MS = 3000;

/**
 * Manages the WebSocket connection to the Rust backend.
 *
 * Returns connection status, a scrollable log list, a progress value
 * (0–100 or null), and the 1-indexed step currently executing.
 * Automatically reconnects while mounted.
 */
export function useRobotConnection() {
    const [isConnected, setIsConnected] = useState(false);
    const [logs, setLogs] = useState<SystemEvent[]>([]);
    const [progress, setProgress] = useState<number | null>(null);
    const [activeStepIndex, setActiveStepIndex] = useState<number | null>(null);

    const ws = useRef<WebSocket | null>(null);
    const hasLoggedConnection = useRef(false);
    const reconnectTimer = useRef<number | null>(null);

    const addLog = useCallback((log: SystemEvent) => {
        setLogs(prev => [...prev, log].slice(-MAX_LOG_ENTRIES));
    }, []);

    // Named function expression so the onclose handler can schedule a
    // reconnect without referencing the useCallback binding itself.
    const connect = useCallback(function connectSocket() {
        try {
            const socket = new WebSocket(WS_URL);

            socket.onopen = () => {
                setIsConnected(true);

                // Only add the connection log once (avoids duplicates on reconnect)
                if (!hasLoggedConnection.current) {
                    hasLoggedConnection.current = true;
                    addLog({
                        timestamp: new Date().toISOString(),
                        event_type: 'INFO',
                        message: 'Connection established with the control system.',
                    });
                }
            };

            socket.onclose = () => {
                setIsConnected(false);
                ws.current = null;
                hasLoggedConnection.current = false;
                reconnectTimer.current = window.setTimeout(connectSocket, RECONNECT_DELAY_MS);
            };

            socket.onmessage = (event) => {
                try {
                    const data: SystemEvent = JSON.parse(event.data);

                    if (data.event_type === 'PROGRESS' && data.metadata?.percent !== undefined) {
                        setProgress(data.metadata.percent);
                    } else {
                        addLog(data);

                        // Track the executing step for visual block feedback
                        if (data.event_type === 'STEP_START' && data.metadata?.step !== undefined) {
                            setActiveStepIndex(data.metadata.step);
                        }

                        // Clear step highlight when the program ends or is aborted
                        if (data.event_type === 'PROGRAM_FINISH' || data.event_type === 'PROGRAM_ABORT') {
                            setProgress(null);
                            setActiveStepIndex(null);
                        }
                    }
                } catch (e) {
                    console.error('Error parsing WebSocket message:', e);
                }
            };

            ws.current = socket;
        } catch (e) {
            console.error('WebSocket connection error:', e);
        }
    }, [addLog]);

    useEffect(() => {
        connect();
        return () => {
            if (reconnectTimer.current !== null) {
                window.clearTimeout(reconnectTimer.current);
            }
            ws.current?.close();
        };
    }, [connect]);

    return { isConnected, logs, progress, activeStepIndex, addLog };
}
