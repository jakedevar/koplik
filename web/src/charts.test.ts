import { describe, expect, it } from 'vitest';
import { caseChart, rtChart, rtLabel } from './charts';
import { fixtureDataset } from './fixtures.test-utils';

describe('accessible SVG reports', () => {
  it('renders reported zero and leaves missing weeks as gaps', () => {
    const rows = fixtureDataset().cases.filter((r) => r.geography === '35');
    const svg = caseChart(rows, 2025);
    expect([...svg.querySelectorAll('.case-bar')].map((bar) => bar.getAttribute('data-week'))).toEqual(['1', '3']);
    expect(svg.getAttribute('role')).toBe('img');
    expect(svg.querySelector('title')?.textContent).toContain('MMWR 2025');
    const zero = caseChart(fixtureDataset().cases.filter((r) => r.geography === '40'), 2025);
    expect(zero.querySelector('.case-zero title')?.textContent).toContain('0 confirmed cases');
    const comparison = caseChart([{ ...rows[0], confirmed: { status: 'reported', count: 0 } }, rows[1]], 2025);
    expect(comparison.querySelector('.case-zero')?.tagName).toBe('line');
    expect(comparison.querySelector('.case-zero')?.getAttribute('y1')).toBe('170');
    expect(comparison.querySelectorAll('[data-week="1"]')).toHaveLength(1);
    expect(comparison.querySelectorAll('[data-week="2"]')).toHaveLength(0);
    expect(svg.querySelectorAll('[data-week="2"]')).toHaveLength(0);
  });
  it('draws only final estimates and breaks ribbons over insufficient/provisional/omitted weeks', () => {
    const rows = fixtureDataset().rt.filter((r) => r.geography === '48');
    const svg = rtChart(rows, 2025);
    expect([...svg.querySelectorAll('.rt-point')].map((point) => point.getAttribute('data-week'))).toEqual(['2', '3', '5']);
    expect(svg.querySelectorAll('.rt-ribbon')).toHaveLength(2);
    expect(svg.querySelector('.rt-point title')?.textContent).toContain('Mean 1.2; 90% interval 0.7–1.8');
    expect(rtLabel(rows[0])).toBe('Insufficient data');
    expect(rtLabel(rows[5])).toContain('Provisional — estimate withheld');
    expect(rtChart([rows[1], rows[4]], 2025).querySelectorAll('.rt-ribbon')).toHaveLength(2);
  });
  it('marks every withheld week with distinct visible and accessible quality states', () => {
    const rows = fixtureDataset().rt.filter((r) => r.geography === '48');
    const both = { ...rows[0], week: { year: 2025, week: 7 }, provisional: true };
    const svg = rtChart([...rows, both], 2025);
    expect([...svg.querySelectorAll('.rt-withheld')].map((marker) => marker.getAttribute('data-week'))).toEqual(['1', '4', '6', '7']);
    expect(svg.querySelector('[data-week="1"] .rt-insufficient-band')?.getAttribute('fill')).toMatch(/^url\(#rt-insufficient-hatch-/);
    expect(svg.querySelector('[data-week="6"] .rt-provisional-band')?.tagName).toBe('rect');
    expect(svg.querySelector('[data-week="7"] text')?.textContent).toBe('IP');
    expect(svg.querySelector('[data-week="7"]')?.getAttribute('aria-label')).toContain('Insufficient data · Provisional');
    expect(svg.querySelectorAll('[data-week="8"]')).toHaveLength(0);
    expect(svg.querySelector('.rt-status-legend')?.textContent).toContain('Blank: no row');
    expect(svg.querySelector('[data-week="1"] title')?.textContent).toBe('Week 1: Insufficient data');
    const description = svg.querySelector(`[id="${svg.getAttribute('aria-describedby')}"]`);
    expect(description?.textContent).toContain('Week 1: Insufficient data');
    expect(description?.textContent).toContain('Week 6: Estimate available · Provisional — estimate withheld');
    expect(svg.querySelectorAll('[data-week="1"] .rt-point, [data-week="6"] .rt-point, [data-week="7"] .rt-point')).toHaveLength(0);
  });
});
