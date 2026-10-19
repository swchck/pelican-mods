import { computed, onMounted, ref } from "vue";
import { saves, type SaveSummary } from "@/lib/api";
import { SEASONS, seasonKey, type Season } from "@/lib/game-clock";
import { useCompanion } from "@/stores/companion";

/** The in-game date the helpers plan around. */
export interface FarmDate {
  season: Season;
  day: number;
  /** HHMM; known only while the game runs. */
  time: number | null;
  /** "sun", "rain", "storm" or "snow"; known only while the game runs. */
  weather: string | null;
  /** The save it belongs to, when one matches. */
  saveId: string | null;
  live: boolean;
}

/** The running game's date, or else the most recently played save's. */
export function useFarmDate() {
  const companion = useCompanion();
  const list = ref<SaveSummary[]>([]);
  onMounted(async () => {
    list.value = [...(await saves.list().catch(() => []))].sort((a, b) => b.modified - a.modified);
  });

  return computed<FarmDate | null>(() => {
    const state = companion.status.state;
    if (state) {
      const season = seasonKey(state.season);
      if (season) {
        const save = list.value.find((s) => s.farmer === state.farmer && s.farmName === state.farm);
        return {
          season,
          day: state.day,
          time: state.time,
          weather: state.weather || null,
          saveId: save?.id ?? null,
          live: true,
        };
      }
    }
    const last = list.value[0];
    if (!last) return null;
    return {
      season: SEASONS[last.season] ?? "spring",
      day: last.day,
      time: null,
      weather: null,
      saveId: last.id,
      live: false,
    };
  });
}
