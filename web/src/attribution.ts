// Web copy of the "Licences and terms" table in SOURCES.md, ruled 2026-10-07: CDC and Census data are US
// federal public domain (17 USC 105); Texas DSHS data is public information used with attribution and a
// link; Internet Archive captures fall under the Archive's terms plus the publisher's. attribution.test.ts
// checks this table against SOURCES.md, so edit both together.
//
// Licence ids already written into data are immutable, so ids that say "unconfirmed" stay in rows and are
// mapped to the ruling here (and in SOURCES.md), never rewritten.

export interface LicenceTerms {
  /** The licence the ruling assigns; "none stated" where the publisher states none. */
  ruling: string;
  terms: string;
  /** Attribution text to show; null when nothing is shown (synthetic input, rejected fixture). */
  attribution: string | null;
}
export interface SourceEntry {
  name: string;
  url: string;
  /** Every `source_id` recorded in provenance for this source. */
  ids: string[];
  /** Licence ids that apply to its rows; more than one means all of them apply. */
  licences: string[];
}

/** The licence id of every Internet Archive capture; it applies only to records whose own URL is an Archive capture. */
const ARCHIVE_LICENCE = 'internet-archive-terms-of-use';
const PUBLIC_DOMAIN = 'US federal public domain (17 USC 105)';
const NONE_STATED = 'none stated (public information; attribution and link required)';

