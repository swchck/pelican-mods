<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "vue-sonner";
import {
  ArrowUpCircleIcon,
  CircleAlertIcon,
  ExternalLinkIcon,
  FolderIcon,
  Loader2Icon,
  SlidersHorizontalIcon,
  Trash2Icon,
  Undo2Icon,
} from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { library, type Issue, type PackageView } from "@/lib/api";
import DependencyList from "@/components/library/DependencyList.vue";
import { splitLinks, type Segment } from "@/lib/md-links";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useNexus } from "@/stores/nexus";
import {
  packageAuthor,
  packageCompat,
  packageIssues,
  packageTitle,
  packageUpdate,
  packageVersion,
} from "@/lib/packages";
import { errorText } from "@/i18n";
import { useLibrary } from "@/stores/library";

const props = defineProps<{ pkg: PackageView }>();
const { t } = useI18n();
const lib = useLibrary();
const confirmRemove = ref(false);
const router = useRouter();
const nx = useNexus();
// mods with settings first; a multi-part package opens its first configurable part
const configTarget = computed(() =>
  props.pkg.system ? null : (props.pkg.mods.find((m) => m.hasConfig || m.hasSchema) ?? props.pkg.mods[0] ?? null),
);

const single = computed(() => (props.pkg.mods.length === 1 ? props.pkg.mods[0] : null));
const description = computed(() => single.value?.manifest.description ?? "");
const update = computed(() => packageUpdate(props.pkg, lib.report));
const compat = computed(() => packageCompat(props.pkg, lib.report));
const busy = computed(() => lib.updating.has(props.pkg.id));
const compatSegments = computed(() => (compat.value?.summary ? splitLinks(` ${compat.value.summary}`) : []));

// a Nexus mod opens in the app's own browser tab; anything else in the system browser
function openSegment(seg: Segment) {
  if (seg.nexusModId) {
    void nx.select(seg.nexusModId);
    void router.push("/browse");
  } else if (seg.url) {
    void openUrl(seg.url);
  }
}
const requiredBy = computed(() => [...new Set(props.pkg.mods.flatMap((m) => m.requiredBy))]);

const tags = computed(() => {
  const m = single.value?.manifest;
  const out: { text: string; tone: "green" | "clay" }[] = [];
  if (props.pkg.system) out.push({ text: t("library.system"), tone: "green" });
  if (m?.minimumApiVersion) out.push({ text: t("library.needsSmapi", { v: m.minimumApiVersion }), tone: "green" });
  if (m?.minimumGameVersion) out.push({ text: t("library.needsGame", { v: m.minimumGameVersion }), tone: "green" });
  if (m?.contentPackFor)
    out.push({ text: t("library.contentPackFor", { id: m.contentPackFor.uniqueId }), tone: "green" });
  for (const key of m?.updateKeys ?? []) out.push({ text: key, tone: "clay" });
  return out;
});

function issueText(i: Issue): string {
  if (i.kind === "missingDependency")
    return t("issues.missingDependency", { id: i.id, min: i.min ? ` ${i.min}+` : "" });
  return t(`issues.${i.kind}`, { ...i });
}

async function openFolder() {
  try {
    await library.open(props.pkg.path);
  } catch (e) {
    toast.error(errorText(e));
  }
}

async function remove() {
  confirmRemove.value = false;
  await lib.remove(props.pkg.id);
}
</script>

