<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { saves, isAppError, type FarmMap, type MapFeature } from "@/lib/api";
import { useFarmDate } from "@/lib/farm-date";

const { t } = useI18n();
const date = useFarmDate();
const PLACES = ["Farm", "Greenhouse", "IslandWest"] as const;
const place = ref<(typeof PLACES)[number]>("Farm");
const map = ref<FarmMap | null>(null);
const missing = ref(false);
const canvas = ref<HTMLCanvasElement | null>(null);
const hovered = ref<{ f: MapFeature; left: number; top: number } | null>(null);

watch(
  [() => date.value?.saveId, place],
  async ([id]) => {
    map.value = null;
    missing.value = false;
    if (!id) return;
    try {
      map.value = await saves.map(id, place.value);
    } catch (e) {
      if (isAppError(e) && e.kind === "not_found") missing.value = true;
    }
  },
  { immediate: true },
);

const COLORS: Record<MapFeature["kind"], string> = {
  building: "#a0703f",
  crop: "#5b8c32",
  deadCrop: "#7a6a5a",
  dirt: "#b58a5a",
  tree: "#2f6b2a",
  fruitTree: "#3c8a3c",
  bush: "#4f8a3a",
  grass: "#b9d27c",
  flooring: "#d8c4a0",
  clump: "#8a8a8a",
  debris: "#9a8a6a",
  chest: "#e09a2a",
  sprinkler: "#4f7fb0",
  scarecrow: "#c0503a",
  machine: "#6a6a6a",
  fence: "#7a5230",
  object: "#b0a090",
};
// ground first, so what stands on it stays visible
const LAYER: Partial<Record<MapFeature["kind"], number>> = { grass: 0, flooring: 0, dirt: 1, building: 3 };

const ordered = computed(() =>
  [...(map.value?.features ?? [])].sort((a, b) => (LAYER[a.kind] ?? 2) - (LAYER[b.kind] ?? 2)),
);

const counts = computed(() => {
  const fs = map.value?.features ?? [];
  return {
    ready: fs.filter((f) => f.kind === "crop" && f.days === 0).length,
    dry: fs.filter((f) => f.kind === "crop" && !f.watered && f.days !== 0).length,
    machines: fs.filter((f) => f.kind === "machine" && f.ready).length,
  };
});

function draw() {
  const c = canvas.value;
  const m = map.value;
  if (!c || !m) return;
  // drawn at the screen's own resolution: a small canvas scaled up blurs the labels
  const TILE = Math.max(4, (c.clientWidth * window.devicePixelRatio) / m.width);
  c.width = Math.round(m.width * TILE);
  c.height = Math.round(m.height * TILE);
  const g = c.getContext("2d");
  if (!g) return;
  g.clearRect(0, 0, c.width, c.height);
  for (const f of ordered.value) {
    const [x, y, w, h] = [f.x * TILE, f.y * TILE, f.w * TILE, f.h * TILE];
    g.fillStyle = f.kind === "dirt" && f.watered ? "#6b4a2a" : COLORS[f.kind];
    if (f.kind === "crop") {
      g.fillStyle = f.watered ? "#6b4a2a" : COLORS.dirt;
      g.fillRect(x, y, w, h);
      g.fillStyle = f.days === 0 ? "#e8b82a" : COLORS.crop;
      g.beginPath();
      g.arc(x + w / 2, y + h / 2, TILE * 0.35, 0, Math.PI * 2);
      g.fill();
    } else if (f.kind === "tree" || f.kind === "fruitTree" || f.kind === "bush") {
      g.beginPath();
      g.arc(x + w / 2, y + h / 2, (Math.min(w, h) / 2) * 0.9, 0, Math.PI * 2);
      g.fill();
    } else if (f.kind === "debris") {
      g.fillRect(x + TILE * 0.3, y + TILE * 0.3, TILE * 0.4, TILE * 0.4);
    } else {
      g.fillRect(x, y, w, h);
    }
    if (f.kind === "building") {
      g.strokeStyle = "#4e2c14";
      g.lineWidth = 2;
      g.strokeRect(x + 1, y + 1, w - 2, h - 2);
      g.fillStyle = "#fff6e0";
      g.font = `bold ${Math.round(TILE)}px sans-serif`;
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.fillText(f.name, x + w / 2, y + h / 2, w - 4);
    }
    if (f.kind === "machine" && f.ready) {
      g.strokeStyle = "#e8b82a";
      g.lineWidth = 2;
      g.strokeRect(x + 1, y + 1, w - 2, h - 2);
    }
  }
}
watch([map, canvas], draw, { flush: "post" });

