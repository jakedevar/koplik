import { caseDefinitionLabels, caseDefinitionsAt, compareWeeks, defaultCaseDefinition, mapMetricValue, type CaseDefinition, type Dataset, type Metric } from './data';
import type { Provenance } from './generated/Provenance';
import type { WeeklyCaseCount } from './generated/v3/WeeklyCaseCount';
import { uniqueProvenance } from './provenance';

/**
 * The Explorer's national summary. Every number is an aggregate over the state-level jurisdictions the site already shows:
 * only a jurisdiction that has the figure itself (a complete reported series for the period, under one case definition) is
 * included; one that does not is left out and counted as missing, never as zero. Each stat says how many of the
 * jurisdictions it covers and which are not included, and carries the records of every row it sums.
 */
export interface SummaryStat {
  id: 'cases-2025' | 'cases-2026' | 'latest-week';
  label: string;
  /** The sum over the included jurisdictions, or null when none is included ("No data"). */
  value: number | null;
  /** The jurisdictions the sum covers, out of all state-level jurisdictions. */
  included: number;
  total: number;
  /** Names of the jurisdictions not included, sorted. */
  notIncluded: string[];
  /** One line saying what the number is and how many jurisdictions it covers. */
  detail: string;
  /** Every source record of every row that was summed. */
  records: Partial<Provenance>[];
  /** The latest-week sum only: the week is provisional (reporting delay) and the count may still change. */
  provisional?: boolean;
}
export interface SummaryAsOf {
  week: { year: number; week: number };
  /** The latest week is provisional: still subject to reporting delay. */
  provisional: boolean;
  /** The latest retrieval time among the records behind the latest week's reports, exactly as recorded. */
  retrieved?: string;
  records: Partial<Provenance>[];
}
export interface ExplorerSummary {
  definition?: CaseDefinition;
  /** Absent when no jurisdiction has a reported weekly count. */
  asOf?: SummaryAsOf;
  stats: SummaryStat[];
}

/** `2026-10-07T09:55:29Z` as `2026-10-07 09:55 UTC`; anything that is not that shape is shown as recorded. */
export function formatRetrieved(retrieved: string): string {
  const match = /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2})(?::\d{2}(?:\.\d+)?)?Z$/.exec(retrieved);
  return match ? `${match[1]} ${match[2]} UTC` : retrieved;
}
/** The wording the pipeline gives its provisional flag (RtEstimate.provisional): a recent week still subject to reporting delay. */
export const provisionalWords = 'Provisional (reporting delay): the two newest weeks are still being reported and the count may change';
export const weekWords = (week: { year: number; week: number }) => `MMWR ${week.year} W${week.week}`;

export function explorerSummary(data: Dataset): ExplorerSummary {
  const jurisdictions = data.geographies.filter((g) => g.level === 'state');
  const total = jurisdictions.length;
  const definition = defaultCaseDefinition(caseDefinitionsAt(data, 'state'));
  const states = new Set(jurisdictions.map((g) => g.id));
  const rows: WeeklyCaseCount[] = definition ? data.cases.filter((r) => states.has(r.geography) && r.case_definition === definition) : [];
  const words = definition ? caseDefinitionLabels[definition] : 'cases';
  const names = (ids: Set<string>) => jurisdictions.filter((g) => !ids.has(g.id)).map((g) => g.name).sort((a, b) => a.localeCompare(b, 'en'));
  const coverage = (included: number) => `${included} of ${total} jurisdictions`;

  const periodStat = (id: 'cases-2025' | 'cases-2026', label: string, metric: Metric, year: number): SummaryStat => {
    const used = new Set<string>();
    let sum = 0;
    if (definition) for (const g of jurisdictions) {
      const value = mapMetricValue(data, g.id, metric, definition).value;
      if (value !== null) { used.add(g.id); sum += value; }
    }
    const summed = rows.filter((r) => used.has(r.geography) && r.week.year === year);
    return { id, label, value: used.size ? sum : null, included: used.size, total, notIncluded: names(used),
      detail: `${words} · ${coverage(used.size)}${used.size < total ? ', the rest with missing or incomplete weekly reports and not counted as zero' : ''} · Sum of each jurisdiction's contiguous reported weeks, MMWR ${year}; not a full-year total`,
      records: uniqueProvenance(summed.flatMap((r) => r.provenance)) };
  };

  const reported = rows.filter((r) => r.cases.status === 'reported').sort(compareWeeks);
  const latest = reported.at(-1)?.week;
  const inLatest = latest ? reported.filter((r) => r.week.year === latest.year && r.week.week === latest.week) : [];
  const latestIds = new Set(inLatest.map((r) => r.geography));
  const latestSum = inLatest.reduce((n, r) => n + (r.cases.status === 'reported' ? r.cases.count : 0), 0);
  const latestRecords = uniqueProvenance(inLatest.flatMap((r) => r.provenance));
  // The pipeline's rule (koplik-epi RtConfig.provisional_weeks = 2): the two newest weeks are provisional, and it flags them on the R_t rows.
  // So the newest week is provisional when the state-level R_t rows for it say so; with no R_t row for that week the rule itself applies.
  const latestRt = latest ? data.rt.filter((r) => states.has(r.geography) && r.week.year === latest.year && r.week.week === latest.week) : [];
  const provisional = latestRt.length ? latestRt.some((r) => r.provisional) : true;
  const retrieved = latestRecords.map((r) => r.retrieved_at).filter((t): t is string => Boolean(t)).sort().at(-1);

  return {
    definition,
    asOf: latest ? { week: { year: latest.year, week: latest.week }, provisional, retrieved, records: latestRecords } : undefined,
    stats: [
      periodStat('cases-2025', 'Reported cases · 2025', 'cases-2025', 2025),
      periodStat('cases-2026', 'Reported cases · 2026 to date', 'cases-2026', 2026),
      { id: 'latest-week', label: latest ? `New cases · latest week, ${weekWords(latest)}` : 'New cases · latest week', value: latest ? latestSum : null,
        included: latestIds.size, total, notIncluded: names(latestIds),
        detail: `${words} · ${coverage(latestIds.size)} reported this week${latestIds.size < total ? '; the rest have no report for it and are not counted as zero' : ''}`,
        records: latestRecords, provisional },
    ],
  };
}
