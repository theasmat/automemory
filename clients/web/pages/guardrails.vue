<template>
  <div class="p-8 space-y-8 max-w-7xl mx-auto overflow-y-auto h-full font-mono">
    <!-- Header -->
    <div class="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2">
          <ShieldAlert class="w-5 h-5 text-gold-500" />
          <h1 class="text-xl font-bold text-white">Safety Guardrails & Pre-Action Judgment</h1>
          <span
            class="px-2 py-0.5 text-xs uppercase font-bold border"
            :class="getModeBadgeClass(store.currentMode)"
          >
            {{ store.currentMode }} Mode Active
          </span>
        </div>
        <p class="text-xs text-gray-500 mt-1">
          Zero-latency pre-action judgment gate intercepting catastrophic commands, DB drops, and known failure repetitions.
        </p>
      </div>

      <!-- Mode Selector Buttons -->
      <div class="flex items-center bg-dark-900 border border-dark-700 p-1">
        <button
          v-for="m in modes"
          :key="m.id"
          @click="store.setMode(m.id, store.activeProject || undefined)"
          class="px-3 py-1.5 text-xs transition-all flex items-center gap-1.5"
          :class="[
            store.currentMode === m.id
              ? `${m.activeClass} font-bold shadow-sm`
              : 'text-gray-400 hover:text-white'
          ]"
        >
          <span>{{ m.icon }}</span>
          <span>{{ m.label }}</span>
        </button>
      </div>
    </div>

    <!-- Mode Behavior Explanation Grid -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div
        v-for="m in modes"
        :key="m.id"
        class="p-4 border transition-all"
        :class="[
          store.currentMode === m.id
            ? 'bg-dark-900 border-gold-500/60 ring-1 ring-gold-500/30'
            : 'bg-dark-950 border-dark-800 opacity-60'
        ]"
      >
        <div class="flex items-center justify-between pb-2 border-b border-dark-800">
          <span class="font-bold text-white flex items-center gap-1.5 text-xs">
            <span>{{ m.icon }}</span>
            <span>{{ m.label }} Mode</span>
          </span>
          <span v-if="store.currentMode === m.id" class="text-[10px] px-1.5 py-0.2 bg-gold-500 text-black font-bold">
            ACTIVE
          </span>
        </div>
        <p class="text-[11px] text-gray-300 mt-2 leading-relaxed">
          {{ m.longDescription }}
        </p>
        <div class="mt-3 pt-2 border-t border-dark-800 text-[10px] text-gray-400">
          <strong>Pre-Tool Interception:</strong> {{ m.interception }}
        </div>
      </div>
    </div>

    <!-- Interactive Pre-Action Judgment Playground -->
    <div class="p-6 bg-dark-900 border border-dark-700 space-y-5 shadow-xl">
      <div class="flex items-center justify-between pb-3 border-b border-dark-700 flex-wrap gap-2">
        <div class="flex items-center gap-2">
          <Terminal class="w-4 h-4 text-gold-500" />
          <h2 class="text-sm font-bold text-white uppercase tracking-wider">
            Pre-Action Judgment Tester (Live Gate Simulator)
          </h2>
        </div>
        <span class="text-[11px] text-gray-500">
          Simulates <code>automemory hook --event pre-tool</code> evaluation
        </span>
      </div>

      <!-- Quick Preset Buttons -->
      <div class="space-y-1.5">
        <div class="text-[10px] text-gray-500 uppercase">Test Dangerous & Standard Presets:</div>
        <div class="flex items-center gap-2 flex-wrap">
          <button
            v-for="p in testPresets"
            :key="p.name"
            @click="applyPreset(p)"
            class="px-2.5 py-1 bg-dark-950 hover:bg-dark-800 border border-dark-700 text-[11px] text-gray-300 transition-colors flex items-center gap-1"
          >
            <span>{{ p.icon }}</span>
            <span>{{ p.name }}</span>
          </button>
        </div>
      </div>

      <!-- Input Form -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div>
          <label class="block text-[10px] text-gray-500 uppercase mb-1">Tool Name</label>
          <input
            v-model="testToolName"
            type="text"
            class="w-full bg-dark-950 border border-dark-700 p-2 text-xs text-white placeholder-gray-600 focus:outline-none focus:border-gold-500"
          />
        </div>

        <div class="md:col-span-2">
          <label class="block text-[10px] text-gray-500 uppercase mb-1">Command Line / Target File</label>
          <div class="flex gap-2">
            <input
              v-model="testCommandLine"
              type="text"
              class="flex-1 bg-dark-950 border border-dark-700 p-2 text-xs text-white placeholder-gray-600 focus:outline-none focus:border-gold-500"
            />
            <button
              @click="runEvaluation"
              :disabled="isEvaluating"
              class="px-4 py-2 bg-gold-500 hover:bg-gold-400 text-black font-bold text-xs transition-colors shrink-0 shadow-sm shadow-gold-500/20 disabled:opacity-50"
            >
              {{ isEvaluating ? 'Evaluating...' : 'Evaluate Gate' }}
            </button>
          </div>
        </div>
      </div>

      <!-- Live Evaluation Result Card -->
      <div
        v-if="evaluationResult"
        class="p-4 border transition-all space-y-3"
        :class="getDecisionCardClass(evaluationResult.action)"
      >
        <div class="flex items-center justify-between pb-2 border-b border-dark-700 flex-wrap gap-2">
          <div class="flex items-center gap-2">
            <span
              class="px-2 py-0.5 text-xs uppercase font-extrabold"
              :class="getDecisionBadgeClass(evaluationResult.action)"
            >
              DECISION: {{ evaluationResult.action }}
            </span>
            <span class="text-xs font-bold text-white">{{ evaluationResult.title }}</span>
          </div>

          <div class="flex items-center gap-3 text-xs">
            <span class="text-gray-400">Risk Score: <strong :class="getRiskScoreColor(evaluationResult.risk_score)">{{ (evaluationResult.risk_score * 100).toFixed(0) }}%</strong></span>
            <span v-if="evaluationResult.requires_confirmation" class="px-2 py-0.5 bg-red-950 text-red-400 border border-red-800 text-[10px] font-bold">
              REQUIRES CONFIRMATION
            </span>
          </div>
        </div>

        <p class="text-xs text-gray-300 leading-relaxed">
          {{ evaluationResult.risk_summary }}
        </p>

        <div v-if="evaluationResult.mitigation_hint" class="p-2.5 bg-dark-950 border border-dark-800 text-xs text-gold-300 flex items-start gap-1.5">
          <Lightbulb class="w-3.5 h-3.5 text-gold-500 shrink-0 mt-0.5" />
          <span><strong>Mitigation:</strong> {{ evaluationResult.mitigation_hint }}</span>
        </div>
      </div>
    </div>

    <!-- Active Guardrail Policies Catalog -->
    <div class="space-y-4">
      <div class="flex items-center justify-between pb-2 border-b border-dark-700">
        <div class="flex items-center gap-2">
          <ShieldCheck class="w-4 h-4 text-emerald-400" />
          <h2 class="text-sm font-bold text-white uppercase tracking-wider">
            Active Built-in Guardrail Policies ({{ store.guardrailRules.length }})
          </h2>
        </div>
        <span class="text-xs text-gray-500">Evaluated on every pre-tool hook</span>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div
          v-for="rule in store.guardrailRules"
          :key="rule.id"
          class="p-4 bg-dark-900 border border-dark-700 space-y-2.5 flex flex-col justify-between"
        >
          <div>
            <div class="flex items-center justify-between pb-2 border-b border-dark-800">
              <span class="font-bold text-white text-xs">{{ rule.name }}</span>
              <span
                class="px-1.5 py-0.2 text-[10px] uppercase font-bold"
                :class="rule.is_critical ? 'bg-red-950 text-red-400 border border-red-800' : 'bg-amber-950 text-amber-400 border border-amber-800'"
              >
                {{ rule.is_critical ? 'Critical Stop' : 'High Risk' }}
              </span>
            </div>
            <p class="text-[11px] text-gray-300 mt-2">{{ rule.description }}</p>
          </div>

          <div class="space-y-1.5 pt-2 border-t border-dark-800 text-[10px]">
            <div class="text-gray-500 flex items-center justify-between">
              <span>Regex Pattern:</span>
              <code class="text-cyan-400 bg-dark-950 px-1 py-0.2 border border-dark-800 truncate max-w-[200px]">{{ rule.pattern }}</code>
            </div>
            <div class="text-gray-400">
              <span class="text-gray-600">Mitigation: </span>{{ rule.mitigation_hint }}
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { ShieldAlert, ShieldCheck, Terminal, Lightbulb } from 'lucide-vue-next';

