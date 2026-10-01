/**
 * Network configuration — the single source of truth for every backend URL.
 *
 * The backend host follows the page's hostname so the IDE works both on
 * localhost and when accessed from another machine on the LAN.
 */
const BACKEND_HOST = window.location.hostname;
const BACKEND_PORT = 3000;

/** REST API base URL (Axum server). */
export const API_BASE = `http://${BACKEND_HOST}:${BACKEND_PORT}`;

/** WebSocket endpoint for real-time system events. */
export const WS_URL = `ws://${BACKEND_HOST}:${BACKEND_PORT}/ws`;

/**
 * Backend-provided URL templates (camera stream, rosbridge) use `{host}`
 * as a placeholder so each client can reach the servers through the same
 * host it loaded the page from.
 */
export const resolveHostUrl = (template: string): string =>
    template.replace('{host}', BACKEND_HOST);
