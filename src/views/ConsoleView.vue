<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { SendIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import LogReportView from "@/components/console/LogReport.vue";
import { api, type LogLevel, type LogReport } from "@/lib/api";
import { errorText } from "@/i18n";
import { useGame } from "@/stores/game";
import { useShortcut } from "@/stores/shortcuts";

const { t } = useI18n();
const game = useGame();
const route = useRoute();
const router = useRouter();

const tab = computed({
  get: () => (route.query.tab === "report" ? "report" : "log"),
  set: (v: string) => void router.replace({ query: { ...route.query, tab: v } }),
});
const report = ref<LogReport | null>(null);
async function loadReport() {
  try {
    report.value = await api.logReport();
  } catch (e) {
    toast.error(errorText(e));
  }
}
onMounted(loadReport);
// SMAPI finishes the log when the game closes
watch(
  () => game.running,
  (running, was) => {
    if (was && !running) void loadReport();
  },
);
const box = ref<HTMLElement>();
const follow = ref(true);
const command = ref("");
const commandInput = ref<InstanceType<typeof Input>>();
useShortcut("page.search", () => (commandInput.value?.$el as HTMLInputElement | undefined)?.focus());

// SMAPI's own console hides TRACE too; it is most of the file
const showTrace = ref(false);
const lines = computed(() => (showTrace.value ? game.log : game.log.filter((l) => l.level !== "trace")));

const levelClass: Record<LogLevel, string> = {
  error: "text-[#ff8a75]",
  warn: "text-[#f2c14e]",
  alert: "text-[#d6a8ff]",
  debug: "text-[#a89276]",
  trace: "text-[#7f6c55]",
  info: "text-[#f3e4c3]",
};

// by the newest line, not the count: once the buffer is full the count stops changing
watch(
  () => lines.value[lines.value.length - 1]?.seq,
  async () => {
    if (!follow.value) return;
    await nextTick();
    box.value?.scrollTo({ top: box.value.scrollHeight });
  },
);

async function send() {
  const line = command.value.trim();
  if (!line) return;
  try {
    await api.console(line);
    command.value = "";
  } catch (e) {
    toast.error(errorText(e));
  }
}
</script>

<template>
  <div class="flex h-full flex-col gap-3">
    <Tabs v-model="tab" class="flex min-h-0 flex-1 flex-col gap-3">
      <header class="flex flex-wrap items-center gap-4">
        <h1 class="font-pixel text-wood-dark flex-1 text-3xl font-bold">{{ t("console.title") }}</h1>
        <TabsList>
          <TabsTrigger value="log">{{ t("console.tabLog") }}</TabsTrigger>
          <TabsTrigger value="report">
            {{ t("console.tabReport") }}
            <span
              v-if="report && (report.skipped.length || report.lostLocations.length)"
              class="bg-destructive size-2 rounded-full"
            />
          </TabsTrigger>
        </TabsList>
      </header>

      <TabsContent value="log" class="flex min-h-0 flex-1 flex-col gap-3">
        <div class="flex items-center justify-end gap-4">
          <div class="flex items-center gap-2">
            <Switch id="trace" v-model="showTrace" />
            <Label for="trace" class="font-bold">{{ t("console.trace") }}</Label>
          </div>
          <div class="flex items-center gap-2">
            <Switch id="follow" v-model="follow" />
            <Label for="follow" class="font-bold">{{ t("console.follow") }}</Label>
          </div>
          <Button variant="outline" size="sm" @click="game.log = []">{{ t("console.clear") }}</Button>
        </div>

        <div
          ref="box"
          class="border-wood-dark min-h-0 flex-1 overflow-auto rounded-[10px] border-[3px] bg-[#2b1a0e] p-3 font-mono text-xs leading-5 shadow-[inset_0_0_0_3px_#4e2c14] select-text"
          role="log"
          aria-live="polite"
        >
          <p v-if="!lines.length" class="text-[#a89276]">{{ t("console.empty") }}</p>
          <div v-for="l in lines" :key="l.seq" class="break-all whitespace-pre-wrap" :class="levelClass[l.level]">
            {{ l.text }}
          </div>
        </div>

        <form class="flex gap-2" @submit.prevent="send">
          <Input
            ref="commandInput"
            v-model="command"
            class="bg-card border-input h-10 border-2 font-mono"
            :placeholder="t('console.placeholder')"
            :disabled="!game.running"
            :aria-label="t('console.placeholder')"
          />
          <Button type="submit" size="lg" :disabled="!game.running || !command.trim()">
            <SendIcon />{{ t("console.send") }}
          </Button>
        </form>
      </TabsContent>

      <TabsContent value="report" class="min-h-0 flex-1 overflow-auto">
        <LogReportView v-if="report" :report="report" />
      </TabsContent>
    </Tabs>
  </div>
</template>
