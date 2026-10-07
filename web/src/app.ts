import { caseChart, rtChart, rtLabel } from './charts';
import { compareWeeks, metricLabels, metricValue, type Dataset, type Metric } from './data';
import { createMap, type MapView } from './map';

export const disclaimer = 'Demonstration project; not medical or public-health advice; not affiliated with CDC or WHO.';
function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}
export function shell(root: HTMLElement): HTMLElement {
  root.replaceChildren();
  const header = element('header', undefined, 'site-header');
  const brand = element('a', 'koplik', 'brand');
  brand.href = import.meta.env.BASE_URL;
  header.append(brand, element('span', 'Measles outbreak intelligence', 'tagline'));
  const main = element('main');
  const footer = element('footer', disclaimer, 'disclaimer');
  root.append(header, main, footer);
  return main;
}
export function showStatus(root: HTMLElement, message: string, error = false) {
  const main = shell(root);
  const status = element('p', message, 'notice');
  status.setAttribute('role', error ? 'alert' : 'status');
  main.append(element('h1', 'Measles across the United States'), status);
}

export type MapFactory = (container: HTMLElement, data: Dataset, onSelect: (id: string) => void, onError: () => void) => MapView;

export function mountDashboard(root: HTMLElement, data: Dataset, mapFactory: MapFactory = createMap): () => void {
  const main = shell(root);
  if (data.synthetic) main.append(element('p', 'SYNTHETIC TEST DATA · Invented values and simplified geometry for development only. These are not observed measles reports.', 'synthetic notice'));
  main.append(element('p', 'SURVEILLANCE EXPLORER', 'eyebrow'), element('h1', 'Measles across the United States'),
    element('p', 'Explore reported cases, vaccination coverage and the pace of an outbreak. Missing reports stay missing.', 'intro'));

  let level: 'state' | 'county' = 'state';
  let metric: Metric = 'cases-2025';
  let selected = data.geographies.find((g) => g.id === '48')?.id || data.geographies.find((g) => g.level === 'state')?.id || '';
  const grid = element('div', undefined, 'dashboard-grid');
  const mapPanel = element('section', undefined, 'panel map-panel');
  mapPanel.setAttribute('aria-label', 'Geographic explorer');
  const toolbar = element('div', undefined, 'toolbar');
  const metricLabel = element('label', 'Map measure');
  const metricSelect = element('select');
  metricSelect.id = 'metric';
  metricLabel.htmlFor = metricSelect.id;
  for (const [value, label] of Object.entries(metricLabels)) {
    const option = element('option', label);
    option.value = value;
    metricSelect.append(option);
  }
  const drill = element('button', 'Explore Texas counties →', 'drill-button');
  drill.type = 'button';
  toolbar.append(metricLabel, metricSelect, drill);
  const mapNode = element('div', undefined, 'map');
  mapNode.setAttribute('role', 'region');
  mapNode.setAttribute('aria-label', 'Choropleth map. Use the geography selector below as a keyboard alternative.');
  const mapStatus = element('p', '', 'map-status');
  mapStatus.setAttribute('role', 'status');
  const legend = element('div', undefined, 'legend');
  const geographyLabel = element('label', 'Select a state');
  const geographySelect = element('select');
  geographySelect.id = 'geography';
  geographyLabel.htmlFor = geographySelect.id;
  const chooser = element('div', undefined, 'geography-chooser');
  chooser.append(geographyLabel, geographySelect);
  mapPanel.append(toolbar, mapNode, mapStatus, legend, chooser);
  const detail = element('section', undefined, 'panel detail-panel');
  detail.setAttribute('aria-label', 'Selected geography');
  const name = element('h2');
  const measure = element('p', undefined, 'eyebrow');
  const value = element('p', undefined, 'headline-value');
  const valueDetail = element('p', undefined, 'value-detail');
  const summary = element('div');
  summary.setAttribute('aria-live', 'polite');
  summary.setAttribute('aria-atomic', 'true');
  summary.append(name, measure, value, valueDetail);
  const yearLabel = element('label', 'Chart year (MMWR)');
  const yearSelect = element('select');
  yearSelect.id = 'chart-year';
  yearLabel.htmlFor = yearSelect.id;
  for (const year of ['2025', '2026']) {
    const option = element('option', year);
    option.value = year;
    yearSelect.append(option);
  }
  const chartControls = element('div', undefined, 'chart-controls');
  chartControls.append(yearLabel, yearSelect);
  const charts = element('div', undefined, 'charts');
  detail.append(summary, chartControls, charts);
  grid.append(mapPanel, detail);
  main.append(grid);
  const reports = element('details', undefined, 'panel reports');
  reports.append(element('summary', 'Read exact weekly reports and R_t status'));
  const tableContainer = element('div', undefined, 'table-scroll');
  reports.append(tableContainer);
  main.append(reports);
  let map: MapView | undefined;
  function available() {
    return data.geographies.filter((g) => g.level === level && (level !== 'county' || g.id.startsWith('48')))
      .sort((a, b) => a.name.localeCompare(b.name, 'en'));
  }
  function chooseOptions() {
    geographySelect.replaceChildren();
    const geographies = available();
    if (!geographies.some((g) => g.id === selected)) selected = geographies[0]?.id || '';
    for (const geography of geographies) {
      const option = element('option', `${geography.name} · ${metricValue(data, geography.id, metric).label}`);
      option.value = geography.id;
      geographySelect.append(option);
    }
    geographySelect.value = selected;
    geographySelect.disabled = geographies.length === 0;
  }
  function renderReports(year: number) {
    const cases = data.cases.filter((r) => r.geography === selected && r.week.year === year).sort(compareWeeks);
    const rt = data.rt.filter((r) => r.geography === selected && r.week.year === year).sort(compareWeeks);
    charts.replaceChildren(element('h3', 'Weekly confirmed cases'), caseChart(cases, year),
      element('p', 'Gaps mean no data. Bars show new cases reported in each MMWR week.', 'chart-note'),
      element('h3', 'Effective reproduction number · R_t'), rtChart(rt, year),
      element('p', 'Line: mean · Ribbon: credible interval · Dashed line: R_t = 1. Provisional estimates are withheld; insufficient data has no estimate. Exact interval levels appear in the report table.', 'chart-note'));
    if (!cases.length) charts.prepend(element('p', 'No case data for this geography and year.', 'notice'));
    if (!rt.some((r) => r.status === 'ok' && !r.provisional)) charts.append(element('p', 'No final R_t estimate for this geography and year.', 'notice'));
    const table = element('table');
    table.append(element('caption', `${name.textContent} · MMWR ${year}`));
    const head = element('thead');
    const headers = element('tr');
    for (const text of ['MMWR week', 'New confirmed cases', 'R_t / quality']) {
      const cell = element('th', text);
      cell.scope = 'col';
      headers.append(cell);
    }
    head.append(headers);
    const body = element('tbody');
    const weeks = [...new Set([...cases, ...rt].map((r) => r.week.week))].sort((a, b) => a - b);
    // Show omitted weeks inside a reported period as no data as well as explicit missing rows.
    if (weeks.length) for (let week = weeks[0]; week <= weeks.at(-1)!; week++) {
      const c = cases.find((r) => r.week.week === week);
      const r = rt.find((row) => row.week.week === week);
      const row = element('tr');
      row.dataset.geography = selected;
      row.dataset.week = `${year}-${week}`;
      const weekCell = element('th', `W${week}`);
      weekCell.scope = 'row';
      const caseCell = element('td', c?.confirmed.status === 'reported' ? String(c.confirmed.count) : `No data${c?.confirmed.status === 'missing' ? ` · ${c.confirmed.reason}` : ''}`);
      const rtCell = element('td', r ? rtLabel(r) : 'No data');
      if (r?.provisional) rtCell.className = 'provisional';
      else if (r?.status === 'insufficient_data') rtCell.className = 'insufficient';
      row.append(weekCell, caseCell, rtCell);
      body.append(row);
    }
    table.append(head, body);
    tableContainer.replaceChildren(weeks.length ? table : element('p', 'No weekly reports available.', 'notice'));
  }
  function render() {
    geographySelect.value = selected;
    const geography = data.geographies.find((g) => g.id === selected);
    name.textContent = geography?.name || 'No geography data';
    measure.textContent = level === 'county' ? 'Texas outbreak · Cases 2025' : metricLabels[metric];
    const currentValue = metricValue(data, selected, metric);
    value.textContent = currentValue.label;
    value.classList.toggle('missing-value', currentValue.value === null);
    valueDetail.textContent = currentValue.detail;
    geographyLabel.textContent = level === 'state' ? 'Select a state (keyboard accessible)' : 'Select a Texas county (keyboard accessible)';
    drill.textContent = level === 'state' ? 'Explore Texas counties →' : '← United States';
    metricSelect.disabled = level === 'county';
    yearSelect.disabled = level === 'county';
    legend.replaceChildren();
    const isCoverage = metric === 'coverage';
    legend.append(element('span', isCoverage ? 'MMR coverage · 0–100%' : 'Reported cases · 0 → 500+', 'legend-scale'), element('span', '▨ No data', 'legend-missing'));
    renderReports(Number(yearSelect.value));
    map?.update(level, metric, selected);
    root.dispatchEvent(new CustomEvent('koplik:selection', { bubbles: true, detail: { geography: selected, metric, year: Number(yearSelect.value), data } }));
  }
  metricSelect.addEventListener('change', () => {
    metric = metricSelect.value as Metric;
    if (metric !== 'coverage') yearSelect.value = metric === 'cases-2025' ? '2025' : '2026';
    chooseOptions(); render();
  });
  geographySelect.addEventListener('change', () => { selected = geographySelect.value; render(); });
  yearSelect.addEventListener('change', render);
  drill.addEventListener('click', () => {
    level = level === 'state' ? 'county' : 'state';
    if (level === 'county') {
      metric = 'cases-2025'; metricSelect.value = metric; yearSelect.value = '2025';
      selected = available().find((g) => g.id === '48165')?.id || available()[0]?.id || '';
    } else selected = '48';
    chooseOptions(); render(); geographySelect.focus();
  });
  chooseOptions();
  render();
  try {
    map = mapFactory(mapNode, data, (id) => {
      if (available().some((g) => g.id === id)) { selected = id; render(); }
    }, () => { mapStatus.textContent = 'Map unavailable. Select a geography below to explore all reports.'; });
    map.update(level, metric, selected);
  } catch {
    mapStatus.textContent = 'Map unavailable. Select a geography below to explore all reports.';
  }
  return () => map?.destroy();
}
