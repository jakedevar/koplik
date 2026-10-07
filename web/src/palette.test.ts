import { readFileSync } from 'node:fs';
import { caseScales, coverageScale } from './map-scales';
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

// Colour-blind checks. Each colour is simulated for protanopia, deuteranopia and tritanopia (Machado, Oliveira and Fernandes 2009,
// severity 1.0, applied in linear sRGB), converted to CIELAB, and compared by CIE76 ΔE. A ΔE of about 10 or more is an easy
// difference; the thresholds below are deliberately above the just-noticeable ~2.3. Ordered scales must also keep strictly
// falling lightness (L*) under every simulation, so their order survives without hue.
const machado = {
  normal: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
  protanopia: [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]],
  deuteranopia: [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.01182, 0.04294, 0.968881]],
  tritanopia: [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.3039]],
} as const;
type Vision = keyof typeof machado;
const visions = Object.keys(machado) as Vision[];
const toLinear = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
function lab(hex: string, vision: Vision): [number, number, number] {
  const rgb = [1, 3, 5].map((i) => toLinear(parseInt(hex.slice(i, i + 2), 16) / 255));
  const [r, g, b] = machado[vision].map((row) => Math.min(1, Math.max(0, row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2])));
  const f = (t: number) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  const x = f((0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047), y = f(0.2126 * r + 0.7152 * g + 0.0722 * b), z = f((0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883);
  return [116 * y - 16, 500 * (x - y), 200 * (y - z)];
}
const deltaE = (a: string, b: string, vision: Vision) => Math.hypot(...lab(a, vision).map((v, i) => v - lab(b, vision)[i]));

describe('data-encoding colours', () => {
  const scales: [string, [number, string][]][] = [
    ['confirmed or unknown-status cases (map)', caseScales.confirmed_or_unknown_status], ['confirmed cases (map)', caseScales.confirmed], ['MMR coverage (map)', coverageScale]];
  describe.each(scales)('%s scale', (_name, stops) => {
    const colours = stops.map(([, colour]) => colour);
    it.each(visions)('keeps its break points in order of falling lightness, %s', (vision) => {
      const lightness = colours.map((c) => lab(c, vision)[0]);
      lightness.slice(1).forEach((l, i) => expect(l, `stop ${i + 1}`).toBeLessThan(lightness[i] - 8));
    });
    it.each(visions)('keeps neighbouring break points distinguishable, %s', (vision) => {
      colours.slice(1).forEach((c, i) => expect(deltaE(colours[i], c, vision), `stops ${i}-${i + 1}`).toBeGreaterThanOrEqual(10));
    });
  });
  it('keeps the scale breaks of each map scale as published', () => {
    expect(caseScales.confirmed_or_unknown_status.map(([v]) => v)).toEqual([0, 1, 50, 100, 500]);
    expect(caseScales.confirmed.map(([v]) => v)).toEqual([0, 1, 50, 100, 500]);
    expect(coverageScale.map(([v]) => v)).toEqual([0, 80, 90, 95, 100]);
  });
  const marks: [string, string, number][] = [
    ['chart-bar', 'surface', 3], ['chart-line', 'surface', 3], ['chart-line', 'band-50', 3], ['chart-missing', 'surface', 4.5], ['chart-provisional', 'surface', 3],
    ['chart-reference', 'surface', 3], ['chart-hatch', 'chart-hatch-bg', 3], ['chart-interval', 'surface', 2],
  ];
  it.each(marks)('draws %s on %s with a contrast of at least %s : 1', (fg, bg, minimum) => {
    expect(root[fg], fg).toBeDefined(); expect(root[bg], bg).toBeDefined();
    expect(contrast(root[fg], root[bg])).toBeGreaterThanOrEqual(minimum);
  });
  it.each(visions)('tells the forecast median, the 50% band, the 90% band and the page apart, %s', (vision) => {
    expect(deltaE(root['band-90'], root['band-50'], vision)).toBeGreaterThanOrEqual(12);
    expect(deltaE(root['band-50'], root['chart-line'], vision)).toBeGreaterThanOrEqual(25);
    expect(deltaE(root['band-90'], root.surface, vision)).toBeGreaterThanOrEqual(6);
  });
  it.each(visions)('keeps the selected outline apart from both ends of every map scale, %s', (vision) => {
    for (const [, stops] of scales) for (const end of [stops[0][1], stops.at(-1)![1]]) expect(deltaE(root.focus, end, vision)).toBeGreaterThanOrEqual(30);
  });
});

describe('interface tokens added for the polish pass', () => {
  const text: [string, string][] = [
    ['notice-info-ink', 'notice-info-bg'], ['notice-caution-ink', 'notice-caution-bg'], ['notice-neutral-ink', 'notice-neutral-bg'],
    ['notice-measured-ink', 'notice-measured-bg'], ['notice-synthetic-ink', 'notice-synthetic-bg'], ['link-on-brand', 'brand'], ['ink', 'surface'],
  ];
  it.each(text)('%s on %s meets WCAG AA for text', (fg, bg) => {
    expect(root[fg], fg).toBeDefined(); expect(root[bg], bg).toBeDefined();
    expect(contrast(root[fg], root[bg])).toBeGreaterThanOrEqual(4.5);
  });
  const graphics: [string, string][] = [
    ['field-border', 'surface'], ['focus', 'surface'], ['focus', 'page'], ['notice-info-rule', 'notice-info-bg'], ['notice-caution-rule', 'notice-caution-bg'],
    ['notice-neutral-rule', 'notice-neutral-bg'], ['notice-measured-rule', 'notice-measured-bg'], ['notice-synthetic-rule', 'notice-synthetic-bg'],
  ];
  it.each(graphics)('%s on %s meets the 3 : 1 non-text contrast', (fg, bg) => {
    expect(contrast(root[fg], root[bg])).toBeGreaterThanOrEqual(3);
  });
  it('tells the notice kinds apart without colour: each is a distinct left rule colour under every vision', () => {
    const rules = ['notice-info-rule', 'notice-caution-rule', 'notice-neutral-rule', 'notice-measured-rule'];
    // Colour is never the only signal (each notice says in words what it is); the rules still differ in lightness or hue.
    for (const vision of visions) for (const [i, a] of rules.entries()) for (const b of rules.slice(i + 1)) {
      expect(deltaE(root[a], root[b], vision), `${a} / ${b} (${vision})`).toBeGreaterThanOrEqual(8);
    }
  });
  it('defines the type scale and the 4 / 8 px spacing scale as custom properties', () => {
    const block = css.slice(css.indexOf(':root'), css.indexOf('}', css.indexOf(':root')));
    for (const name of ['text-xs', 'text-sm', 'text-md', 'text-lg', 'text-xl', 'text-2xl', 'text-title', 'text-stat']) expect(block, name).toContain(`--${name}:`);
    const spaces = [...block.matchAll(/--space-\d:\s*(\d+)px/g)].map((m) => Number(m[1]));
    expect(spaces.length).toBeGreaterThanOrEqual(6);
    for (const px of spaces) expect(px % 4).toBe(0);
  });
});
