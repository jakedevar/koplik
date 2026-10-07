import Ajv from 'ajv/dist/2020';
import addFormats from 'ajv-formats';
import geography from '../../crates/koplik-contracts/schema/v6/GeographyArtifact.schema.json';
import cases from '../../crates/koplik-contracts/schema/v6/WeeklyCaseCountArtifact.schema.json';
import coverage from '../../crates/koplik-contracts/schema/v6/KindergartenMmrCoverageArtifact.schema.json';
import rt from '../../crates/koplik-contracts/schema/v6/RtEstimateArtifact.schema.json';
import forecast from '../../crates/koplik-contracts/schema/v6/ForecastArtifact.schema.json';
import type { GeographyArtifact } from './generated/v6/GeographyArtifact';
import type { WeeklyCaseCountArtifact } from './generated/v6/WeeklyCaseCountArtifact';
import type { KindergartenMmrCoverageArtifact } from './generated/v6/KindergartenMmrCoverageArtifact';
import type { RtEstimateArtifact } from './generated/v6/RtEstimateArtifact';
import type { ForecastArtifact } from './generated/v6/ForecastArtifact';

type Artifact = GeographyArtifact | WeeklyCaseCountArtifact | KindergartenMmrCoverageArtifact | RtEstimateArtifact | ForecastArtifact;
const ajv = new Ajv({ strict: false, allErrors: true });
addFormats(ajv);
for (const [format, maximum] of [['uint8', 255], ['uint16', 65535], ['uint32', 4294967295], ['uint64', 18446744073709551615]] as const) {
  ajv.addFormat(format, { type: 'number', validate: (value: number) => Number.isInteger(value) && value >= 0 && value <= maximum });
}
ajv.addFormat('double', { type: 'number', validate: Number.isFinite });
const validators = {
  geographies: ajv.compile(geography), cases: ajv.compile(cases), coverage: ajv.compile(coverage),
  rt: ajv.compile(rt), forecast: ajv.compile(forecast),
};

/** Expand v6 before the released row validators and UI consume it. Legacy arrays
 * remain accepted for development fixtures and explicit lossless upgrades. */
export function expandRowArtifact(kind: keyof typeof validators, input: unknown): unknown {
  if (Array.isArray(input)) return input;
  const validate = validators[kind];
  if (!validate(input)) throw new Error(`${kind}: invalid v6 artifact: ${ajv.errorsText(validate.errors)}`);
  const artifact = input as unknown as Artifact;
  return artifact.rows.map((row) => ({ ...row, provenance: row.provenance.map((index) => {
    const record = artifact.provenance[index];
    if (!record) throw new Error(`${kind}: provenance index ${index} outside this file's table`);
    return record;
  }) }));
}
