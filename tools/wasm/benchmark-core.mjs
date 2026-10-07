// Shared by Node and the browser. Times the public facade, including JSON work.
export function benchmark(runEnsemble, scenarioJson, clock = performance) {
  const input = JSON.parse(scenarioJson);
  if (input.run_count !== 1000) throw new Error('benchmark requires exactly 1,000 members');
  function sample() {
    const start = clock.now();
    const output = JSON.parse(runEnsemble(scenarioJson));
    const elapsed_ms = clock.now() - start;
    if (output.members.length !== 1000) throw new Error('incomplete ensemble');
    return { elapsed_ms, fingerprint: output.fingerprint, output_json_bytes: JSON.stringify(output).length };
  }
  const cold = sample(); // First call after module initialization, not in warm median.
  const warm = Array.from({ length: 5 }, sample);
  if (warm.some(s => s.fingerprint !== cold.fingerprint)) throw new Error('benchmark is nondeterministic');
  const sorted = warm.map(s => s.elapsed_ms).sort((a, b) => a - b);
  return {
    synthetic: true,
    fixture: 'data/fixtures/seir/synthetic-scenario.json',
    runs: input.run_count,
    counties: input.nodes.length,
    horizon_days: input.parameters.horizon_days,
    time_step_days: input.parameters.time_step_days,
    seed: String(input.seed),
    measured_scope: 'WASM facade: input JSON parse, engine, summaries, all member hashes, result JSON serialize + JS parse; excludes module load',
    cold, warm,
    warm_median_ms: sorted[2],
    target_ms: 3000,
    warm_median_meets_target: sorted[2] < 3000,
  };
}
