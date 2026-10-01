import { useState, useEffect, useRef, useCallback } from 'react';

const BOTTOM_MIN_PX = 120;
const BOTTOM_MAX_VIEWPORT_RATIO = 0.6;
const RIGHT_MIN_PX = 280;
const RIGHT_MAX_PX = 600;

/**
 * IDE panel layout: visibility toggles for the three panels and
 * drag-to-resize logic for the bottom/right splitters.
 */
export function usePanelLayout() {
    const [showLeftSidebar, setShowLeftSidebar] = useState(true);
    const [showBottomPanel, setShowBottomPanel] = useState(true);
    const [showRightSidebar, setShowRightSidebar] = useState(true);

    const [bottomHeight, setBottomHeight] = useState(220);
    const [rightWidth, setRightWidth] = useState(340);

    const bottomDragging = useRef(false);
    const rightDragging = useRef(false);

    const startBottomResize = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        bottomDragging.current = true;
    }, []);

    const startRightResize = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        rightDragging.current = true;
    }, []);

    useEffect(() => {
        const onMouseMove = (e: MouseEvent) => {
            if (bottomDragging.current) {
                const newHeight = window.innerHeight - e.clientY - 1; // -1 for splitter offset
                setBottomHeight(Math.min(
                    Math.max(newHeight, BOTTOM_MIN_PX),
                    window.innerHeight * BOTTOM_MAX_VIEWPORT_RATIO,
                ));
            }
            if (rightDragging.current) {
                const newWidth = window.innerWidth - e.clientX;
                setRightWidth(Math.min(Math.max(newWidth, RIGHT_MIN_PX), RIGHT_MAX_PX));
            }
        };
        const onMouseUp = () => {
            bottomDragging.current = false;
            rightDragging.current = false;
        };
        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);
        return () => {
            window.removeEventListener('mousemove', onMouseMove);
            window.removeEventListener('mouseup', onMouseUp);
        };
    }, []);

    return {
        showLeftSidebar,
        showBottomPanel,
        showRightSidebar,
        toggleLeftSidebar: useCallback(() => setShowLeftSidebar(v => !v), []),
        toggleBottomPanel: useCallback(() => setShowBottomPanel(v => !v), []),
        toggleRightSidebar: useCallback(() => setShowRightSidebar(v => !v), []),
        bottomHeight,
        rightWidth,
        startBottomResize,
        startRightResize,
    };
}
