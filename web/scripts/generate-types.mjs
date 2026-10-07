import { readdir, readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { compile } from 'json-schema-to-typescript';

// v1 types live at the top level; later versions only generate the types they add or change.
const generatedFromLater = { v2: ['EnsembleResult.schema.json'], v3: ['WeeklyCaseCount.schema.json'], v4: ['ScenarioProvenance.schema.json'], v5: ['ForecastProvenance.schema.json'] };
for (const version of ['v1', 'v2', 'v3', 'v4', 'v5']) {
  const schemaDir = new URL(`../../crates/koplik-contracts/schema/${version}/`, import.meta.url);
  const outputDir = new URL(`../src/generated/${version === 'v1' ? '' : `${version}/`}`, import.meta.url);
  await mkdir(outputDir, { recursive: true });
  for (const file of (await readdir(schemaDir)).filter((name) => version === 'v1' ? name.endsWith('.json') : generatedFromLater[version].includes(name)).sort()) {
    const schema = JSON.parse(await readFile(new URL(file, schemaDir), 'utf8'));
    const text = await compile(schema, schema.title, {
      bannerComment: `/* Generated from koplik-contracts schema/${version}. Run npm run generate:types. */`,
      cwd: fileURLToPath(schemaDir),
      additionalProperties: false,
    });
    const target = new URL(`${schema.title}.ts`, outputDir);
    if (process.argv.includes('--check')) {
      if ((await readFile(target, 'utf8')) !== text) {
        throw new Error(`${schema.title} types have drifted; run npm run generate:types`);
      }
    } else {
      await writeFile(target, text);
    }
  }
}
