<script setup lang="ts">
defineProps<{ checked: boolean; disabled?: boolean; label: string }>();
defineEmits<{ toggle: [] }>();
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="checked"
    :aria-label="label"
    :disabled="disabled"
    class="flex h-5 w-9 shrink-0 items-center rounded border-2 px-0.5 transition-colors disabled:cursor-not-allowed disabled:opacity-60"
    :class="checked ? 'bg-success justify-end border-[#23491a]' : 'justify-start border-[#8c6e4a] bg-[#d8c4a0]'"
    @click.stop="$emit('toggle')"
  >
    <span class="bg-card size-3 rounded-[2px]" />
  </button>
</template>
