/**
 * The Stardew Valley Fair grange display, scored as `Event.judgeGrange` does in 1.6:
 * 14 base points, per item its quality and price, 5 per category group up to 30,
 * and 9 minus 2 for each empty slot.
 */

export interface GrangeItem {
  id: string;
  name: string;
  category: number;
  /** 0 normal, 1 silver, 2 gold, 4 iridium. */
  quality: number;
  /** What a shop pays for one, professions included. */
  price: number;
  stack: number;
  /** "inventory" or the location of the chest it sits in. */
  where: string;
  /** What holds it; older companions leave it out. */
  kind?: "chest" | "fridge" | "grabber" | "junimo" | "inventory";
  tile?: { x: number; y: number } | null;
}

export const SLOTS = 9;
const BASE = 14;
const GROUP_POINTS = 5;
const GROUP_CAP = 30;

// categories the judge lumps together, keyed by the one it records
const GROUP_OF: Record<number, number> = {
  [-75]: -75,
  [-79]: -79,
  [-18]: -5,
  [-14]: -5,
  [-6]: -5,
  [-5]: -5,
  [-12]: -12,
  [-2]: -12,
  [-4]: -4,
  [-81]: -81,
  [-80]: -81,
  [-27]: -81,
  [-7]: -7,
  [-26]: -26,
};

export function groupOf(item: GrangeItem): number | null {
  return GROUP_OF[item.category] ?? null;
}

/** What one item adds before the variety bonus. */
export function itemPoints(item: GrangeItem): number {
  const p = item.price;
  const q = item.quality;
  return (
    q + 1 + Number(p >= 20) + Number(p >= 90) + Number(p >= 200) + Number(p >= 300 && q < 2) + Number(p >= 400 && q < 1)
  );
}

export function grangeScore(items: GrangeItem[]): number {
  const shown = items.slice(0, SLOTS);
  const groups = new Set(shown.map(groupOf).filter((g) => g !== null));
  const empty = SLOTS - shown.length;
  return (
    BASE +
    shown.reduce((sum, i) => sum + itemPoints(i), 0) +
    Math.min(GROUP_CAP, groups.size * GROUP_POINTS) +
    (SLOTS - 2 * empty)
  );
}

/** Lewis' verdict: 1st at 90, 2nd at 75, 3rd at 60, else 4th. */
export function place(score: number): 1 | 2 | 3 | 4 {
  if (score >= 90) return 1;
  if (score >= 75) return 2;
  if (score >= 60) return 3;
  return 4;
}

/**
 * Picks the best display from `items`. Stacks can fill several slots. Exact: for every
 * set of groups to cover it takes the best item of each, then the best of the rest,
 * and keeps the highest score; with at most eight groups that is 256 tries.
 */
export function bestGrange(items: GrangeItem[]): GrangeItem[] {
  const copies = items
    .flatMap((i) => Array.from({ length: Math.min(i.stack, SLOTS) }, () => i))
    .sort((a, b) => itemPoints(b) - itemPoints(a));
  const groups = [...new Set(copies.map(groupOf).filter((g): g is number => g !== null))];
  let best: GrangeItem[] = copies.slice(0, SLOTS);
  let bestScore = grangeScore(best);
  for (let mask = 1; mask < 1 << groups.length; mask++) {
    const want = groups.filter((_, i) => mask & (1 << i));
    if (want.length > SLOTS) continue;
    const used = new Set<number>();
    const pick: GrangeItem[] = [];
    for (const g of want) {
      const at = copies.findIndex((c, i) => !used.has(i) && groupOf(c) === g);
      used.add(at);
      pick.push(copies[at]);
    }
    for (let i = 0; i < copies.length && pick.length < SLOTS; i++) {
      if (!used.has(i)) {
        used.add(i);
        pick.push(copies[i]);
      }
    }
    const score = grangeScore(pick);
    if (score > bestScore) {
      best = pick;
      bestScore = score;
    }
  }
  return best;
}
