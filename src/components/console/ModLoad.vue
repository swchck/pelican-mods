<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { GaugeIcon, Loader2Icon, SplitIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { companion as api, type PatchedMethod, type PatchReport, type HandlerTimes } from "@/lib/api";
import { errorText } from "@/i18n";
import { useCompanion } from "@/stores/companion";
import { useLibrary } from "@/stores/library";
import { packageTitle } from "@/lib/packages";

const SECONDS = 30;
const { t, n } = useI18n();
const link = useCompanion();
const lib = useLibrary();

const left = ref(0);
const profile = ref<HandlerTimes | null>(null);
const patches = ref<PatchReport | null>(null);
const loadingPatches = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

async function measure() {
  try {
    await api.profileStart();
  } catch (e) {
    toast.error(errorText(e));
    return;
  }
  profile.value = null;
  left.value = SECONDS;
  timer = setInterval(async () => {
    if (--left.value > 0) return;
    stopTimer();
    try {
      profile.value = await api.profileStop();
    } catch (e) {
      toast.error(errorText(e));
    }
  }, 1000);
}

function stopTimer() {
  if (timer) clearInterval(timer);
  timer = null;
  left.value = 0;
}

onUnmounted(() => {
  // leaving mid-measurement must not leave the game patched
  if (timer) {
    stopTimer();
    void api.profileStop().catch(() => {});
  }
});

const perSecond = (ms: number) => (profile.value?.seconds ? ms / profile.value.seconds : 0);
const top = computed(() => Math.max(...(profile.value?.mods.map((m) => m.totalMs) ?? [1]), 1));
// a frame at 60 fps has ~16.7 ms; a handler eating a few of them a second is felt
const heavy = (ms: number) => perSecond(ms) >= 50;

const names = computed(() => {
  const out = new Map<string, string>();
  for (const p of lib.packages)
    for (const m of p.mods) out.set(m.manifest.uniqueId.toLowerCase(), m.manifest.name || packageTitle(p));
  return out;
});
const owner = (id: string) => names.value.get(id.toLowerCase()) ?? id;
const risky = (m: PatchedMethod) => m.patches.some((p) => p.kind === "transpiler" || p.canSkip);
const sortedPatches = computed(() =>
  [...(patches.value?.shared ?? [])].sort((a, b) => Number(risky(b)) - Number(risky(a))),
);

async function loadPatches() {
  loadingPatches.value = true;
  try {
    patches.value = await api.patches();
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    loadingPatches.value = false;
  }
}
</script>

<template>
  <div class="flex flex-col gap-3">
    <p v-if="!link.connected" class="text-muted-foreground p-4 text-center text-sm font-semibold">
      {{ t("modLoad.needsGame") }}
    </p>
    <template v-else>
      <section class="wood-panel flex flex-col gap-2 p-3.5">
        <div class="flex flex-wrap items-center gap-2">
          <h2 class="flex-1 font-extrabold">{{ t("modLoad.title") }}</h2>
          <Button size="sm" :disabled="left > 0" @click="measure">
            <Loader2Icon v-if="left > 0" class="animate-spin" /><GaugeIcon v-else />{{
              left > 0 ? t("modLoad.measuring", { n: left }) : t("modLoad.measure", { n: SECONDS })
            }}
          </Button>
        </div>
        <p class="text-muted-foreground text-xs font-semibold">{{ t("modLoad.hint") }}</p>
        <p v-if="profile && !profile.mods.length" class="text-sm font-semibold">{{ t("modLoad.nothing") }}</p>
        <ul v-else-if="profile" class="flex flex-col gap-1.5 text-sm">
          <li v-for="m in profile.mods" :key="m.mod" class="flex flex-col gap-0.5">
            <div class="flex items-baseline gap-2">
              <span class="min-w-0 flex-1 truncate font-bold" :class="{ 'text-destructive': heavy(m.totalMs) }">{{
                m.mod
              }}</span>
              <span class="text-muted-foreground font-mono text-xs">
                {{ t("modLoad.perSecond", { ms: n(perSecond(m.totalMs), { maximumFractionDigits: 1 }) }) }} ·
                {{ t("modLoad.spike", { ms: n(m.maxMs, { maximumFractionDigits: 1 }) }) }}
              </span>
            </div>
            <div class="bg-parchment border-border-soft h-1.5 overflow-hidden rounded border">
              <div
                class="h-full"
                :class="heavy(m.totalMs) ? 'bg-destructive' : 'bg-gold'"
                :style="{ width: `${(m.totalMs / top) * 100}%` }"
              />
            </div>
            <span class="text-muted-foreground text-xs">
              {{
                m.events
                  .map((e) => t("modLoad.eventMs", { event: e.event, ms: n(e.totalMs, { maximumFractionDigits: 0 }) }))
                  .join(" · ")
              }}
            </span>
          </li>
        </ul>
      </section>

      <section class="wood-panel flex flex-col gap-2 p-3.5">
        <div class="flex flex-wrap items-center gap-2">
          <h2 class="flex-1 font-extrabold">{{ t("modLoad.patchesTitle") }}</h2>
          <Button size="sm" variant="outline" :disabled="loadingPatches" @click="loadPatches">
            <Loader2Icon v-if="loadingPatches" class="animate-spin" /><SplitIcon v-else />{{ t("modLoad.findPatches") }}
          </Button>
        </div>
        <p class="text-muted-foreground text-xs font-semibold">{{ t("modLoad.patchesHint") }}</p>
        <p v-if="patches" class="text-sm font-semibold">
          {{ t("modLoad.patchesFound", { patched: patches.patched, shared: patches.shared.length }) }}
        </p>
        <ul v-if="patches?.shared.length" class="flex flex-col gap-2 text-sm">
          <li v-for="m in sortedPatches" :key="m.method" class="flex flex-col gap-0.5">
            <code class="text-xs font-bold break-all" :class="{ 'text-destructive': risky(m) }">{{ m.method }}</code>
            <span class="flex flex-wrap gap-1.5">
              <span
                v-for="(p, i) in m.patches"
                :key="i"
                class="rounded border px-1.5 text-xs font-semibold"
                :class="
                  p.kind === 'transpiler' || p.canSkip
                    ? 'bg-warning-soft border-[#d98b76]'
                    : 'bg-parchment border-border-soft'
                "
                :title="p.canSkip ? t('modLoad.canSkip') : undefined"
              >
                {{ owner(p.owner) }} · {{ t(`modLoad.kind.${p.kind}`) }}{{ p.canSkip ? " ⤼" : "" }}
              </span>
            </span>
          </li>
        </ul>
      </section>
    </template>
  </div>
</template>