export const licences: Record<string, LicenceTerms> = {
  'cdc-open-data-terms-unconfirmed': { ruling: PUBLIC_DOMAIN,
    terms: "The dataset page and metadata name no licence (license: null); the publisher is CDC's Office of Public Health Data, Surveillance, and Technology. CDC is a US federal agency, so its data is a US government work in the public domain (17 USC 105). The dataset notes say counts are provisional, subject to ongoing revision, and \"presented as published each week\".",
    attribution: 'Source: Centers for Disease Control and Prevention (CDC), NNDSS Weekly Data, https://data.cdc.gov/resource/x9gk-5huc. Counts are provisional and combine confirmed and unknown-status cases. Koplik is not affiliated with or endorsed by CDC.' },
  'cdc-schoolvaxview-terms-unconfirmed': { ruling: PUBLIC_DOMAIN,
    terms: "The SchoolVaxView dataset is provided by CDC NCIRD, a US federal agency; US government work in the public domain (17 USC 105).",
    attribution: 'Source: Centers for Disease Control and Prevention (CDC), SchoolVaxView, Vaccination Coverage and Exemptions among Kindergartners, https://data.cdc.gov/Vaccinations/Vaccination-Coverage-and-Exemptions-among-Kinderga/ijqb-a7ye. Not affiliated with or endorsed by CDC.' },
  'us-census-public-domain': { ruling: PUBLIC_DOMAIN,
    terms: "US Census Bureau geographic materials are public domain US government works. 2024 technical documentation, §1.2 (https://www2.census.gov/geo/pdfs/maps-data/data/tiger/tgrshp2024/TGRSHP2024_TechDoc.pdf) states Census materials may be reproduced and requests source attribution. Boundaries are statistical depictions, not legal land descriptions (§1.1).",
    attribution: 'Source: US Census Bureau, 2024 cartographic boundary files (1:20,000,000), https://www.census.gov/geographies/mapping-files/time-series/geo/cartographic-boundary.html; Vintage 2025 population estimates, https://www.census.gov/programs-surveys/popest.html; 2025 Gazetteer files, https://www.census.gov/geographies/reference-files/time-series/geo/gazetteer-files.html. Boundaries are statistical depictions, not legal land descriptions.' },
  'census-open-data-terms-unconfirmed': { ruling: PUBLIC_DOMAIN,
    terms: "A US Census Bureau reference file (national_county2020.txt: county names and FIPS codes), a US government work in the public domain (17 USC 105); the file carries no licence statement of its own.",
    attribution: 'Source: US Census Bureau, national county reference file (2020 codes), https://www2.census.gov/geo/docs/reference/codes2020/national_county2020.txt.' },
  'census-county-codes-terms-unconfirmed': { ruling: `${PUBLIC_DOMAIN}; not consumed`,
    terms: "Recorded only on the rejected Census county API HTTP-200 \"Missing Key\" response, which is a negative fixture and is never read as county identities or coverage data.", attribution: null },
  'dshs-copyright-noncommercial-no-alteration': { ruling: NONE_STATED, terms: "DSHS states no data licence. Its \"Copyright and Disclaimer\" page (https://www.dshs.texas.gov/site-policies/copyright-disclaimer, read 2026-10-07) says: \"Unless otherwise noted on an individual document, file, home page, or the like, DSHS grants permission to copy and distribute files, documents and information provided for non-commercial use, so long as the information is copied and distributed without alteration.\" The outbreak page and PDFs carry no other notice. The ruling treats DSHS data as public information used with attribution and a link; Koplik therefore always names DSHS, links the DSHS page, and states which figures it derived (differenced, re-keyed by FIPS, charted) rather than presenting them as DSHS's own.",
    attribution: 'Source: Texas Department of State Health Services (DSHS), 2025 Measles Outbreak data, https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025. Counts were processed by Koplik (differenced by week, keyed by county FIPS); DSHS states no data licence. Not affiliated with or endorsed by DSHS.' },
  'texas-dshs-terms-unconfirmed': { ruling: NONE_STATED, terms: "DSHS's pages carry a copyright footer and DSHS states no data licence for the kindergarten coverage workbooks or the county crosswalk (see the DSHS notice quoted above). The ruling treats them as public information used with attribution and a link.",
    attribution: 'Source: Texas Department of State Health Services (DSHS), School Vaccination Coverage Levels, Kindergarten, https://www.dshs.texas.gov/immunizations/data/school/coverage; county identifiers from https://www.dshs.texas.gov/center-health-statistics/texas-county-numbers-public-health-regions. Percentages converted by Koplik from DSHS fractions; DSHS states no data licence. Not affiliated with or endorsed by DSHS.' },
  'internet-archive-terms-of-use': { ruling: "Internet Archive terms of use plus the underlying publisher's terms",
    terms: "The Internet Archive's terms of use (https://archive.org/about/terms.php) cover use of its service; the archived bytes remain the original publisher's content under the publisher's terms (DSHS: see above; Census: public domain). Captures are evidence of what the publisher published and when; nothing is republished from the Archive.",
    attribution: 'Archived copy retrieved through the Internet Archive Wayback Machine (https://web.archive.org); content is by its original publisher, credited as above.' },
  'synthetic-test-only': { ruling: 'Not a source', terms: 'Invented test input, not observed data.', attribution: null },
  'synthetic-project-test-input': { ruling: 'Not a source', terms: 'Invented test input, not observed data.', attribution: null },
};

