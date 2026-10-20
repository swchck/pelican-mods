<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  ArrowUpCircleIcon,
  ChevronDownIcon,
  CircleAlertIcon,
  Loader2Icon,
  RefreshCwIcon,
  SearchCheckIcon,
  TriangleAlertIcon,
} from "@lucide/vue";
import { Button } from "@/components/ui/button";
import type { Crash, LogReport, PackageView, SkippedMod } from "@/lib/api";
import { packageCompat, packageUpdate } from "@/lib/packages";
import { splitLinks } from "@/lib/md-links";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relativeTime } from "@/lib/time";
import { useLibrary } from "@/stores/library";

const props = defineProps<{ report: LogReport; crash?: Crash | null }>();
const { t, locale } = useI18n();
const lib = useLibrary();
const open = ref<string | null>(null);

// the log names mods by their manifest Name
const packageOf = (s: SkippedMod): PackageView | undefined =>
  lib.packages.find((p) => p.mods.some((m) => m.manifest.name === s.name));
const updateOf = (s: SkippedMod) => {
  const p = packageOf(s);
  return p ? packageUpdate(p, lib.report) : null;
};
// no update to wait for: SMAPI skips these every launch until they're turned off
const stuck = (s: SkippedMod) => {
  const p = packageOf(s);
  return s.kind === "incompatible" && p?.enabled && !updateOf(s) ? p : null;
};
const advice = (s: SkippedMod) => {
  const p = packageOf(s);
  const summary = p ? packageCompat(p, lib.report)?.summary : null;
  return summary ? splitLinks(summary) : [];
};
// only the roots: mods skipped for a missing dependency load once it does
const fixable = computed(() =>
  props.report.skipped.filter((s) => s.kind !== "dependency" && updateOf(s)).map((s) => packageOf(s)!),
);
const busy = ref(false);
const router = useRouter();
async function findCulprit() {
  if (await lib.bisectStart(t("bisect.profileName"))) await router.push("/");
}
async function updateAll() {
  busy.value = true;
  try {
    for (const p of fixable.value) await lib.update(p);
  } finally {
    busy.value = false;
  }
}
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
        v-if="crash"
        class="bg-warning-soft text-destructive flex flex-col gap-1.5 rounded-md border-2 border-[#d98b76] p-3.5"
      >
        <b class="flex items-center gap-2"
          ><TriangleAlertIcon class="size-5" />{{
            crash.suspect ? t("crash.titleMod", { mod: crash.suspect }) : t("crash.title", { source: crash.source })
          }}</b
        >
        <span class="text-sm font-semibold break-words select-text">{{ crash.message }}</span>
        <pre
          v-if="crash.details.length"
          class="overflow-auto text-[11px] leading-snug whitespace-pre-wrap select-text"
          >{{ crash.details.join("\n") }}</pre>
        <span class="text-xs font-semibold">{{ t("crash.hint") }}</span>
        <Button
          v-if="!lib.view?.bisect"
          size="sm"
          variant="outline"
          class="self-start"
          :disabled="lib.working"
          @click="findCulprit"
          ><SearchCheckIcon />{{ t("bisect.start") }}</Button
        >
      </section>

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
        <div class="flex flex-wrap items-center gap-2">
          <h2 class="flex-1 font-extrabold">{{ t("logReport.skipped", { n: report.skipped.length }) }}</h2>
          <Button variant="outline" size="sm" :disabled="lib.checking" @click="lib.checkUpdates(true)">
            <Loader2Icon v-if="lib.checking" class="animate-spin" /><RefreshCwIcon v-else />{{ t("updates.check") }}
          </Button>
          <Button v-if="fixable.length" size="sm" :disabled="busy || lib.working" @click="updateAll">
            <Loader2Icon v-if="busy" class="animate-spin" /><ArrowUpCircleIcon v-else />{{
              t("logReport.updateRoots", { n: fixable.length })
            }}
          </Button>
        </div>
        <p class="text-muted-foreground text-xs font-semibold">{{ t("logReport.skippedHint") }}</p>
        <ul class="flex flex-col gap-1 text-sm">
          <li v-for="s in report.skipped" :key="s.name" class="flex flex-wrap items-baseline gap-x-2">
            <span class="font-bold">{{ s.name }}</span>
            <span class="text-muted-foreground font-mono text-xs">{{ s.version }}</span>
            <span class="text-muted-foreground text-xs font-semibold">
              {{ s.kind === "other" ? s.reason : t(`logReport.kind.${s.kind}`, { needs: s.needs ?? "" }) }}
            </span>
            <span v-if="s.kind !== 'dependency' && updateOf(s)" class="text-success text-xs font-bold">
              → {{ updateOf(s)!.version }}
            </span>
            <span
              v-else-if="s.kind === 'incompatible' && !packageOf(s)"
              class="text-muted-foreground text-xs font-semibold italic"
            >
              {{ t("logReport.notInLibrary") }}
            </span>
            <span v-if="advice(s).length" class="text-xs font-semibold">
              <template v-for="(seg, i) in advice(s)" :key="i">
                <a
                  v-if="seg.url"
                  :href="seg.url"
                  class="underline decoration-dotted underline-offset-2"
                  @click.prevent="openUrl(seg.url)"
                  >{{ seg.text }}</a
                >
                <template v-else>{{ seg.text }}</template>
              </template>
            </span>
            <Button
              v-if="stuck(s)"
              variant="outline"
              size="xs"
              class="ml-auto"
              :disabled="lib.working"
              :title="t('logReport.turnOffHint')"
              @click="lib.toggle(stuck(s)!.id, false)"
            >
              {{ t("logReport.turnOff") }}
            </Button>
          </li>
        </ul>
      </section>

      <section v-if="report.startupSecs !== null" class="wood-panel flex flex-col gap-1.5 p-3.5 text-sm">
        <h2 class="font-extrabold">{{ t("logReport.startup", { n: report.startupSecs }) }}</h2>
        <template v-if="report.slow.length">
          <span class="text-muted-foreground text-xs font-semibold">{{ t("logReport.slowHint") }}</span>
          <span v-for="s in report.slow" :key="s.name" class="flex justify-between gap-2">
            <span class="font-semibold">{{ s.name }}</span>
            <span class="text-muted-foreground font-mono text-xs">{{ t("logReport.secs", { n: s.secs }) }}</span>
          </span>
        </template>
        <template v-if="report.heavy.length">
          <span class="text-muted-foreground mt-1 text-xs font-semibold">{{ t("logReport.heavyHint") }}</span>
          <span v-for="h in report.heavy" :key="h.name" class="flex justify-between gap-2">
            <span class="font-semibold">{{ h.name }}</span>
            <span class="text-muted-foreground font-mono text-xs">{{ t("logReport.assets", { n: h.assets }) }}</span>
          </span>
        </template>
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
            class="break-words whitespace-pre-line"
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
