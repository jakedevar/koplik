import { caseCharts, caseSeries, rtChart, rtLabel } from './charts';
import { caseDefinitionLabels, caseDefinitionWords, caseDefinitionsAt, compareWeeks, defaultCaseDefinition, metricLabels, metricValue, missingReasonWords, otherDefinitionGeographies, type CaseDefinition, type Dataset, type Metric } from './data';
import { cumulativeSection, cumulativeSeries, cumulativeTable } from './cumulative';
import { attributionSection } from './attribution-view';
import { explorerSummary } from './summary';
import { summarySection } from './summary-view';
import { footerSources } from './attribution';
import { mountRouter, pageEvent, pageHash, pageIds, pageTitles, type PageId } from './router';
import { caseScales, coverageScale, type MapView } from './map-scales';
import { createLazyMap } from './map-lazy';
import { mountProvenanceDrawer, provenanceNumber } from './provenance';

/** Said above a Texas county's weekly chart: why most weeks are "No data", and that nothing fills them. */
export const countyWeeklyNote = 'Weekly counts for Texas counties are derived only for a week whose last DSHS report has a county table, when the previous week’s last report has one too: the week’s new cases are the difference between the two printed cumulative counts. Every other week is No data, with its reason: “not reported” means DSHS published no county table that week (or this is the first table, whose cumulative cannot be assigned to one week); “ambiguous” means a report exists but this week’s count cannot be separated from earlier weeks (after a gap, or in a week whose last report had no county table). Nothing is estimated or interpolated. The cumulative chart above shows what each DSHS report printed.';

export const disclaimer = 'Demonstration project; not medical or public-health advice; not affiliated with CDC or WHO.';
function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}
/** What the Sources page says about how every number got here (the rules in AGENTS.md and the spec). */
export const methodNote = [
  'Every reported number traces back to a snapshot of a source: the snapshot’s SHA-256 hash, the source URL and the time it was retrieved. Click or press a number to open its record. Raw source snapshots are never edited.',
  'Missing or ambiguous data is shown as missing, never guessed or filled in. R_t and the forecast are derived from the reported counts by the code in this project, with every model parameter cited; below the minimum-count threshold we publish “insufficient data” rather than an estimate. A forecast is a model projection, not a prediction. The what-if is a hypothetical introduction of one infectious person, driven by population and coverage data, cited parameters and stated assumptions, not by reported counts.',
];