export const sources: SourceEntry[] = [
  { name: 'CDC NNDSS Weekly Data (measles, by state)', url: 'https://data.cdc.gov/resource/x9gk-5huc',
    ids: ['cdc-nndss-weekly-measles'], licences: ['cdc-open-data-terms-unconfirmed'] },
  { name: 'CDC SchoolVaxView (kindergarten MMR coverage)', url: 'https://data.cdc.gov/Vaccinations/Vaccination-Coverage-and-Exemptions-among-Kinderga/ijqb-a7ye',
    ids: ['cdc-schoolvaxview-kindergarten'], licences: ['cdc-schoolvaxview-terms-unconfirmed'] },
  { name: 'US Census Bureau cartographic boundaries (2024, 1:20m)', url: 'https://www.census.gov/geographies/mapping-files/time-series/geo/cartographic-boundary.html',
    ids: ['census-cb-2024-states-20m', 'census-cb-2024-counties-20m'], licences: ['us-census-public-domain'] },
  { name: 'US Census Bureau population estimates (Vintage 2025) and 2025 Gazetteer', url: 'https://www.census.gov/programs-surveys/popest.html',
    ids: ['census-state-population-2025', 'census-county-population-2025', 'census-county-gazetteer-2025', 'census-state-gazetteer-2025'], licences: ['us-census-public-domain'] },
  { name: 'US Census Bureau county reference file (via the Internet Archive)', url: 'https://www2.census.gov/geo/docs/reference/codes2020/national_county2020.txt',
    ids: ['census-county-codes-2020-wayback'], licences: ['census-open-data-terms-unconfirmed', ARCHIVE_LICENCE] },
  { name: 'Texas DSHS 2025 measles outbreak page and data reports (live from dshs.texas.gov)', url: 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025',
    ids: ['dshs-measles-outbreak-page', 'dshs-measles-data-report'], licences: ['dshs-copyright-noncommercial-no-alteration'] },
  { name: 'Texas DSHS outbreak page and data reports as Internet Archive captures (publisher: DSHS)', url: 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025',
    ids: ['dshs-measles-outbreak-page-wayback', 'dshs-measles-data-report-wayback'],
    licences: ['dshs-copyright-noncommercial-no-alteration', ARCHIVE_LICENCE] },
  { name: 'Texas DSHS kindergarten coverage workbooks and county crosswalk', url: 'https://www.dshs.texas.gov/immunizations/data/school/coverage',
    ids: ['texas-dshs-kindergarten-2023', 'texas-dshs-kindergarten-2024', 'texas-dshs-county-fips'], licences: ['texas-dshs-terms-unconfirmed'] },
  { name: 'Internet Archive CDX capture listings', url: 'https://web.archive.org/',
    ids: ['wayback-cdx-listing'], licences: [ARCHIVE_LICENCE] },
];

/** Own-property lookup only: ids such as `__proto__` or `constructor` are unknown terms, never inherited members. */
export function licenceTerms(licenceId: string | undefined): LicenceTerms | undefined {
  return licenceId !== undefined && Object.hasOwn(licences, licenceId) ? licences[licenceId] : undefined;
}
export function sourceEntry(sourceId: string | undefined): SourceEntry | undefined {
  return sourceId ? sources.find((s) => s.ids.includes(sourceId)) : undefined;
}
/** True when a record's own source URL is an Internet Archive (Wayback) capture. */
export function isArchiveCapture(url: string | undefined): boolean {
  try { return !!url && new URL(url).hostname === 'web.archive.org'; } catch { return false; }
}
/**
 * Attribution text(s) for a provenance record: the source's own, else its recorded licence's. The Internet
 * Archive terms apply only when the record's own URL is an Archive capture (that record also keeps the
 * publisher's attribution); a record fetched live from the publisher never gets them, whatever its source id.
 * Without a URL the source id alone decides, by its own table entry.
 */
export function attributionFor(sourceId: string | undefined, licenceId: string | undefined, url?: string): string[] {
  const entry = sourceEntry(sourceId);
  let ids = entry?.licences ?? (licenceId ? [licenceId] : []);
  if (url !== undefined) {
    ids = ids.filter((id) => id !== ARCHIVE_LICENCE);
    if (isArchiveCapture(url)) ids = [...ids, ARCHIVE_LICENCE];
  }
  return ids.map((id) => licenceTerms(id)?.attribution).filter((text): text is string => !!text);
}

/**
 * The three data providers the compact footer on every page names, each with its link (DSHS terms require both).
 * A provider with `links` is shown as its name followed by each page in brackets; `url` is its first page.
 */
export const footerSources: { label: string; url: string; links?: { label: string; url: string }[] }[] = [
  { label: 'CDC', url: 'https://data.cdc.gov/resource/x9gk-5huc' },
  { label: 'US Census Bureau', url: 'https://www.census.gov/' },
  { label: 'Texas DSHS', url: 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025', links: [
    { label: 'outbreak data', url: 'https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025' },
    { label: 'school coverage', url: 'https://www.dshs.texas.gov/immunizations/data/school/coverage' },
  ] },
];
