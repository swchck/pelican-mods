<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
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
import { packageTitle } from "@/lib/packages";
import { useLibrary } from "@/stores/library";

const { t } = useI18n();
const lib = useLibrary();
const pkg = computed(() => lib.packages.find((p) => p.id === lib.pendingDisable?.id) ?? null);
</script>

<template>
  <Dialog :open="lib.pendingDisable !== null" @update:open="(v: boolean) => !v && (lib.pendingDisable = null)">
    <DialogContent v-if="lib.pendingDisable">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">
          {{ t("library.disableTitle", { name: pkg ? packageTitle(pkg) : lib.pendingDisable.id }) }}
        </DialogTitle>
        <DialogDescription>{{ t("library.usageText") }}</DialogDescription>
      </DialogHeader>
      <ul class="flex flex-col gap-1 text-sm font-semibold">
        <li v-for="u in lib.pendingDisable.usage" :key="u.saveId">
          {{ u.farmer }} · {{ u.farmName }}:
          <template v-if="u.locationCount">{{
            t("library.usageLocations", { n: u.locationCount, list: u.locations.join(", ") })
          }}</template>
          <template v-if="u.locationCount && u.references">; </template>
          <template v-if="u.references">{{ t("library.usageRefs", { n: u.references }) }}</template>
        </li>
      </ul>
      <DialogFooter>
        <DialogClose as-child>
          <Button variant="outline">{{ t("library.keepOn") }}</Button>
        </DialogClose>
        <Button variant="destructive" @click="lib.confirmDisable()">{{ t("library.disableAnyway") }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
