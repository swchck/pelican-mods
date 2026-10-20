import { SEASONS, type Season } from "@/lib/game-clock";

/** A crop as the game's lists describe it. */
export interface Crop {
  /** Unqualified harvest item id. */
  id: string;
  seed: string | null;
  seasons: string[];
  /** Days per growth phase; empty in catalogs from before the calculator. */
  phases: number[];
  /** Days between harvests after the first; -1 when it's harvested once. */
  regrow: number;
  minStack: number;
  maxStack: number;
  extraChance: number;
  paddy: boolean;
  trellis: boolean;
  price: number;
  /** 0 where no shop price is known. */
  seedPrice: number;
}

export const FERTILIZERS = { none: 0, speedGro: 0.1, deluxe: 0.25, hyper: 0.33 } as const;
export type Fertilizer = keyof typeof FERTILIZERS;

export interface PlanOptions {
  season: Season;
  /** Day of the month it's planted on, 1–28. */
  day: number;
  fertilizer: Fertilizer;
  agriculturist: boolean;
  tiller: boolean;
  /** Greenhouse or Ginger Island: any season, so the plan covers the next 28 days. */
  anySeason: boolean;
}

export interface Plan {
  days: number;
  harvests: number;
  /** Seeds bought: once for a regrowing crop, once per harvest otherwise. */
  seeds: number;
  /** Expected items per harvest, extra-harvest rolls included. */
  perHarvest: number;
  income: number;
  profit: number;
  /** Profit per day the tile is in use over the plan. */
  perDay: number;
}

const f = Math.fround;

/**
 * Days from planting to the first harvest with a speed bonus, the way HoeDirt
 * applySpeedIncreases does it: up to three passes taking a day off each phase.
 */
export function growthDays(phases: number[], speed: number): number {
  const days = [...phases];
  const total = days.reduce((a, b) => a + b, 0);
  // the game multiplies in float32; doubles land on the other side of ceil for 0.35 and the like
  let remove = Math.ceil(f(total * speed));
  for (let tries = 0; remove > 0 && tries < 3; tries++) {
    for (let i = 0; i < days.length && remove > 0; i++) {
      if (i > 0 || days[i] > 1) {
        days[i]--;
        remove--;
      }
    }
  }
  return days.reduce((a, b) => a + b, 0);
}

export function speedBonus(crop: Crop, o: Pick<PlanOptions, "fertilizer" | "agriculturist">): number {
  let s = f(FERTILIZERS[o.fertilizer]);
  if (o.agriculturist) s = f(s + f(0.1));
  // paddy crops grow a quarter faster next to water, which is where they're meant to go
  if (crop.paddy) s = f(s + f(0.25));
  return s;
}

/** Days the crop can still grow from `day`: the rest of this season and any of its seasons after. */
export function daysLeft(crop: Crop, o: Pick<PlanOptions, "season" | "day" | "anySeason">): number {
  if (o.anySeason) return 28;
  if (!crop.seasons.includes(o.season)) return 0;
  let days = 28 - o.day;
  for (let i = SEASONS.indexOf(o.season) + 1; i < SEASONS.length && crop.seasons.includes(SEASONS[i]); i++) days += 28;
  return days;
}

export function plan(crop: Crop, o: PlanOptions): Plan | null {
  if (!crop.phases.length) return null;
  const days = growthDays(crop.phases, speedBonus(crop, o));
  const left = daysLeft(crop, o);
  let harvests = 0;
  let seeds = 0;
  if (days > 0 && left >= days) {
    if (crop.regrow > 0) {
      harvests = 1 + Math.floor((left - days) / crop.regrow);
      seeds = 1;
    } else {
      harvests = Math.floor(left / days);
      seeds = harvests;
    }
  }
  const max = Math.max(crop.minStack, crop.maxStack);
  // each extra roll keeps going while it hits, capped at 90% by the game
  const chance = Math.min(0.9, crop.extraChance);
  const perHarvest = (crop.minStack + max) / 2 + chance / (1 - chance);
  const price = o.tiller ? Math.floor(crop.price * 1.1) : crop.price;
  const income = harvests * perHarvest * price;
  const profit = income - seeds * crop.seedPrice;
  return { days, harvests, seeds, perHarvest, income, profit, perDay: left > 0 ? profit / left : 0 };
}
