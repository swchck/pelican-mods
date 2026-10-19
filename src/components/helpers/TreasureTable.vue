<script setup lang="ts">
import type { TreasureSeries } from "@/lib/api";

/** Upcoming results per container, one column each, numbered from `opened + 1`. */
defineProps<{ title: string; hint: string; opened: number; series: TreasureSeries[] }>();
</script>

<template>
  <section class="wood-panel flex flex-col gap-2 p-4">
    <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ title }}</h2>
    <p class="text-muted-foreground text-xs font-semibold">{{ hint }}</p>
    <div class="overflow-auto">
      <table class="w-full text-sm">
        <thead>
          <tr class="text-muted-foreground text-left text-xs">
            <th class="py-1 pr-3 font-bold">#</th>
            <th v-for="g in series" :key="g.id" class="py-1 pr-3 font-bold">{{ g.name }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(_, row) in series[0]?.next ?? []" :key="row" class="border-border-soft border-t border-dashed">
            <td class="text-muted-foreground py-1 pr-3 font-mono text-xs">{{ opened + row + 1 }}</td>
            <td v-for="g in series" :key="g.id" class="py-1 pr-3 font-semibold">
              <template v-if="g.next[row]">
                {{ g.next[row].name
                }}<span v-if="g.next[row].stack > 1" class="text-muted-foreground"> ×{{ g.next[row].stack }}</span>
              </template>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
