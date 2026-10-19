<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import PixelToggle from "@/components/PixelToggle.vue";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { choiceValues, type Field, type Json } from "@/lib/config-form";

const props = defineProps<{ field: Field; value: Json }>();
const emit = defineEmits<{ change: [value: Json] }>();
const { t } = useI18n();

const id = computed(() => `cfg-${props.field.path.join("-")}`);
const text = computed(() => {
  const v = props.value;
  if (props.field.kind === "list" && Array.isArray(v)) return v.join(", ");
  if (props.field.kind === "json") return JSON.stringify(v, null, 2);
  return v === null || v === undefined ? "" : String(v);
});
const selected = computed(() => choiceValues(props.value));

function onText(raw: string | number) {
  const s = String(raw);
  switch (props.field.kind) {
    case "number": {
      const n = props.field.integer ? Number.parseInt(s, 10) : Number.parseFloat(s);
      if (!Number.isNaN(n)) emit("change", n);
      return;
    }
    case "list":
      emit(
        "change",
        s
          .split(",")
          .map((p) => p.trim())
          .filter(Boolean),
      );
      return;
    case "json":
      try {
        emit("change", JSON.parse(s) as Json);
      } catch {
        // keep the last valid value until the JSON parses again
      }
      return;
    default:
      emit("change", s);
  }
}

// content patcher wrote this value; keep its type (bool stays bool, list stays a string)
function onChoice(option: string) {
  if (props.field.kind !== "choice") return;
  if (!props.field.multiple) {
    emit("change", typeof props.value === "boolean" ? option === "true" : option);
    return;
  }
  const next = selected.value.includes(option)
    ? selected.value.filter((o) => o !== option)
    : [...selected.value, option];
  emit("change", next.join(", "));
}
</script>

<template>
  <div class="flex flex-col gap-1.5 py-2" :class="{ 'col-span-full': field.kind === 'json' }">
    <div v-if="field.kind === 'bool'" class="flex items-center justify-between gap-3">
      <span class="flex flex-col">
        <span :id="id" class="text-sm font-extrabold">{{ field.label }}</span>
        <span v-if="field.description" class="text-muted-foreground text-xs">{{ field.description }}</span>
      </span>
      <PixelToggle
        :checked="value === true || value === 'true'"
        :label="field.label"
        @toggle="emit('change', !(value === true || value === 'true'))"
      />
    </div>

    <template v-else>
      <label :for="id" class="text-sm font-extrabold">{{ field.label }}</label>
      <span v-if="field.description" class="text-muted-foreground -mt-1 text-xs leading-snug">
        {{ field.description }}
      </span>

      <div
        v-if="field.kind === 'choice' && field.multiple"
        class="flex flex-wrap gap-1.5"
        role="group"
        :aria-labelledby="id"
      >
        <button
          v-for="o in field.options"
          :key="o"
          type="button"
          :aria-pressed="selected.includes(o)"
          class="rounded-md border-2 px-2.5 py-1 text-xs font-extrabold"
          :class="selected.includes(o) ? 'bg-gold border-wood-dark' : 'bg-parchment border-[#a0703f]'"
          @click="onChoice(o)"
        >
          {{ field.optionLabels?.[o] ?? o }}
        </button>
      </div>
      <Select
        v-else-if="field.kind === 'choice'"
        :model-value="selected[0] ?? ''"
        @update:model-value="(v) => onChoice(String(v))"
      >
        <SelectTrigger :id="id" class="bg-parchment border-input h-9 w-full max-w-72 border-2 font-bold">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="o in field.options" :key="o" :value="o">{{ field.optionLabels?.[o] ?? o }}</SelectItem>
        </SelectContent>
      </Select>

      <textarea
        v-else-if="field.kind === 'json'"
        :id="id"
        :value="text"
        rows="5"
        spellcheck="false"
        class="bg-parchment border-input w-full rounded-md border-2 p-2 font-mono text-xs"
        @change="onText(($event.target as HTMLTextAreaElement).value)"
      />
      <Input
        v-else
        :id="id"
        :model-value="text"
        :type="field.kind === 'number' ? 'number' : 'text'"
        :step="field.kind === 'number' && !field.integer ? 'any' : undefined"
        class="bg-parchment border-input h-9 border-2"
        :class="{
          'font-mono': field.kind === 'keybind' || field.kind === 'number',
          'max-w-40': field.kind === 'number',
        }"
        @update:model-value="onText"
      />
      <span v-if="field.kind === 'list'" class="text-muted-foreground text-xs">{{ t("config.listHint") }}</span>
    </template>
  </div>
</template>