<template>
  <aside
    :aria-label="packageTitle(pkg)"
    class="wood-panel flex w-[330px] shrink-0 flex-col gap-3 overflow-auto p-[18px]"
  >
    <div class="flex h-24 shrink-0 items-end overflow-hidden rounded-md border-2 border-[#a0703f] bg-[#cfe6f5]">
      <div class="grass-strip h-9 w-full border-t-4 border-[#3f7f2a]" />
    </div>

    <div class="flex flex-col gap-0.5">
      <h2 class="font-pixel text-wood-dark text-[22px] leading-tight font-bold break-words">{{ packageTitle(pkg) }}</h2>
      <span class="text-muted-foreground text-[13px] font-bold">
        {{ packageAuthor(pkg) }}<template v-if="packageVersion(pkg)"> · {{ packageVersion(pkg) }}</template>
      </span>
    </div>

    <p v-if="description" class="text-sm leading-relaxed select-text">{{ description }}</p>

    <div v-if="tags.length" class="flex flex-wrap gap-1.5">
      <span
        v-for="tag in tags"
        :key="tag.text"
        class="rounded border-2 px-2 py-0.5 text-xs font-bold"
        :class="
          tag.tone === 'green'
            ? 'bg-success-soft border-[#9cc07a] text-[#2e5a1e]'
            : 'border-[#e2b07a] bg-[#fbe9d0] text-[#7a3e12]'
        "
      >
        {{ tag.text }}
      </span>
    </div>

    <div
      v-if="update"
      class="border-border-soft bg-parchment flex flex-col gap-2 rounded-md border-2 border-dashed p-2.5 text-[13px]"
    >
      <span class="font-extrabold">{{ t("updates.available", { version: update.version }) }}</span>
      <span v-if="!update.direct" class="text-muted-foreground text-xs font-semibold">{{ t("updates.manual") }}</span>
      <Button size="sm" class="self-start" :disabled="busy" @click="lib.update(pkg)">
        <Loader2Icon v-if="busy" class="animate-spin" />
        <ArrowUpCircleIcon v-else-if="update.direct" />
        <ExternalLinkIcon v-else />
        {{ update.direct ? t("updates.update") : t("updates.openPage") }}
      </Button>
    </div>

    <p
      v-if="compat"
      class="bg-warning-soft text-destructive rounded-md border-2 border-[#d98b76] p-2.5 text-[13px] font-semibold break-words"
    >
      <b>{{ t(`updates.compat.${compat.status}`) }}</b>
      <template v-for="(seg, i) in compatSegments" :key="i">
        <a
          v-if="seg.url"
          :href="seg.url"
          class="underline decoration-dotted underline-offset-2 hover:text-[#4a1a10]"
          @click.prevent="openSegment(seg)"
          >{{ seg.text }}</a
        >
        <template v-else>{{ seg.text }}</template>
      </template>
    </p>

    <ul
      v-if="pkg.enabled && packageIssues(pkg).length"
      class="bg-warning-soft flex flex-col gap-1.5 rounded-md border-2 border-[#d98b76] p-2.5 text-[13px] font-semibold"
    >
      <li v-for="(issue, i) in packageIssues(pkg)" :key="i" class="text-destructive flex gap-1.5">
        <CircleAlertIcon class="mt-0.5 size-3.5 shrink-0" />{{ issueText(issue) }}
      </li>
    </ul>
    <ul
      v-if="pkg.broken.length"
      class="bg-warning-soft flex flex-col gap-1.5 rounded-md border-2 border-[#d98b76] p-2.5 text-[13px] font-semibold"
    >
      <li v-for="b in pkg.broken" :key="b.relPath" class="text-destructive break-words">
        <b v-if="b.relPath">{{ b.relPath }}: </b>{{ t("library.brokenManifest", { error: b.error }) }}
      </li>
    </ul>

    <DependencyList v-if="single?.manifest.dependencies.length" :deps="single.manifest.dependencies" />

    <div v-if="pkg.mods.length > 1" class="flex flex-col gap-1 text-[13px]">
      <span class="font-extrabold">{{ t("library.parts") }}</span>
      <span v-for="m in pkg.mods" :key="m.relPath" class="flex justify-between gap-2">
        <span class="truncate">{{ m.manifest.name }}</span>
        <span class="text-muted-foreground font-mono text-xs">{{ m.manifest.version }}</span>
      </span>
    </div>

    <p v-if="requiredBy.length" class="text-muted-foreground text-xs font-semibold">
      {{ t("library.requiredBy", { list: requiredBy.join(", ") }) }}
    </p>

    <div class="flex-1" />
    <Button
      v-if="pkg.backupVersion && pkg.mods[0]"
      variant="outline"
      :disabled="lib.working"
      @click="lib.rollback(pkg.mods[0].manifest.uniqueId)"
    >
      <Undo2Icon />{{ t("updates.rollback", { version: pkg.backupVersion }) }}
    </Button>
    <Button
      v-if="configTarget"
      variant="outline"
      @click="router.push({ path: '/config', query: { pkg: pkg.id, rel: configTarget.relPath } })"
    >
      <SlidersHorizontalIcon />{{ t("config.open") }}
    </Button>
    <div class="grid grid-cols-2 gap-2">
      <Button variant="outline" @click="openFolder"><FolderIcon />{{ t("library.openFolder") }}</Button>
      <Button v-if="!pkg.system" variant="destructive" :disabled="lib.working" @click="confirmRemove = true">
        <Trash2Icon />{{ t("library.remove") }}
      </Button>
    </div>

    <Dialog v-model:open="confirmRemove">
      <DialogContent>
        <DialogHeader>
          <DialogTitle class="font-pixel text-xl">{{
            t("library.removeTitle", { name: packageTitle(pkg) })
          }}</DialogTitle>
          <DialogDescription>{{ t("library.removeText") }}</DialogDescription>
        </DialogHeader>
        <p v-if="requiredBy.length" class="text-destructive text-sm font-bold">
          {{ t("library.removeWarn", { list: requiredBy.join(", ") }) }}
        </p>
        <DialogFooter>
          <DialogClose as-child>
            <Button variant="outline">{{ t("common.cancel") }}</Button>
          </DialogClose>
          <Button variant="destructive" @click="remove">{{ t("library.remove") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </aside>
</template>
