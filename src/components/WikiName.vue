<script setup lang="ts">
import { computed } from "vue";
import { openWiki, wikiUrl } from "@/lib/wiki";

const props = defineProps<{ title: string | null | undefined }>();
const url = computed(() => wikiUrl(props.title));
</script>

<template>
  <a
    v-if="url"
    :href="url"
    class="decoration-wood-light underline decoration-dotted underline-offset-2 hover:decoration-solid"
    @click.prevent="openWiki(title)"
    ><slot
  /></a>
  <span v-else><slot /></span>
</template>
