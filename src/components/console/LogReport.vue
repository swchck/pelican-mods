<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDownIcon, CircleAlertIcon, TriangleAlertIcon } from "@lucide/vue";
import type { LogReport } from "@/lib/api";
import { relativeTime } from "@/lib/time";

defineProps<{ report: LogReport }>();
const { t, locale } = useI18n();
const open = ref<string | null>(null);
</script>

<template>
  <div class="flex flex-col gap-3">
    <p v-if="!report.written" class="text-muted-foreground p-4 text-center text-sm font-semibold">
      {{ t("logReport.noLog") }}
    </p>
    <template v-else>
      <p class="text-muted-foreground text-xs font-bold">
        {{ t("logReport.from", { time: relativeTime(new Date(report.written * 1000), new Date(), locale) }) }}
      </p>

      <section
        v-if="report.lostLocations.length"
        class="bg-warning-soft text-destructive flex gap-3 rounded-md border-2 border-[#d98b76] p-3.5"
      >
        <TriangleAlertIcon class="mt-0.5 size-5 shrink-0" />
        <div class="flex flex-col gap-1 text-sm font-semibold">
          <b>{{ t("logReport.lostTitle", { n: report.lostLocations.length }) }}</b>
          <span>{{ t("logReport.lostText") }}</span>
          <span v-if="report.lostNpcs.length" class="text-xs">
            {{ t("logReport.lostNpcs", { list: report.lostNpcs.join(", ") }) }}
          </span>
        </div>
      </section>

      <section v-if="report.skipped.length" class="wood-panel flex flex-col gap-2 p-3.5">
        <h2 class="font-extrabold">{{ t("logReport.skipped", { n: report.skipped.length }) }}</h2>
        <p class="text-muted-foreground text-xs font-semibold">{{ t("logReport.skippedHint") }}</p>
        <ul class="flex flex-col gap-1 text-sm">
          <li v-for="s in report.skipped" :key="s.name" class="flex flex-wrap items-baseline gap-x-2">
            <span class="font-bold">{{ s.name }}</span>
            <span class="text-muted-foreground font-mono text-xs">{{ s.version }}</span>
            <span class="text-muted-foreground text-xs font-semibold">
              {{ s.kind === "other" ? s.reason : t(`logReport.kind.${s.kind}`, { needs: s.needs ?? "" }) }}
            </span>
          </li>
        </ul>
      </section>

      <section v-for="m in report.mods" :key="m.source" class="wood-panel flex flex-col gap-2 p-3.5">
        <button
          type="button"
          class="flex items-center gap-2 text-left"
          :aria-expanded="open === m.source"
          @click="open = open === m.source ? null : m.source"
        >
          <CircleAlertIcon class="size-4 shrink-0" :class="m.errors ? 'text-destructive' : 'text-gold-shade'" />
          <span class="flex-1 font-extrabold">{{ m.source === "game" ? t("logReport.game") : m.source }}</span>
          <span class="text-muted-foreground font-mono text-xs font-bold">
            {{ t("logReport.counts", { errors: m.errors, warnings: m.warnings }) }}
          </span>
          <ChevronDownIcon
            class="text-muted-foreground size-4 transition-transform"
            :class="{ 'rotate-180': open === m.source }"
          />
        </button>
        <ul v-if="open === m.source" class="flex flex-col gap-1.5 font-mono text-xs select-text">
          <li
            v-for="(e, i) in m.entries"
            :key="i"
            class="break-words"
            :class="e.level === 'error' ? 'text-destructive' : 'text-muted-foreground'"
          >
            <span v-if="e.count > 1" class="font-bold">×{{ e.count }} </span>{{ e.text }}
          </li>
        </ul>
      </section>

      <p
        v-if="!report.skipped.length && !report.lostLocations.length && !report.mods.length"
        class="text-success p-4 text-center text-sm font-bold"
      >
        {{ t("logReport.clean") }}
      </p>
    </template>
  </div>
</template>