export interface Shell {
  main: HTMLElement;
  /** One container per page, in navbar order; the Explorer's h1 is the dashboard's own. */
  pages: Record<PageId, HTMLElement>;
}
const pageIntros: Record<Exclude<PageId, 'explorer'>, [string, string, string]> = {
  forecast: ['FORECAST', 'Where is measles going next?', 'A model projection from reported counts, with its measured backtest skill beside it. It follows the geography chosen on the Explorer when a forecast series exists for it.'],
  'what-if': ['WHAT-IF SIMULATION', 'What if vaccination coverage were different?', 'Move the slider and the in-browser ensemble re-runs a stated hypothetical outbreak. An illustrative scenario, not a prediction.'],
  sources: ['SOURCES', 'Data sources and method', 'Where every number comes from, under what terms, and how it is handled.'],
};
function sourceFooter(): HTMLElement {
  const line = element('p', undefined, 'source-footer');
  line.append('Data: ');
  footerSources.forEach((source, i) => {
    const link = (label: string, url: string, name?: string) => {
      const a = element('a', label);
      a.href = url; a.target = '_blank'; a.rel = 'noopener noreferrer';
      if (name) a.setAttribute('aria-label', name);
      return a;
    };
    if (source.links) {
      // One provider, several pages: the name, then each page it is linked to.
      line.append(`${source.label} (`);
      source.links.forEach((l, j) => line.append(link(l.label, l.url, `${source.label} ${l.label}`), j < source.links!.length - 1 ? ', ' : ''));
      line.append(')');
    } else line.append(link(source.label, source.url));
    line.append(i < footerSources.length - 1 ? ', ' : ' · ');
  });
  const all = element('a', 'Sources and attribution');
  all.href = pageHash('sources');
  line.append(all);
  return line;
}
let unroute: (() => void) | undefined;
export function shell(root: HTMLElement): Shell {
  root.replaceChildren();
  const header = element('header', undefined, 'site-header');
  const brand = element('a', 'koplik', 'brand');
  brand.href = pageHash('explorer');
  const nav = element('nav', undefined, 'primary-nav');
  nav.setAttribute('aria-label', 'Primary');
  const list = element('ul');
  for (const id of pageIds) {
    const link = element('a', pageTitles[id]);
    link.href = pageHash(id);
    link.dataset.page = id;
    const item = element('li');
    item.append(link);
    list.append(item);
  }
  nav.append(list);
  header.append(brand, element('span', 'Measles outbreak intelligence', 'tagline'), nav);
  const main = element('main');
  main.id = 'main';
  const pages = {} as Record<PageId, HTMLElement>;
  for (const id of pageIds) {
    const page = element('div', undefined, `page page-${id}`);
    page.dataset.pageView = id;
    // Until the router runs, only the first page shows.
    page.hidden = id !== 'explorer';
    if (id !== 'explorer') {
      const [eyebrow, title, intro] = pageIntros[id];
      page.append(element('p', eyebrow, 'eyebrow'), element('h1', title), element('p', intro, 'intro'));
    }
    pages[id] = page;
    main.append(page);
  }
  const footer = element('footer', undefined, 'site-footer');
  // The disclaimer (spec line 7) and the source attribution appear on every page.
  footer.append(element('p', disclaimer, 'disclaimer'), sourceFooter());
  root.append(header, main, footer);
  mountProvenanceDrawer(root);
  // Each shell replaces the last one, so the previous router (bound to removed nodes) is released first.
  unroute?.();
  unroute = mountRouter(root);
  return { main, pages };
}
export function showStatus(root: HTMLElement, message: string, error = false) {
  const { pages } = shell(root);
  const status = element('p', message, 'notice');
  status.setAttribute('role', error ? 'alert' : 'status');
  pages.explorer.append(element('h1', 'Measles across the United States'), status);
}

export type MapFactory = (container: HTMLElement, data: Dataset, onSelect: (id: string) => void, onError: () => void) => MapView;

