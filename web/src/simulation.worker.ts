import init, { runEnsemble } from '../../pkg/web/koplik_wasm.js';
import type { SimulationRequest, SimulationResponse } from './simulation';

const ready = init();
const scope = self as unknown as {
  onmessage: (event: MessageEvent<SimulationRequest>) => void;
  postMessage: (response: SimulationResponse) => void;
};
scope.onmessage = async ({ data }) => {
  try {
    await ready;
    const result = JSON.parse(runEnsemble(data.scenarioJson));
    scope.postMessage({ id: data.id, result });
  } catch (error) {
    scope.postMessage({ id: data.id, error: error instanceof Error ? error.message : String(error) });
  }
};