function onMove(e: MouseEvent) {
  const c = canvas.value;
  const m = map.value;
  if (!c || !m) return;
  const r = c.getBoundingClientRect();
  const tx = Math.floor(((e.clientX - r.left) / r.width) * m.width);
  const ty = Math.floor(((e.clientY - r.top) / r.height) * m.height);
  const hit = [...ordered.value]
    .reverse()
    .find((f) => f.kind !== "grass" && tx >= f.x && tx < f.x + f.w && ty >= f.y && ty < f.y + f.h);
  hovered.value = hit ? { f: hit, left: e.clientX - r.left + 12, top: e.clientY - r.top + 12 } : null;
}

function describe(f: MapFeature): string {
  const parts = [f.name || t(`mapTab.kinds.${f.kind}`), `(${f.x}, ${f.y})`];
  if (f.kind === "crop") {
    parts.push(f.days === 0 ? t("mapTab.ready") : t("mapTab.daysLeft", { n: f.days ?? 0 }, f.days ?? 0));
    if (!f.watered && f.days !== 0) parts.push(t("mapTab.dry"));
  }
  if (f.kind === "machine" && f.ready) parts.push(t("mapTab.machineReady"));
  return parts.join(" · ");
}

const LEGEND = ["building", "crop", "dirt", "tree", "chest", "machine", "sprinkler", "scarecrow", "debris"] as const;

let resize: (() => void) | null = null;
onMounted(() => window.addEventListener("resize", (resize = draw)));
onBeforeUnmount(() => resize && window.removeEventListener("resize", resize));
</script>

<template>
  <div class="flex flex-col gap-3">
    <p v-if="!date?.saveId" class="text-muted-foreground p-4 text-center text-sm font-semibold">
      {{ t("museumTab.noSave") }}
    </p>
    <template v-else>
      <div class="flex flex-wrap items-center gap-3">
        <div class="bg-parchment border-border-soft flex rounded-md border-2 p-0.5">
          <button
            v-for="p in PLACES"
            :key="p"
            type="button"
            class="rounded px-3 py-1 text-sm font-bold"
            :class="place === p ? 'bg-gold' : 'hover:bg-[#fbefd4]'"
            :aria-pressed="place === p"
            @click="place = p"
          >
            {{ t(`mapTab.places.${p}`) }}
          </button>
        </div>
        <span v-if="map" class="text-sm font-bold">
          {{ t("mapTab.summary", counts) }}
        </span>
      </div>
      <p class="text-muted-foreground text-xs font-semibold">{{ t("mapTab.hint") }}</p>
      <p v-if="missing" class="text-muted-foreground p-4 text-center text-sm font-semibold">
        {{ t("mapTab.missing") }}
      </p>
      <div v-else-if="map" class="relative">
        <canvas
          ref="canvas"
          class="w-full rounded-md border-2 border-[#a0703f] bg-[#8fb35a]"
          :aria-label="t(`mapTab.places.${place}`)"
          role="img"
          @mousemove="onMove"
          @mouseleave="hovered = null"
        />
        <div
          v-if="hovered"
          class="bg-card pointer-events-none absolute z-10 rounded border-2 border-[#4e2c14] px-2 py-1 text-xs font-bold whitespace-nowrap"
          :style="{ left: `${hovered.left}px`, top: `${hovered.top}px` }"
        >
          {{ describe(hovered.f) }}
        </div>
      </div>
      <div class="flex flex-wrap gap-x-4 gap-y-1 text-xs font-semibold">
        <span v-for="k in LEGEND" :key="k" class="flex items-center gap-1.5">
          <span class="size-3 rounded-sm border border-[#4e2c14]" :style="{ background: COLORS[k] }" />
          {{ t(`mapTab.kinds.${k}`) }}
        </span>
        <span class="flex items-center gap-1.5">
          <span class="size-3 rounded-full border border-[#4e2c14] bg-[#e8b82a]" />{{ t("mapTab.ready") }}
        </span>
      </div>
    </template>
  </div>
</template>
