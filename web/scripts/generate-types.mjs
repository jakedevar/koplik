import { readdir, readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { compile } from 'json-schema-to-typescript';

for (const version of ['v1', 'v2']) {
  const schemaDir = new URL(`../../crates/koplik-contracts/schema/${version}/`, import.meta.url);
  const outputDir = new URL(`../src/generated/${version === 'v1' ? '' : 'v2/'}`, import.meta.url);
  await mkdir(outputDir, { recursive: true });
  for (const file of (await readdir(schemaDir)).filter((name) => version === 'v1' ? name.endsWith('.json') : name === 'EnsembleResult.schema.json').sort()) {
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
