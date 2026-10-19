/** The game clock is HHMM from 600 to 2600; past midnight it keeps counting (2530 = 1:30). */
export function formatClock(hhmm: number): string {
  const h = Math.floor(hhmm / 100) % 24;
  const m = hhmm % 100;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

export const SEASONS = ["spring", "summer", "fall", "winter"] as const;
export type Season = (typeof SEASONS)[number];

/** Normalizes the game's season id ("spring", "Fall") to a known key, or null. */
export function seasonKey(raw: string): Season | null {
  const s = raw.toLowerCase();
  return (SEASONS as readonly string[]).includes(s) ? (s as Season) : null;
}
