import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

// Reads the interface palette from style.css, so the contrast of the pairs below is checked against what ships.
const css = readFileSync(resolve(process.cwd(), 'src/style.css'), 'utf8');
function variables(block: string): Record<string, string> {
  return Object.fromEntries([...block.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/gi)].map((m) => [m[1], m[2]]));
}
const root = variables(css.slice(css.indexOf(':root'), css.indexOf('}', css.indexOf(':root'))));
const accents = Object.fromEntries([...css.matchAll(/\.page-([\w-]+)\s*\{([^}]*)\}/g)].map((m) => [m[1], variables(m[2])]));

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe('interface palette', () => {
  const pairs: [string, string, string][] = [
    ['ink', 'page', 'body text'], ['ink', 'surface', 'panel text'], ['ink-muted', 'page', 'intro text'], ['ink-muted', 'surface', 'muted text'],
    ['link', 'surface', 'links on panels'], ['link', 'page', 'links on the page'],
    ['brand-ink', 'brand', 'brand and nav text on the band'], ['brand-ink-muted', 'brand', 'tagline and footer note on the band'],
    ['brand-ink', 'brand-hover', 'hovered nav link'], ['highlight-ink', 'highlight', 'active nav link'], ['highlight', 'brand', 'focus ring on the band'],
    ['button-ink', 'button', 'filled button'],
  ];
  it.each(pairs)('%s on %s meets WCAG AA for text (%s)', (fg, bg) => {
    expect(root[fg], fg).toBeDefined(); expect(root[bg], bg).toBeDefined();
    expect(contrast(root[fg], root[bg])).toBeGreaterThanOrEqual(4.5);
  });
  it('keeps every per-page accent legible as eyebrow text on the page background and its tint', () => {
    const all = { explorer: { accent: root.accent, 'accent-tint': root['accent-tint'] }, ...accents };
    expect(Object.keys(all).sort()).toEqual(['explorer', 'forecast', 'sources', 'what-if']);
    for (const [name, v] of Object.entries(all)) {
      expect(contrast(v.accent, root.page), name).toBeGreaterThanOrEqual(4.5);
      expect(contrast(v.accent, v['accent-tint']), name).toBeGreaterThanOrEqual(4.5);
    }
  });
});
