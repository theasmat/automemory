<template>
  <!-- eslint-disable-next-line vue/no-v-html -- the text is HTML-escaped before it is formatted -->
  <div class="prose-am" v-html="html" />
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{ text: string }>();

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const inline = (s: string) =>
  esc(s)
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>');

/** A tiny, safe markdown subset: headings, bullet lists, bold, inline code, paragraphs. */
const html = computed(() => {
  const out: string[] = [];
  let list = false;
  const closeList = () => {
    if (list) out.push('</ul>');
    list = false;
  };
  for (const raw of (props.text || '').split('\n')) {
    const line = raw.trimEnd();
    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    const li = /^\s*[-*]\s+(.*)$/.exec(line);
    if (h) {
      closeList();
      const level = Math.min(h[1].length + 1, 4);
      out.push(`<h${level}>${inline(h[2])}</h${level}>`);
    } else if (li) {
      if (!list) out.push('<ul>');
      list = true;
      out.push(`<li>${inline(li[1])}</li>`);
    } else if (line.trim() === '') {
      closeList();
    } else {
      closeList();
      out.push(`<p>${inline(line)}</p>`);
    }
  }
  closeList();
  return out.join('');
});
</script>

<style scoped>
.prose-am :deep(h2) { font-size: 20px; font-weight: 600; margin: 4px 0 8px; }
.prose-am :deep(h3) { font-size: 17px; font-weight: 600; margin: 22px 0 6px; }
.prose-am :deep(h4) { font-size: 15px; font-weight: 600; margin: 16px 0 4px; }
.prose-am :deep(p) { color: var(--text-muted); font-size: 15px; line-height: 1.6; margin: 6px 0; }
.prose-am :deep(ul) { margin: 6px 0; padding-left: 20px; list-style: disc; }
.prose-am :deep(li) { color: var(--text); font-size: 15px; line-height: 1.6; margin: 3px 0; }
.prose-am :deep(code) { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 13px; background: var(--surface-2); padding: 1px 6px; border-radius: 6px; }
</style>