const store = useAutoMemoryStore();

const testToolName = ref('run_command');
const testCommandLine = ref('rm -rf /');
const isEvaluating = ref(false);
const evaluationResult = ref<any>(null);

const modes = [
  {
    id: 'vibe' as const,
    label: 'Vibe',
    icon: '⚡',
    activeClass: 'bg-gold-500 text-black',
    longDescription: 'High developer velocity. Non-critical warnings are surfaced as non-blocking inline notifications. Only catastrophic actions halt tool execution.',
    interception: 'Blocks destructive root deletions and production force pushes; logs inline warnings for migrations.',
  },
  {
    id: 'strict' as const,
    label: 'Strict',
    icon: '🔒',
    activeClass: 'bg-red-950 text-red-300 border border-red-800',
    longDescription: 'High safety mode. Database migrations, destructive git resets, cloud teardowns, and known failure repetitions require explicit approval.',
    interception: 'Blocks and requires confirmation on all high/medium risk actions before tools run.',
  },
  {
    id: 'review' as const,
    label: 'Review',
    icon: '🔍',
    activeClass: 'bg-cyan-950 text-cyan-300 border border-cyan-800',
    longDescription: 'Proactive session briefing. Surfaces relevant past decisions, known failure records, and architectural constraints before task execution starts.',
    interception: 'Warns on high risk and proactively briefs the developer on similar past executions.',
  },
];

