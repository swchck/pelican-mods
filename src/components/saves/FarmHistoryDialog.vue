<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { saves, type FarmPoint } from "@/lib/api";
import { errorText } from "@/i18n";
import { SEASONS } from "@/lib/game-clock";

const props = defineProps<{ saveId: string }>();
const open = defineModel<boolean>("open", { required: true });
const { t, n } = useI18n();

const points = ref<FarmPoint[] | null>(null);
const metric = ref<"totalEarned" | "money" | "perfection" | "skills">("totalEarned");

watch(
  [open, () => props.saveId],
  async ([isOpen]) => {
    if (!isOpen) return;
    points.value = null;
    try {
      points.value = await saves.history(props.saveId);
    } catch (e) {
      toast.error(errorText(e));
    }
  },
  { immediate: true },
);

const dayIndex = (p: FarmPoint) => (p.year - 1) * 112 + p.season * 28 + p.day;
const dateText = (p: FarmPoint) =>
  t("farmHistory.date", { day: p.day, season: t(`calendar.season.${SEASONS[p.season]}`), year: p.year });
const value = (p: FarmPoint) => (metric.value === "perfection" ? p.perfection : p[metric.value]);

const W = 640;
const H = 240;
const PAD = 8;
const chart = computed(() => {
  const shown = (points.value ?? []).filter((p) => value(p) !== null);
  if (shown.length < 2) return null;
  const xs = shown.map(dayIndex);
  const ys = shown.map((p) => value(p) as number);
  const [x0, x1] = [Math.min(...xs), Math.max(...xs)];
  const [y0, y1] = [Math.min(...ys), Math.max(...ys)];
  const x = (v: number) => PAD + ((v - x0) / Math.max(1, x1 - x0)) * (W - 2 * PAD);
  const y = (v: number) => H - PAD - ((v - y0) / Math.max(1e-9, y1 - y0)) * (H - 2 * PAD);
  const dots = shown.map((p, i) => ({ x: x(xs[i]), y: y(ys[i]), label: `${dateText(p)}: ${format(ys[i])}` }));
  return {
    path: dots.map((d, i) => `${i ? "L" : "M"}${d.x.toFixed(1)},${d.y.toFixed(1)}`).join(" "),
    dots,
    first: shown[0],
    last: shown[shown.length - 1],
    min: y0,
    max: y1,
  };
});
const format = (v: number) => (metric.value === "perfection" ? `${n(Math.round(v * 10) / 10)}%` : n(v));
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">{{ t("farmHistory.title") }}</DialogTitle>
        <DialogDescription>{{ t("farmHistory.hint") }}</DialogDescription>
      </DialogHeader>
      <div class="bg-parchment border-border-soft flex w-fit flex-wrap rounded-md border-2 p-0.5">
        <button
          v-for="m in ['totalEarned', 'money', 'perfection', 'skills'] as const"
          :key="m"
          type="button"
          class="rounded px-3 py-1 text-sm font-bold"
          :class="metric === m ? 'bg-gold' : 'hover:bg-[#fbefd4]'"
          :aria-pressed="metric === m"
          @click="metric = m"
        >
          {{ t(`farmHistory.metric.${m}`) }}
        </button>
      </div>
      <p v-if="points && !chart" class="text-muted-foreground p-6 text-center text-sm font-semibold">
        {{ t("farmHistory.tooFew") }}
      </p>
      <figure v-else-if="chart" class="flex flex-col gap-1">
        <div class="flex justify-between font-mono text-xs font-bold">
          <span>{{ format(chart.max) }}</span>
          <span class="text-muted-foreground">{{ t("farmHistory.min", { v: format(chart.min) }) }}</span>
        </div>
        <svg
          :viewBox="`0 0 ${W} ${H}`"
          class="bg-card w-full rounded-md border-2 border-[#a0703f]"
          role="img"
          :aria-label="t(`farmHistory.metric.${metric}`)"
        >
          <path :d="chart.path" fill="none" stroke="#5b8c32" stroke-width="2.5" stroke-linejoin="round" />
          <circle v-for="(d, i) in chart.dots" :key="i" :cx="d.x" :cy="d.y" r="3.5" fill="#d9a520" stroke="#4e2c14">
            <title>{{ d.label }}</title>
          </circle>
        </svg>
        <figcaption class="text-muted-foreground flex justify-between text-xs font-semibold">
          <span>{{ dateText(chart.first) }}</span>
          <span>{{ dateText(chart.last) }}</span>
        </figcaption>
      </figure>
    </DialogContent>
  </Dialog>
</template>
