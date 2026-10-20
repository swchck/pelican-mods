<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { saves, type SaveDiff, type SaveSide, type Snapshot } from "@/lib/api";
import { errorText } from "@/i18n";
import { relativeTime } from "@/lib/time";

const props = defineProps<{ saveId: string; snapshots: Snapshot[] }>();
const open = defineModel<boolean>("open", { required: true });
const { t, n, locale } = useI18n();

const from = ref("previous");
const to = ref("current");
const diff = ref<SaveDiff | null>(null);
const busy = ref(false);

const options = computed(() => [
  { value: "current", label: t("saveDiff.current") },
  { value: "previous", label: t("saveDiff.previous") },
  ...props.snapshots.map((s) => ({
    value: s.file,
    label: `${relativeTime(new Date(s.created), new Date(), locale.value)} · ${t(`saves.reason.${s.reason}`)}`,
  })),
]);

async function compare() {
  busy.value = true;
  try {
    diff.value = await saves.compare(props.saveId, from.value, to.value);
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    busy.value = false;
  }
}

watch(open, (on) => {
  if (on) {
    diff.value = null;
    void compare();
  }
});

const date = (s: SaveSide) => t("saves.date", { year: s.year, season: t(`saves.season.${s.season}`), day: s.day });
const signed = (v: number) => (v > 0 ? "+" : v < 0 ? "−" : "±") + n(Math.abs(v));
const numbers = computed(() => {
  const d = diff.value;
  if (!d) return [];
  const rows = [
    { label: t("saveDiff.money"), value: signed(d.to.money - d.from.money) },
    { label: t("saveDiff.earned"), value: signed(d.to.totalEarned - d.from.totalEarned) },
    {
      label: t("saveDiff.played"),
      value: t("saveDiff.minutes", { n: n(Math.max(0, Math.round((d.to.playTimeMs - d.from.playTimeMs) / 60000))) }),
    },
  ];
  if (d.from.perfection !== null && d.to.perfection !== null)
    rows.push({
      label: t("saveDiff.perfection"),
      value: `${n(d.from.perfection, { maximumFractionDigits: 1 })}% → ${n(d.to.perfection, { maximumFractionDigits: 1 })}%`,
    });
  return rows;
});
const quiet = computed(
  () =>
    diff.value &&
    !diff.value.skills.length &&
    !diff.value.friends.length &&
    !diff.value.firsts.length &&
    !diff.value.items.length,
);
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-h-[85vh] overflow-auto sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">{{ t("saveDiff.title") }}</DialogTitle>
        <DialogDescription>{{ t("saveDiff.hint") }}</DialogDescription>
      </DialogHeader>
      <div class="flex flex-wrap items-center gap-2 text-sm">
        <select
          v-model="from"
          :aria-label="t('saveDiff.from')"
          class="bg-card rounded-md border-2 border-[#a0703f] px-2 py-1"
        >
          <option v-for="o in options" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
        <span>→</span>
        <select
          v-model="to"
          :aria-label="t('saveDiff.to')"
          class="bg-card rounded-md border-2 border-[#a0703f] px-2 py-1"
        >
          <option v-for="o in options" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
        <Button size="sm" :disabled="busy" @click="compare">
          <Loader2Icon v-if="busy" class="animate-spin" />{{ t("saveDiff.compare") }}
        </Button>
      </div>

      <template v-if="diff">
        <p class="text-sm font-extrabold">{{ date(diff.from) }} → {{ date(diff.to) }}</p>
        <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
          <template v-for="r in numbers" :key="r.label">
            <dt class="text-muted-foreground font-semibold">{{ r.label }}</dt>
            <dd class="font-mono font-bold">{{ r.value }}</dd>
          </template>
        </dl>
        <p v-if="quiet" class="text-muted-foreground text-sm font-semibold">{{ t("saveDiff.nothing") }}</p>

        <section v-if="diff.skills.length" class="flex flex-col gap-1 text-sm">
          <h3 class="font-extrabold">{{ t("saveDiff.skills") }}</h3>
          <span v-for="s in diff.skills" :key="s.key">{{ s.name }}: {{ s.from }} → {{ s.to }}</span>
        </section>
        <section v-if="diff.friends.length" class="flex flex-col gap-1 text-sm">
          <h3 class="font-extrabold">{{ t("saveDiff.friends") }}</h3>
          <span v-for="f in diff.friends" :key="f.key">{{ f.name }}: {{ f.from }} → {{ f.to }} ♥</span>
        </section>
        <section v-for="f in diff.firsts" :key="f.kind" class="flex flex-col gap-1 text-sm">
          <h3 class="font-extrabold">{{ t(`saveDiff.firsts.${f.kind}`, { n: f.names.length }) }}</h3>
          <span class="text-muted-foreground">{{ f.names.join(", ") }}</span>
        </section>
        <section v-if="diff.items.length" class="flex flex-col gap-1 text-sm">
          <h3 class="font-extrabold">{{ t("saveDiff.items") }}</h3>
          <span v-for="i in diff.items" :key="i.key" class="flex gap-2">
            <span class="min-w-0 flex-1 truncate">{{ i.name }}</span>
            <span class="font-mono" :class="i.to > i.from ? 'text-success' : 'text-destructive'">{{
              signed(i.to - i.from)
            }}</span>
          </span>
        </section>
      </template>
    </DialogContent>
  </Dialog>
</template>
