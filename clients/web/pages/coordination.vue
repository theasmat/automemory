<template>
  <div class="p-8 space-y-8 max-w-7xl mx-auto overflow-y-auto h-full">
    <div class="pb-4 border-b border-dark-700">
      <div class="flex items-center gap-2">
        <Users class="w-5 h-5 text-gold-500" />
        <h1 class="text-xl font-bold font-mono text-white">Agents &amp; Locks</h1>
      </div>
      <p class="text-xs font-mono text-gray-500 mt-1">
        Who holds which file, open planning rooms, and the latest handoff batons. Refreshes every 5 seconds.
      </p>
    </div>

    <section class="space-y-3">
      <h2 class="text-sm font-mono font-bold text-gray-300">Active leases ({{ data.leases.length }})</h2>
      <div v-if="!data.leases.length" class="text-xs font-mono text-gray-600 border border-dark-800 bg-dark-950 p-6">
        No leases held. Agents take one with <code>lock.acquire</code> before editing a shared file.
      </div>
      <table v-else class="w-full text-xs font-mono border border-dark-800">
        <thead class="bg-dark-900 text-gray-400 text-left">
          <tr><th class="p-2">Agent</th><th class="p-2">Scope</th><th class="p-2">Goal</th><th class="p-2">Left</th></tr>
        </thead>
        <tbody>
          <tr v-for="l in data.leases" :key="l.session_id + l.scope" class="border-t border-dark-800 text-gray-300">
            <td class="p-2">{{ l.agent }} <span class="text-gray-600">{{ l.session_id }}</span></td>
            <td class="p-2 text-gold-400">{{ l.scope }}</td>
            <td class="p-2">{{ l.goal }}</td>
            <td class="p-2">{{ l.remaining_seconds }}s</td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="space-y-3">
      <h2 class="text-sm font-mono font-bold text-gray-300">Planning rooms ({{ data.rooms.length }})</h2>
      <div v-if="!data.rooms.length" class="text-xs font-mono text-gray-600 border border-dark-800 bg-dark-950 p-6">
        No rooms. Agents open one with <code>room.join</code>.
      </div>
      <div v-for="r in data.rooms" :key="r.id" class="border border-dark-800 bg-dark-950 p-3 text-xs font-mono text-gray-300">
        <span class="text-white font-bold">{{ r.topic }}</span>
        <span class="ml-2 px-1.5 py-0.5 border border-dark-700 text-gold-400">{{ r.state }}</span>
        <span class="ml-2 text-gray-500">round {{ r.round }} · turn: {{ r.turn_holder || 'none' }}</span>
      </div>
    </section>

    <section class="space-y-3">
      <h2 class="text-sm font-mono font-bold text-gray-300">Handoff batons ({{ data.batons.length }})</h2>
      <div v-if="!data.batons.length" class="text-xs font-mono text-gray-600 border border-dark-800 bg-dark-950 p-6">
        No batons. An agent leaves one with <code>baton.pass</code>.
      </div>
      <div v-for="b in data.batons" :key="b.id" class="border border-dark-800 bg-dark-950 p-3 text-xs font-mono text-gray-300">
        <div class="text-gray-500 mb-1">{{ b.agent }} → {{ b.to_session || 'anyone' }} · {{ b.created_at }}</div>
        <pre class="whitespace-pre-wrap">{{ b.packet }}</pre>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { Users } from 'lucide-vue-next';

const data = ref<{ leases: any[]; rooms: any[]; batons: any[] }>({ leases: [], rooms: [], batons: [] });
let timer: ReturnType<typeof setInterval> | undefined;

async function load() {
  try {
    const res = await fetch('/api/coordination');
    if (res.ok) data.value = await res.json();
  } catch { /* daemon restarting; keep the last view */ }
}

onMounted(() => { load(); timer = setInterval(load, 5000); });
onUnmounted(() => { if (timer) clearInterval(timer); });
</script>