const testPresets = [
  { name: 'rm -rf /', icon: '🛑', tool: 'run_command', cmd: 'rm -rf /' },
  { name: 'prisma migrate', icon: '🔒', tool: 'run_command', cmd: 'prisma migrate deploy' },
  { name: 'force push prod', icon: '🛑', tool: 'run_command', cmd: 'git push -f origin main' },
  { name: 'git reset --hard', icon: '⚠️', tool: 'run_command', cmd: 'git reset --hard HEAD~1' },
  { name: 'cargo test', icon: '✅', tool: 'run_command', cmd: 'cargo test --workspace' },
];

const applyPreset = (preset: typeof testPresets[0]) => {
  testToolName.value = preset.tool;
  testCommandLine.value = preset.cmd;
  runEvaluation();
};

const runEvaluation = async () => {
  isEvaluating.value = true;
  try {
    const res = await store.evaluateGuardrail(testToolName.value, {
      CommandLine: testCommandLine.value,
      command: testCommandLine.value,
    });
    evaluationResult.value = res;
  } finally {
    isEvaluating.value = false;
  }
};

const getModeBadgeClass = (mode: string) => {
  switch (mode.toLowerCase()) {
    case 'vibe':
      return 'bg-gold-500/20 text-gold-400 border-gold-500/40';
    case 'strict':
      return 'bg-red-950 text-red-400 border-red-800';
    case 'review':
      return 'bg-cyan-950 text-cyan-400 border-cyan-800';
    default:
      return 'bg-dark-800 text-gray-400 border-dark-700';
  }
};

const getDecisionCardClass = (action: string) => {
  switch (action?.toLowerCase()) {
    case 'block':
      return 'bg-red-950/20 border-red-900/80';
    case 'warn':
      return 'bg-amber-950/20 border-amber-900/80';
    case 'allow':
    default:
      return 'bg-emerald-950/20 border-emerald-900/80';
  }
};

const getDecisionBadgeClass = (action: string) => {
  switch (action?.toLowerCase()) {
    case 'block':
      return 'bg-red-950 text-red-300 border border-red-800';
    case 'warn':
      return 'bg-amber-950 text-amber-300 border border-amber-800';
    case 'allow':
    default:
      return 'bg-emerald-950 text-emerald-300 border border-emerald-800';
  }
};

const getRiskScoreColor = (score: number) => {
  if (score >= 0.7) return 'text-red-400';
  if (score >= 0.4) return 'text-amber-400';
  return 'text-emerald-400';
};

onMounted(() => {
  store.fetchGuardrailRules();
  runEvaluation();
});
</script>
