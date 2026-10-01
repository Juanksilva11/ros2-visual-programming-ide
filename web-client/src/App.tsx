import { useMemo } from 'react';
import { ReactFlow, Background, Controls, ReactFlowProvider } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { TriangleAlert, X } from 'lucide-react';

import { useRobotConnection } from './hooks/useRobotConnection';
import { useProfiles } from './hooks/useProfiles';
import { useExecution } from './hooks/useExecution';
import { usePanelLayout } from './hooks/usePanelLayout';
import { useCanvasGraph } from './hooks/useCanvasGraph';
import { nodeTypes } from './nodes';
import { HeaderBar } from './components/HeaderBar';
import { BottomPanel } from './components/BottomPanel';
import { SensorSidebar } from './components/SensorSidebar';
import { Sidebar } from './components/Sidebar';

/**
 * Application shell. All domain logic lives in dedicated hooks:
 * - useRobotConnection — WebSocket telemetry (logs, progress, active step)
 * - useProfiles       — HAL robot profile list + switching
 * - useExecution      — graph compilation, run/abort, error state
 * - useCanvasGraph    — React Flow state, DnD, connection rules, highlights
 * - usePanelLayout    — panel visibility + splitter resizing
 */
function FlowEditor() {
  const { isConnected, logs, progress, activeStepIndex } = useRobotConnection();
  const { profiles, activeProfileId, activeProfile, selectProfile } = useProfiles();
  const layout = usePanelLayout();
  const execution = useExecution();

  // Map the 1-indexed STEP event to the canvas node via the compiled order
  const executingNodeId = useMemo(() => {
    if (activeStepIndex === null) return null;
    return execution.executionOrder[activeStepIndex - 1] ?? null;
  }, [activeStepIndex, execution.executionOrder]);

  const canvas = useCanvasGraph({
    executingNodeId,
    errorNodeIds: execution.errorNodeIds,
    activeProfile,
    onGraphEdit: execution.clearCompileError,
  });

  return (
    <div className="w-full h-screen flex flex-col bg-background text-slate-300 font-sans overflow-hidden">

      <HeaderBar
        isConnected={isConnected}
        isRunning={execution.isRunning}
        onRun={() => execution.execute(canvas.nodes, canvas.edges)}
        onAbort={execution.abort}
        profiles={profiles}
        activeProfileId={activeProfileId}
        onProfileChange={selectProfile}
        showLeftSidebar={layout.showLeftSidebar}
        showBottomPanel={layout.showBottomPanel}
        showRightSidebar={layout.showRightSidebar}
        onToggleLeftSidebar={layout.toggleLeftSidebar}
        onToggleBottomPanel={layout.toggleBottomPanel}
        onToggleRightSidebar={layout.toggleRightSidebar}
      />

      {/* ── Main Layout ───────────────────────────────────────────── */}
      <div className="flex-1 flex min-h-0">

        {/* Left Sidebar — Block Palette (filtered by control mode) */}
        {layout.showLeftSidebar && <Sidebar controlMode={activeProfile?.control_mode ?? null} />}

        {/* Center Column: Canvas + Bottom Panel */}
        <div className="flex-1 flex flex-col min-h-0 min-w-0">

          {/* ReactFlow Canvas */}
          <div
            className="flex-1 bg-background relative bg-cyber-grid min-h-0"
            onDrop={canvas.onDrop}
            onDragOver={canvas.onDragOver}
          >
            <ReactFlow
              nodes={canvas.nodes}
              edges={canvas.edges}
              onNodesChange={canvas.onNodesChange}
              onEdgesChange={canvas.onEdgesChange}
              onConnect={canvas.onConnect}
              isValidConnection={canvas.isValidConnection}
              nodeTypes={nodeTypes}
              fitView
              className="bg-transparent"
              colorMode="dark"
            >
              <Background gap={20} size={1} color="rgba(255,255,255,0.05)" />
              <Controls className="!bg-surface !border-white/10 !text-slate-400 [&>button]:!fill-slate-400" />
            </ReactFlow>

            {execution.compileError && (
              <div className="absolute top-4 left-1/2 -translate-x-1/2 max-w-[80%] glass-panel !border-red-500/50 px-4 py-2.5 flex items-center gap-3 z-50 animate-fade-in">
                <TriangleAlert size={14} className="text-red-400 shrink-0" />
                <span className="text-xs font-mono text-red-300">{execution.compileError}</span>
                <button
                  onClick={execution.clearCompileError}
                  className="text-slate-500 hover:text-slate-300 transition-colors shrink-0"
                  title="Dismiss"
                >
                  <X size={12} />
                </button>
              </div>
            )}

            {progress !== null && (
              <div className="absolute bottom-4 left-1/2 -translate-x-1/2 w-80 glass-panel p-3 z-50 animate-fade-in">
                <div className="h-1.5 bg-surface_light rounded-full overflow-hidden">
                  <div
                    className="h-full bg-primary_glow shadow-neon-primary transition-all duration-300 ease-out"
                    style={{ width: `${progress}%` }}
                  />
                </div>
              </div>
            )}
          </div>

          {/* Bottom Panel Splitter */}
          {layout.showBottomPanel && (
            <div
              onMouseDown={layout.startBottomResize}
              className="h-1 shrink-0 bg-surface cursor-row-resize hover:bg-primary/30 active:bg-primary_glow/50 transition-colors border-y border-white/5"
            />
          )}

          {/* Bottom Panel — Terminal + ROS Info */}
          {layout.showBottomPanel && (
            <BottomPanel logs={logs} height={layout.bottomHeight} />
          )}
        </div>

        {/* Right Sidebar Splitter */}
        {layout.showRightSidebar && (
          <div
            onMouseDown={layout.startRightResize}
            className="w-1 shrink-0 bg-surface cursor-col-resize hover:bg-primary/30 active:bg-primary_glow/50 transition-colors border-x border-white/5"
          />
        )}

        {/* Right Sidebar — Sensors */}
        {layout.showRightSidebar && (
          <SensorSidebar width={layout.rightWidth} />
        )}
      </div>
    </div>
  );
}

// ─── Root Wrapper ───────────────────────────────────────────────────────────

export default function AppWrapper() {
  return (
    <ReactFlowProvider>
      <FlowEditor />
    </ReactFlowProvider>
  );
}
