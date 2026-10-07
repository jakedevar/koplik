import type { EnsembleResult } from './generated/v2/EnsembleResult';

// Local Worker messages, not published data contracts.
export interface SimulationRequest { id: number; scenarioJson: string }
export type SimulationResponse = { id: number; result: EnsembleResult } | { id: number; error: string };
export interface SimulationWorker {
  onmessage: ((event: MessageEvent<SimulationResponse>) => void) | null;
  onerror: ((event: ErrorEvent) => void) | null;
  postMessage(request: SimulationRequest): void;
  terminate(): void;
}
export function createSimulationWorker(): SimulationWorker {
  return new Worker(new URL('./simulation.worker.ts', import.meta.url), { type: 'module' });
}
