import { readFileSync } from 'node:fs';

const lock = readFileSync(new URL('../../Cargo.lock', import.meta.url), 'utf8');
const packages = lock.split('[[package]]').filter(p => /^name = "wasm-bindgen"$/m.test(p));
if (packages.length !== 1) throw new Error('Cargo.lock must contain exactly one wasm-bindgen version');
const version = packages[0].match(/^version = "([0-9]+\.[0-9]+\.[0-9]+)"$/m)?.[1];
if (!version) throw new Error('Cannot read wasm-bindgen version from Cargo.lock');
console.log(version);
