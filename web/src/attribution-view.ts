import { licences, licenceTerms, sources } from './attribution';
import type { Dataset } from './data';

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}

/** Every source with its attribution text, link and terms, always visible. Data-bearing ids we hold no terms for are said so. */
export function attributionSection(data: Dataset): HTMLElement {
  const section = element('section', undefined, 'panel attribution');
  section.setAttribute('aria-labelledby', 'attribution-heading');
  const heading = element('h3', 'Data sources and attribution');
  heading.id = 'attribution-heading';
  section.append(heading, element('p', 'CDC and Census data are US federal public domain (17 USC 105). Texas DSHS data is public information, used with attribution and a link. Open any number for the exact snapshot, terms and attribution behind it.', 'chart-note'));
  const list = element('ul', undefined, 'attribution-list');
  for (const source of sources) {
    const item = element('li');
    const link = element('a', source.name);
    link.href = source.url; link.target = '_blank'; link.rel = 'noopener noreferrer';
    const rulings = [...new Set(source.licences.map((id) => licenceTerms(id)?.ruling).filter(Boolean))];
    item.append(link, element('span', ` · ${rulings.join('; ')}`, 'attribution-ruling'));
    for (const id of source.licences) {
      const text = licences[id]?.attribution;
      if (text) item.append(element('p', text, 'attribution-text'));
    }
    list.append(item);
  }
  section.append(list);
  const known = new Set(sources.flatMap((s) => s.ids));
  const records = [...data.geographies, ...data.cases, ...data.coverage, ...data.rt].flatMap((row) => row.provenance)
    .concat(...[...data.states.features, ...data.counties.features].map((f) => f.properties.provenance ?? []));
  const unknown = [...new Set(records.map((p) => p.source_id).filter((id) => !known.has(id) && !id.startsWith('synthetic')))].sort();
  if (unknown.length) section.append(element('p', `Sources in this build without recorded terms or attribution: ${unknown.join(', ')}.`, 'notice'));
  if (data.synthetic) section.append(element('p', 'SYNTHETIC · The figures in this build are invented test input, not observations from any source above.', 'synthetic notice'));
  return section;
}
