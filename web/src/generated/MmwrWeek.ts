/* Generated from koplik-contracts schema/v1. Run npm run generate:types. */

/**
 * An MMWR year and week number (`week` is `1..=52` or `1..=53`).
 */
export interface MmwrWeek {
  /**
   * Week of the MMWR year; 1 to 52, or 53 in a 53-week year.
   */
  week: number;
  /**
   * MMWR year (not always the calendar year of every day in the week).
   */
  year: number;
}
