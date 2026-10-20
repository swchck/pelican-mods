<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { SearchCheckIcon, XIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { packageTitle } from "@/lib/packages";
import { useGame } from "@/stores/game";
import { useLibrary } from "@/stores/library";

const { t } = useI18n();
const lib = useLibrary();
const game = useGame();

const search = computed(() => lib.view?.bisect ?? null);
const title = (id: string) => {
  const p = lib.packages.find((x) => x.id === id);
  return p ? packageTitle(p) : id;
};
const result = computed(() => (lib.bisectResult?.kind === "found" ? lib.bisectResult : null));
const found = computed(() => result.value?.package ?? null);
const foundOn = computed(() => (found.value ? lib.packages.find((p) => p.id === found.value)?.enabled : false));

async function turnOff() {
  if (found.value && (await lib.toggle(found.value, false))) lib.bisectResult = null;
}
</script>

<template>
  <section
    v-if="search"
    class="bg-parchment flex flex-col gap-2 rounded-md border-2 border-dashed border-[#a0703f] p-3 text-sm"
    :aria-label="t('bisect.title')"
  >
    <div class="flex flex-wrap items-center gap-2">
      <SearchCheckIcon class="text-wood size-5 shrink-0" />
      <b class="flex-1">{{ t("bisect.round", { n: search.round }) }}</b>
      <Button size="sm" variant="ghost" :disabled="lib.working || game.running" @click="lib.bisectCancel()">
        <XIcon />{{ t("bisect.cancel") }}
      </Button>
    </div>
    <p v-if="search.alone" class="font-semibold">{{ t("bisect.alone", { name: title(search.pinned[0]) }) }}</p>
    <p v-else class="font-semibold">
      <template v-if="search.pinned.length">{{ t("bisect.pinned", { name: title(search.pinned[0]) }) }} </template>
      {{ t("bisect.status", { on: search.testing.length, suspects: search.suspects.length }) }}
      <template v-if="search.cleared.length">{{ t("bisect.cleared", { n: search.cleared.length }) }}</template>
    </p>
    <p class="text-muted-foreground text-xs font-semibold">{{ t("bisect.hint") }}</p>
    <p v-if="game.crash" class="text-destructive text-xs font-bold">{{ t("bisect.crashed") }}</p>
    <div class="flex flex-wrap gap-2">
      <Button size="sm" variant="destructive" :disabled="lib.working || game.running" @click="lib.bisectAnswer(true)">
        {{ t("bisect.problem") }}
      </Button>
      <Button size="sm" :disabled="lib.working || game.running" @click="lib.bisectAnswer(false)">
        {{ t("bisect.fine") }}
      </Button>
    </div>
  </section>

  <section
    v-else-if="lib.bisectResult"
    class="bg-parchment flex flex-wrap items-center gap-3 rounded-md border-2 border-[#a0703f] p-3 text-sm"
  >
    <SearchCheckIcon class="text-wood size-5 shrink-0" />
    <span class="flex-1 font-bold">
      <template v-if="result?.with">{{
        t("bisect.pair", { name: title(result.package), with: title(result.with) })
      }}</template>
      <template v-else>{{ found ? t("bisect.found", { name: title(found) }) : t("bisect.clean") }}</template>
    </span>
    <Button
      v-if="result && !result.with && result.cleared.length"
      size="sm"
      variant="outline"
      :disabled="lib.working || game.running"
      @click="lib.bisectPartner(t('bisect.profileName'), result.package, result.cleared)"
      >{{ t("bisect.partner") }}</Button
    >
    <Button v-if="found && foundOn" size="sm" :disabled="lib.working" @click="turnOff">{{
      t("bisect.turnOff")
    }}</Button>
    <Button size="icon-sm" variant="ghost" :aria-label="t('bisect.dismiss')" @click="lib.bisectResult = null">
      <XIcon />
    </Button>
  </section>
</template>
