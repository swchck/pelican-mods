<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  CircleAlertIcon,
  CircleCheckIcon,
  CircleMinusIcon,
  CircleXIcon,
  DownloadIcon,
  Loader2Icon,
  SearchIcon,
} from "@lucide/vue";
import { Button } from "@/components/ui/button";
import type { Dependency } from "@/lib/api";
import { resolveDeps, type DepRow } from "@/lib/deps";
import { useLibrary } from "@/stores/library";
import { useNexus } from "@/stores/nexus";

const props = defineProps<{ deps: Dependency[] }>();
const { t } = useI18n();
const lib = useLibrary();
const nx = useNexus();
const router = useRouter();
const showOptional = ref(false);

const rows = computed(() => resolveDeps(props.deps, lib.packages));
const required = computed(() => rows.value.filter((r) => r.required));
const optional = computed(() => rows.value.filter((r) => !r.required));
const missingRequired = computed(() => required.value.filter((r) => r.state !== "ok").length);

const icon = { ok: CircleCheckIcon, off: CircleMinusIcon, old: CircleAlertIcon, missing: CircleXIcon };

function tone(r: DepRow): string {
  if (r.state === "ok") return "text-success";
  if (!r.required) return "text-muted-foreground";
  return r.state === "missing" ? "text-destructive" : "text-[#b57f17]";
}

function find(r: DepRow) {
  nx.query = r.name;
  void router.push("/browse");
}
</script>

<template>
  <div class="flex flex-col gap-1.5 text-[13px]">
    <div class="flex items-center justify-between">
      <span class="font-extrabold">{{ t("deps.title") }}</span>
      <span
        v-if="required.length"
        class="text-xs font-bold"
        :class="missingRequired ? 'text-destructive' : 'text-success'"
      >
        {{ missingRequired ? t("deps.problems", { n: missingRequired }) : t("deps.allGood") }}
      </span>
    </div>

    <template v-for="group in [required, showOptional ? optional : []]" :key="group === required ? 'req' : 'opt'">
      <div
        v-for="r in group"
        :key="r.uniqueId"
        class="bg-parchment border-border-soft flex items-center gap-2 rounded-md border-2 px-2 py-1.5"
        :class="{ 'cursor-pointer hover:bg-[#fbe3a8]': r.packageId }"
        :role="r.packageId ? 'button' : undefined"
        :tabindex="r.packageId ? 0 : undefined"
        @click="r.packageId && (lib.selectedId = r.packageId)"
        @keydown.enter="r.packageId && (lib.selectedId = r.packageId)"
      >
        <component :is="icon[r.state]" class="size-4 shrink-0" :class="tone(r)" />
        <div class="flex min-w-0 flex-1 flex-col">
          <span class="truncate font-bold">{{ r.name }}</span>
          <span class="text-muted-foreground truncate text-[11px]">
            {{ t(`deps.state.${r.state}`, { have: r.installedVersion ?? "" })
            }}<template v-if="r.minimumVersion"> · {{ t("deps.needs", { v: r.minimumVersion }) }}</template>
          </span>
        </div>
        <Button
          v-if="r.state === 'off' && r.packageId"
          size="xs"
          variant="outline"
          :disabled="lib.working"
          @click.stop="lib.toggle(r.packageId, true)"
        >
          {{ t("deps.enable") }}
        </Button>
        <Button
          v-else-if="r.state === 'missing' || r.state === 'old'"
          size="xs"
          variant="outline"
          :disabled="lib.working || lib.updating.has(r.uniqueId)"
          :title="t('deps.installHint')"
          @click.stop="lib.installDependency(r.uniqueId, r.name)"
        >
          <Loader2Icon v-if="lib.updating.has(r.uniqueId)" class="animate-spin" /><DownloadIcon v-else />{{
            t("deps.install")
          }}
        </Button>
        <Button
          v-if="r.state === 'missing' || r.state === 'old'"
          size="xs"
          variant="ghost"
          :aria-label="t('deps.find')"
          :title="t('deps.find')"
          @click.stop="find(r)"
        >
          <SearchIcon />
        </Button>
      </div>
    </template>

    <button
      v-if="optional.length"
      type="button"
      class="text-muted-foreground self-start text-xs font-bold underline-offset-2 hover:underline"
      @click="showOptional = !showOptional"
    >
      {{ showOptional ? t("deps.hideOptional") : t("deps.showOptional", { n: optional.length }) }}
    </button>
  </div>
</template>
