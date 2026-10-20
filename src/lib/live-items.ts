import { computed, ref, watch } from "vue";
import { companion } from "@/lib/api";
import type { GrangeItem } from "@/lib/grange";
import { useCompanion } from "@/stores/companion";

/** Where the player keeps each item, by unqualified id, while a farm is loaded in the game. */
export function useLiveItems() {
  const link = useCompanion();
  const items = ref<GrangeItem[] | null>(null);
  const ready = computed(() => link.connected && link.status.state !== null);
  watch(
    ready,
    async (on) => {
      // the game answers "no_world" until a farm loads; nothing to show then
      items.value = on ? await companion.items().catch(() => null) : null;
    },
    { immediate: true },
  );
  const byId = computed(() => {
    const map = new Map<string, { where: string; stack: number }[]>();
    for (const i of items.value ?? []) {
      const id = i.id.replace(/^\(O\)/, "");
      const list = map.get(id) ?? [];
      const same = list.find((x) => x.where === i.where);
      if (same) same.stack += i.stack;
      else list.push({ where: i.where, stack: i.stack });
      map.set(id, list);
    }
    return map;
  });
  return { live: computed(() => items.value !== null), byId, items };
}
