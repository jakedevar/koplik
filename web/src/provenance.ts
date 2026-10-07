import type { Provenance } from './generated/Provenance';
import { attributionFor, licenceTerms } from './attribution';

// Local display metadata, not a new published artifact shape. Partial records stay partial.
export interface ProvenanceInfo {
  label: string;
  records: readonly Partial<Provenance>[];
  synthetic?: boolean;
  note?: string;
}
type FocusTarget = HTMLElement | SVGElement;
interface Request extends ProvenanceInfo { trigger: FocusTarget }
const information = new WeakMap<FocusTarget, ProvenanceInfo>();
const drawers = new WeakMap<HTMLElement, () => void>();

export function uniqueProvenance(records: readonly Partial<Provenance>[]): Partial<Provenance>[] {
  const seen = new Set<string>();
  return records.filter((record) => {
    const key = JSON.stringify([record.source_id, record.sha256, record.url, record.retrieved_at, record.licence_id]);
    if (seen.has(key)) return false;
    seen.add(key); return true;
  });
}

/** Both SVG marks and HTML numbers use the same drawer, without reading values from the DOM. */
export function bindProvenance(node: FocusTarget, info: ProvenanceInfo): void {
  if (!information.has(node)) {
    const open = () => node.dispatchEvent(new CustomEvent<Request>('koplik:provenance', {
      bubbles: true, detail: { ...information.get(node)!, trigger: node },
    }));
    node.addEventListener('click', (event) => { event.stopPropagation(); open(); });
    if (!(node instanceof HTMLButtonElement)) {
      node.addEventListener('keydown', (event) => {
        const key = (event as KeyboardEvent).key;
        if (key === 'Enter' || key === ' ') { event.preventDefault(); event.stopPropagation(); open(); }
      });
    }
  }
  information.set(node, info);
  node.classList.add('provenance-trigger');
  node.setAttribute('data-provenance', '');
  node.setAttribute('role', 'button');
  node.setAttribute('tabindex', '0');
  node.setAttribute('aria-haspopup', 'dialog');
  node.setAttribute('aria-label', `${info.label}. Open provenance.`);
}

export function provenanceNumber(text: string, info: ProvenanceInfo): HTMLButtonElement {
  const button = document.createElement('button');
  button.type = 'button'; button.textContent = text; button.className = 'provenance-number';
  bindProvenance(button, info);
  return button;
}

const displayed = (value: string | undefined) => value?.trim() ? value : 'Missing';
function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
}

/** One native modal per shell. Native modality traps focus; the fallback supports unit DOMs. */
export function mountProvenanceDrawer(root: HTMLElement): () => void {
  drawers.get(root)?.();
  const dialog = element('dialog');
  dialog.className = 'provenance-drawer';
  dialog.setAttribute('aria-label', 'Number provenance');
  dialog.setAttribute('aria-modal', 'true');
  const close = element('button', 'Close provenance'); close.type = 'button';
  const heading = element('h2', 'Number provenance');
  const content = element('div');
  dialog.append(close, heading, content); root.append(dialog);
  let trigger: FocusTarget | undefined;
  function returnFocus() {
    if (trigger?.isConnected) trigger.focus?.();
    trigger = undefined;
  }
  function dismiss() {
    if (typeof dialog.close === 'function') dialog.close();
    else dialog.removeAttribute('open');
    returnFocus();
  }
  close.addEventListener('click', dismiss);
  dialog.addEventListener('cancel', (event) => { event.preventDefault(); dismiss(); });
  dialog.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') { event.preventDefault(); dismiss(); }
    if (event.key === 'Tab') {
      const targets = [...dialog.querySelectorAll<HTMLElement>('button, a[href]')];
      const first = targets[0]; const last = targets.at(-1)!;
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
  });
  const onOpen = (event: Event) => {
    const request = (event as CustomEvent<Request>).detail;
    if (!request?.trigger) return;
    trigger = request.trigger;
    heading.textContent = request.label;
    content.replaceChildren();
    const records = uniqueProvenance(request.records);
    const synthetic = request.synthetic || records.some((p) => p.source_id?.startsWith('synthetic'));
    if (synthetic) {
      const label = element('p', 'SYNTHETIC · Test/development input, not a published source or observed data.');
      label.className = 'synthetic notice'; content.append(label);
    }
    if (request.note) content.append(element('p', request.note));
    if (!records.length) content.append(element('p', 'Source provenance missing: this artifact does not link source records for this value.'));
    for (const record of records) {
      const section = element('section'); section.className = 'provenance-record';
      const list = element('dl');
      for (const [label, value] of [['Source identifier', record.source_id], ['Snapshot sha256', record.sha256], ['Source URL', record.url], ['Retrieval time', record.retrieved_at], ['Licence / terms', record.licence_id]] as const) {
        const detail = element('dd');
        if (label === 'Source URL' && value) {
          let url: URL | undefined;
          try { url = new URL(value); } catch { /* Malformed URLs remain plain text. */ }
          if (url && ['http:', 'https:'].includes(url.protocol)) {
            const link = element('a', value); link.href = value; link.target = '_blank'; link.rel = 'noopener noreferrer'; detail.append(link);
          } else detail.textContent = displayed(value);
        } else if (label === 'Snapshot sha256' && value?.trim()) detail.append(element('code', value));
        else detail.textContent = displayed(value);
        list.append(element('dt', label), detail);
      }
      // Terms and attribution sit next to the recorded licence id (which is never rewritten).
      const terms = licenceTerms(record.licence_id);
      const attribution = attributionFor(record.source_id, record.licence_id);
      const termsDetail = element('dd');
      termsDetail.className = 'provenance-terms';
      termsDetail.textContent = terms ? `${terms.ruling}. ${terms.terms}` : 'Terms not recorded for this licence id.';
      const attributionDetail = element('dd');
      attributionDetail.className = 'provenance-attribution';
      attributionDetail.textContent = attribution.length ? attribution.join(' ') : terms ? 'No attribution is shown for this record.' : 'Attribution not recorded for this source.';
      list.append(element('dt', 'Terms (ruling)'), termsDetail, element('dt', 'Attribution'), attributionDetail);
      section.append(list); content.append(section);
    }
    if (!dialog.open) {
      if (typeof dialog.showModal === 'function') dialog.showModal();
      else dialog.setAttribute('open', '');
    }
    close.focus();
  };
  root.addEventListener('koplik:provenance', onOpen);
  const cleanup = () => {
    root.removeEventListener('koplik:provenance', onOpen); dialog.remove();
    if (drawers.get(root) === cleanup) drawers.delete(root);
  };
  drawers.set(root, cleanup);
  return cleanup;
}