export function mountDashboard(root: HTMLElement, data: Dataset, mapFactory: MapFactory = createLazyMap): () => void {
  const { pages } = shell(root);
  const main = pages.explorer;
  if (data.synthetic) main.append(element('p', 'SYNTHETIC TEST DATA · Invented values and simplified geometry for development only. These are not observed measles reports.', 'synthetic notice'));
  main.append(element('p', 'SURVEILLANCE EXPLORER', 'eyebrow'), element('h1', 'Measles across the United States'),
    element('p', 'Explore reported cases, vaccination coverage and the pace of an outbreak. Missing reports stay missing.', 'intro'));

  main.append(summarySection(explorerSummary(data), data.synthetic));
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
  const definitionLabel = element('label', 'Case definition');
  const definitionSelect = element('select');
  definitionSelect.id = 'case-definition';
  definitionLabel.htmlFor = definitionSelect.id;
  let definition: CaseDefinition | undefined;
  let definitionLevel: 'state' | 'county' | undefined;
  const drill = element('button', 'Explore Texas counties →', 'drill-button');
  drill.type = 'button';
  toolbar.append(metricLabel, metricSelect, definitionLabel, definitionSelect, drill);
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
  // Texas counties only: the cumulative count each DSHS report printed (contracts v8), above the weekly series.
  const cumulative = element('div', undefined, 'cumulative-reports');
  const charts = element('div', undefined, 'charts');
  detail.append(summary, chartControls, cumulative, charts);
  grid.append(mapPanel, detail);
  main.append(grid);
  const reports = element('details', undefined, 'panel reports');
  const reportsSummary = element('summary', 'Read exact weekly reports and R_t status');
  reports.append(reportsSummary);
  const tableContainer = element('div', undefined, 'table-scroll');
  reports.append(tableContainer);
  main.append(reports);
  const comparisons = element('details', undefined, 'panel reports');
  comparisons.append(element('summary', 'Compare geography values and sources'));
  const comparisonRows = element('div', undefined, 'table-scroll');
  comparisons.append(comparisonRows); main.append(comparisons);
  // The full attribution lives on the Sources page; every page's footer links to it.
  for (const note of methodNote) pages.sources.append(element('p', note, 'method-note'));
  pages.sources.append(attributionSection(data));
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
      const option = element('option', geography.name);
      option.value = geography.id;
      geographySelect.append(option);
    }
    geographySelect.value = selected;
    geographySelect.disabled = geographies.length === 0;
  }
  function renderReports(year: number) {
    const cases = data.cases.filter((r) => r.geography === selected && r.week.year === year).sort(compareWeeks);
    const rt = data.rt.filter((r) => r.geography === selected && r.week.year === year).sort(compareWeeks);
    const series = caseSeries(cases, year);
    const caseSvgs = caseCharts(cases, year, data.synthetic);
    const caseBlocks = series.flatMap((one, i) => [element('h3', `Weekly ${one.label}`), caseSvgs[i],
      element('p', `Bars show new ${one.label} in each MMWR week. Baseline ticks mean reported zero; gaps mean no data.`, 'chart-note')]);
    if (series.length > 1) caseBlocks.unshift(element('p', `This geography has case counts under ${series.length} different case definitions in MMWR ${year} (${series.map((one) => one.label).join('; ')}). They count different things, so each is charted separately and they are never added together or compared on one axis.`, 'notice case-definitions-notice'));
    if (!series.length) caseBlocks.push(element('h3', 'Weekly cases'));
    if (level === 'county') caseBlocks.unshift(element('p', countyWeeklyNote, 'notice county-weekly-note'));
    const cumulativeRows = level === 'county' ? cumulativeSeries(data.cumulative, selected) : [];
    cumulative.hidden = level !== 'county';
    cumulative.replaceChildren(...(level === 'county' ? cumulativeSection(data.cumulative, selected, name.textContent || '', data.synthetic) : []));
    charts.replaceChildren(...caseBlocks,
      element('h3', 'Effective reproduction number · R_t'), rtChart(rt, year, data.synthetic),
      element('p', 'Line: mean · Ribbon: credible interval · Dashed line: R_t = 1. I / grey hatch: insufficient data. P / dashed outline: provisional, estimate withheld. IP: both statuses. Blank: no row. Exact interval levels appear in the report table.', 'chart-note'));
    if (!cases.length) charts.prepend(element('p', 'No case data for this geography and year.', 'notice'));
    if (!rt.some((r) => r.status === 'ok' && !r.provisional)) charts.append(element('p', 'No final R_t estimate for this geography and year.', 'notice'));
    const table = element('table');
    table.append(element('caption', `${name.textContent} · MMWR ${year}`));
    const head = element('thead');
    const headers = element('tr');
    const caseColumns = series.length ? series : [undefined];
    for (const text of ['MMWR week', ...caseColumns.map((one) => `New ${one?.label ?? 'cases'}`), 'R_t / quality']) {
      const cell = element('th', text);
      cell.scope = 'col';
      headers.append(cell);
    }
    head.append(headers);
    const body = element('tbody');
    const weeks = [...new Set([...cases, ...rt].map((r) => r.week.week))].sort((a, b) => a - b);
    // Show omitted weeks inside a reported period as no data as well as explicit missing rows.
    if (weeks.length) for (let week = weeks[0]; week <= weeks.at(-1)!; week++) {
      const estimates = rt.filter((row) => row.week.week === week).sort((a, b) => a.interval_level - b.interval_level);
      const row = element('tr');
      row.dataset.geography = selected;
      row.dataset.week = `${year}-${week}`;
      const weekCell = element('th', `W${week}`);
      weekCell.scope = 'row';
      const caseCells = caseColumns.map((one) => {
        const c = (one ? one.rows : cases).find((r) => r.week.week === week);
        const cell = element('td', c?.cases.status === 'reported' ? String(c.cases.count) : `No data${c?.cases.status === 'missing' ? ` · ${missingReasonWords[c.cases.reason]}` : ''}`);
        if (c?.cases.status === 'reported') cell.replaceChildren(provenanceNumber(String(c.cases.count), {
          label: `${name.textContent} · ${year} W${week} · ${c.cases.count} ${caseDefinitionLabels[c.case_definition]}`, records: c.provenance, synthetic: data.synthetic,
        }));
        return cell;
      });
      const rtCell = element('td', estimates.length ? undefined : 'No data');
      for (const estimate of estimates) {
        const report = element('div');
        const text = `${estimate.interval_level * 100}% · ${rtLabel(estimate)}`;
        report.append(provenanceNumber(text, {
          label: `${name.textContent} · ${year} W${week} · ${text}`, records: estimate.provenance, synthetic: data.synthetic,
          note: 'Derived R_t estimate. These are all source records linked by this estimate artifact; sources are not inferred from neighbouring reports.',
        }));
        report.dataset.intervalLevel = String(estimate.interval_level);
        if (estimate.provisional) report.className = 'provisional';
        else if (estimate.status === 'insufficient_data') report.className = 'insufficient';
        rtCell.append(report);
      }
      row.append(weekCell, ...caseCells, rtCell);
      body.append(row);
    }
    table.append(head, body);
    reportsSummary.textContent = level === 'county' ? 'Read exact weekly and cumulative reports and R_t status' : 'Read exact weekly reports and R_t status';
    tableContainer.replaceChildren(weeks.length ? table : element('p', 'No weekly reports available.', 'notice'),
      ...(cumulativeRows.length ? [cumulativeTable(cumulativeRows.filter((r) => r.case_definition === 'confirmed'), name.textContent || '', data.synthetic)] : []));
  }
  function render() {
    geographySelect.value = selected;
    const geography = data.geographies.find((g) => g.id === selected);
    name.textContent = geography?.name || 'No geography data';
    const caseRows = (id: string) => data.cases.filter((r) => r.geography === id && r.week.year === (metric === 'cases-2025' ? 2025 : 2026));
    // Every case measure says in words which cases it counts, taken from the rows behind it.
    const measureName = (id: string) => metric === 'coverage' ? metricLabels[metric] : `${metricLabels[metric]} · ${caseDefinitionWords(caseRows(id))}`;
    measure.textContent = level === 'county' ? `Texas outbreak · Cases 2025 · ${caseDefinitionWords(caseRows(selected))}` : measureName(selected);
    const currentValue = metricValue(data, selected, metric);
    const sources = (id: string) => metric === 'coverage' ?
      data.coverage.filter((r) => r.geography === id).sort((a, b) => b.school_year.localeCompare(a.school_year)).slice(0, 1).flatMap((r) => r.provenance) :
      caseRows(id).sort(compareWeeks).flatMap((r) => r.provenance);
    const numberInfo = (id: string, label: string) => ({
      label, records: sources(id), synthetic: data.synthetic,
      note: metric === 'coverage' ? 'Latest school-year coverage row, including any imputation metadata shown alongside the value.' :
        `Counts ${caseDefinitionWords(caseRows(id))}. Sum of the displayed contiguous reported period. Lists every source record attached to the input weekly reports; this is not a full-year total.`,
    });
    value.replaceChildren(provenanceNumber(currentValue.label, numberInfo(selected, `${name.textContent} · ${measureName(selected)} · ${currentValue.label}`)));
    value.classList.toggle('missing-value', currentValue.value === null);
    valueDetail.textContent = currentValue.detail;
    geographyLabel.textContent = level === 'state' ? 'Select a state (keyboard accessible)' : 'Select a Texas county (keyboard accessible)';
    drill.textContent = level === 'state' ? 'Explore Texas counties →' : '← United States';
    metricSelect.disabled = level === 'county';
    yearSelect.disabled = level === 'county';
    legend.replaceChildren();
    const isCoverage = metric === 'coverage';
    // Each case definition has its own colour scale and legend block; no value is coloured on another definition's scale.
    const definitions = caseDefinitionsAt(data, level);
    if (!definitions.includes(definition!) || definitionLevel !== level) definition = defaultCaseDefinition(definitions);
    definitionLevel = level;
    definitionSelect.replaceChildren(...definitions.map((one) => { const option = element('option', caseDefinitionLabels[one]); option.value = one; return option; }));
    if (definition) definitionSelect.value = definition;
    const showDefinition = !isCoverage && definitions.length > 0;
    definitionLabel.hidden = definitionSelect.hidden = !showDefinition;
    definitionSelect.disabled = definitions.length < 2;
    if (isCoverage) {
      const block = element('span', 'MMR coverage · 0–100%', 'legend-scale');
      block.style.setProperty('--scale', `linear-gradient(90deg, ${coverageScale.map(([, colour]) => colour).join(', ')})`);
      legend.append(block);
    }
    else if (definition) {
      const block = element('span', `Reported ${caseDefinitionLabels[definition]} · 0 → 500+`, 'legend-scale');
      block.dataset.caseDefinition = definition;
      block.style.setProperty('--scale', `linear-gradient(90deg, ${caseScales[definition].map(([, colour]) => colour).join(', ')})`);
      legend.append(block);
    } else legend.append(element('span', 'Reported cases · no case data', 'legend-scale'));
    legend.append(element('span', '▨ No data', 'legend-missing'));
    const others = !isCoverage && definition ? otherDefinitionGeographies(data, level, metric, definition) : [];
    if (others.length) legend.append(element('p', `${others.length} ${others.length === 1 ? 'geography reports' : 'geographies report'} cases under a different case definition and ${others.length === 1 ? 'is' : 'are'} shown as No data on this ${caseDefinitionLabels[definition!]} scale. Different definitions count different things, so choose another case definition above to see them; no value is coloured on another definition's scale.`, 'legend-note'));
    renderReports(Number(yearSelect.value));
    const comparisonTable = element('table');
    comparisonTable.append(element('caption', metricLabels[metric]));
    const comparisonBody = element('tbody');
    for (const geography of available()) {
      const row = element('tr');
      const title = element('th', geography.name); title.scope = 'row';
      const cell = element('td');
      const current = metricValue(data, geography.id, metric);
      cell.append(provenanceNumber(current.label, numberInfo(geography.id, `${geography.name} · ${measureName(geography.id)} · ${current.label}`)));
      row.append(title, cell, element('td', current.detail)); comparisonBody.append(row);
    }
    comparisonTable.append(comparisonBody); comparisonRows.replaceChildren(comparisonTable);
    map?.update(level, metric, selected, isCoverage ? undefined : definition);
    root.dispatchEvent(new CustomEvent('koplik:selection', { bubbles: true, detail: { geography: selected, metric, year: Number(yearSelect.value), data } }));
  }
  metricSelect.addEventListener('change', () => {
    metric = metricSelect.value as Metric;
    if (metric !== 'coverage') yearSelect.value = metric === 'cases-2025' ? '2025' : '2026';
    chooseOptions(); render();
  });
  definitionSelect.addEventListener('change', () => { definition = definitionSelect.value as CaseDefinition; render(); });
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
    map.update(level, metric, selected, definition);
  } catch {
    mapStatus.textContent = 'Map unavailable. Select a geography below to explore all reports.';
  }
  // The map was laid out while hidden (or not) as the page changed: it must be told its size when the Explorer shows again.
  const onPage = (event: Event) => { if ((event as CustomEvent<{ page: PageId }>).detail.page === 'explorer') map?.resize?.(); };
  root.addEventListener(pageEvent, onPage);
  return () => { root.removeEventListener(pageEvent, onPage); map?.destroy(); };
}
