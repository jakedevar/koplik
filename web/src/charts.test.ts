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
    expect(zero.querySelector('.case-bar title')?.textContent).toContain('0 confirmed cases');
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
});
