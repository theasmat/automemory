<template>
  <div class="flex h-full bg-black overflow-hidden select-none">
    <!-- Left Session Selector Sidebar -->
    <div class="w-80 border-r border-dark-700 bg-dark-950 flex flex-col justify-between shrink-0 h-full">
      <div>
        <div class="p-4 border-b border-dark-700 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <History class="w-4 h-4 text-gold-500" />
            <span class="text-xs font-mono font-bold text-white uppercase tracking-wider">Recorded Sessions</span>
          </div>
          <span class="text-[10px] font-mono px-1.5 py-0.5 bg-dark-850 text-gray-400 border border-dark-700">
            {{ store.sessions.length }}
          </span>
        </div>

        <div class="overflow-y-auto max-h-[calc(100vh-10rem)] p-2 space-y-1">
          <div v-if="store.sessions.length === 0" class="text-center py-12 text-gray-600 font-mono text-xs">
            No recorded sessions yet. Execute tasks to record event streams!
          </div>

          <button
            v-for="s in store.sessions"
            :key="s.session_id"
            @click="store.selectSession(s.session_id)"
            class="w-full text-left p-3 border transition-all duration-150 font-mono text-xs space-y-1.5"
            :class="[
              store.activeSessionId === s.session_id
                ? 'bg-dark-850 border-gold-500/60 text-white ring-1 ring-gold-500/20'
                : 'bg-dark-900 border-dark-700 text-gray-400 hover:text-white hover:bg-dark-850'
            ]"
          >
            <div class="flex items-center justify-between">
              <span class="font-bold truncate max-w-[170px]" :class="store.activeSessionId === s.session_id ? 'text-gold-400' : 'text-gray-300'">
                {{ s.session_id }}
              </span>
              <span class="text-[10px] px-1 bg-dark-950 border border-dark-800 text-gray-500 uppercase">
                {{ s.adapter_id || 'agy' }}
              </span>
            </div>

            <div class="flex items-center justify-between text-[10px] text-gray-500">
              <span class="flex items-center gap-1">
                <Activity class="w-3 h-3 text-cyan-400" /> {{ s.event_count }} events
              </span>
              <span v-if="s.error_count > 0" class="text-red-400 font-bold flex items-center gap-0.5">
                <AlertCircle class="w-3 h-3" /> {{ s.error_count }} err
              </span>
            </div>
          </button>
        </div>
      </div>
    </div>

    <!-- Right Replay & Distillation Stage -->
    <div class="flex-1 flex flex-col h-full overflow-hidden bg-dark-950">
      <!-- Empty State -->
      <div v-if="!store.activeSessionId || store.sessionEvents.length === 0" class="flex-1 flex flex-col items-center justify-center p-8 text-center">
        <History class="w-12 h-12 text-gray-700 mb-3" />
        <div class="text-sm font-mono text-gray-400 font-semibold">Select a session from the left to begin replay</div>
        <p class="text-xs font-mono text-gray-600 mt-1 max-w-sm">
          Scrub through tool actions, inspect diffs, test variable playback speeds, and generate 100-to-1 trajectory distillation summaries.
        </p>
      </div>

      <div v-else class="flex-1 flex flex-col h-full overflow-hidden">
        <!-- Replay Control Bar & Token Reduction Header -->
        <div class="p-4 border-b border-dark-700 bg-dark-900 flex flex-col gap-3 shrink-0">
          <!-- Token Reduction Ribbon for Active Session -->
          <div class="flex flex-wrap items-center justify-between gap-2 pb-2 border-b border-dark-800 text-[11px] font-mono">
            <div class="flex items-center gap-3 flex-wrap">
              <span class="text-gray-400">
                Raw Session: <strong class="text-gold-400">~{{ formatTokens(totalSessionTokens) }} tokens</strong>
              </span>
              <span class="text-gray-600">•</span>
              <span class="text-gray-400">
                Distilled Memory: <strong class="text-cyan-300">~{{ formatTokens(distilledMemoryTokens) }} tokens</strong>
              </span>
              <span class="text-gray-600">•</span>
              <span class="px-2 py-0.5 bg-emerald-950 text-emerald-300 border border-emerald-800/80 font-bold">
                {{ tokenReductionStats.reductionPercentage }}% Token Reduction (~{{ formatTokens(tokenReductionStats.savedTokens) }} saved)
              </span>
            </div>

            <div class="text-[10px] text-gray-500">
              Est. Context Cost: <span class="text-gray-300">{{ estimateTokenCost(totalSessionTokens) }}</span>
            </div>
          </div>

          <div class="flex items-center justify-between gap-4 flex-wrap">
            <!-- Playback Controls -->
            <div class="flex items-center gap-2">
              <button
                @click="stepBackward"
                class="p-1.5 bg-dark-800 hover:bg-dark-700 text-gray-300 border border-dark-700 transition-colors"
                title="Step backward"
              >
                <SkipBack class="w-4 h-4" />
              </button>

              <button
                @click="togglePlay"
                class="px-3 py-1.5 bg-gold-500 hover:bg-gold-400 text-black font-mono font-bold text-xs flex items-center gap-1.5 transition-colors shadow-sm shadow-gold-500/20"
              >
                <component :is="isPlaying ? Pause : Play" class="w-3.5 h-3.5" />
                <span>{{ isPlaying ? 'Pause' : 'Play' }}</span>
              </button>

              <button
                @click="stepForward"
                class="p-1.5 bg-dark-800 hover:bg-dark-700 text-gray-300 border border-dark-700 transition-colors"
                title="Step forward"
              >
                <SkipForward class="w-4 h-4" />
              </button>

              <!-- Speed Multipliers -->
              <div class="flex items-center bg-dark-950 border border-dark-700 p-0.5 ml-2">
                <button
                  v-for="s in [1, 2, 5, 10]"
                  :key="s"
                  @click="playbackSpeed = s"
                  class="px-2 py-0.5 text-[10px] font-mono transition-colors"
                  :class="playbackSpeed === s ? 'bg-gold-500 text-black font-bold' : 'text-gray-400 hover:text-white'"
                >
                  {{ s }}x
                </button>
              </div>

              <!-- Jump to Error -->
              <button
                v-if="hasErrors"
                @click="jumpToNextError"
                class="flex items-center gap-1 px-2.5 py-1 text-[11px] font-mono bg-red-950/40 text-red-400 border border-red-800/60 hover:bg-red-900/40 transition-colors ml-2"
              >
                <AlertCircle class="w-3 h-3" />
                <span>Jump to Error</span>
              </button>
            </div>

            <!-- 1-Click Distillation Button -->
            <button
              @click="store.distillSession(store.activeSessionId!)"
              :disabled="store.isDistilling"
              class="flex items-center gap-1.5 px-3 py-1.5 bg-gradient-to-r from-amber-600 to-gold-600 hover:from-amber-500 hover:to-gold-500 text-black font-mono font-bold text-xs transition-all duration-150 disabled:opacity-50 shadow-md shadow-amber-500/20"
            >
              <Sparkles class="w-3.5 h-3.5" :class="{ 'animate-spin': store.isDistilling }" />
              <span>{{ store.isDistilling ? 'Distilling 100-to-1...' : '1-Click Distillation Summary' }}</span>
            </button>
          </div>

          <!-- Scrubber Timeline Slider -->
          <div class="space-y-1">
            <div class="flex items-center justify-between text-[10px] font-mono text-gray-500">
              <span>Event Step {{ currentStepIndex + 1 }} of {{ store.sessionEvents.length }}</span>
              <span>{{ currentEvent ? formatTime(currentEvent.timestamp) : '' }}</span>
            </div>
            <input
              v-model.number="currentStepIndex"
              type="range"
              min="0"
              :max="Math.max(0, store.sessionEvents.length - 1)"
              class="w-full accent-gold-500 bg-dark-800 h-1.5 cursor-pointer"
            />
          </div>
        </div>

        <!-- Event Inspector Body -->
        <div class="flex-1 overflow-y-auto p-6 space-y-6">
          <!-- Distillation Golden Report Card (if generated) -->
          <div
            v-if="store.distillationReport"
            class="p-5 bg-gradient-to-r from-dark-900 via-dark-850 to-dark-900 border border-gold-500/40 shadow-xl space-y-4 font-mono"
          >
            <div class="flex items-center justify-between pb-3 border-b border-dark-700 flex-wrap gap-2">
              <div class="flex items-center gap-2">
                <Sparkles class="w-4 h-4 text-gold-500" />
                <h3 class="text-xs font-bold text-white uppercase tracking-wider">
                  Distilled Golden Path ({{ store.distillationReport.noise_pruned_percentage }}% Noise Pruned)
                </h3>
              </div>
              <div class="flex items-center gap-2">
                <span class="text-[10px] px-2 py-0.5 bg-gold-500/10 text-gold-400 border border-gold-500/30">
                  {{ store.distillationReport.distilled_events }} Golden Steps / {{ store.distillationReport.total_events }} Raw
                </span>
                <span class="text-[10px] px-2 py-0.5 bg-emerald-950 text-emerald-300 border border-emerald-800">
                  🪙 ~{{ formatTokens(distilledMemoryTokens) }} tokens
                </span>
              </div>
            </div>

            <!-- Golden Steps -->
            <div class="space-y-2">
              <div
                v-for="step in store.distillationReport.golden_steps"
                :key="step.step_index"
                class="p-3 bg-dark-950 border border-dark-800 text-xs space-y-1.5"
              >
                <div class="flex items-center justify-between">
                  <span class="font-bold text-gold-400">Step {{ step.step_index + 1 }}: {{ step.action_name }}</span>
                  <div class="flex items-center gap-1.5">
                    <span class="text-[10px] text-gray-500">🪙 ~{{ estimateTokens(step.summary + ' ' + (step.command || '')) }} tokens</span>
                    <span v-if="step.is_verification" class="text-[10px] px-1.5 py-0.2 bg-emerald-950 text-emerald-400 border border-emerald-800">
                      VERIFICATION
                    </span>
                  </div>
                </div>
                <p class="text-gray-300 text-[11px]">{{ step.summary }}</p>
                <div v-if="step.command" class="bg-dark-900 p-2 border border-dark-800 text-[10px] text-cyan-300">
                  $ {{ step.command }}
                </div>
                <div v-if="step.diff_preview" class="bg-dark-900 p-2 border border-dark-800 text-[10px] text-gray-300 whitespace-pre-wrap">
                  {{ step.diff_preview }}
                </div>
              </div>
            </div>

            <!-- Lessons Learned -->
            <div v-if="store.distillationReport.lessons_learned?.length" class="space-y-1.5 pt-2 border-t border-dark-800">
              <div class="text-[10px] text-emerald-400 font-bold uppercase">Extracted Durable Lessons:</div>
              <ul class="list-disc list-inside text-xs text-gray-300 space-y-1">
                <li v-for="(l, i) in store.distillationReport.lessons_learned" :key="i">
                  {{ l }}
                </li>
              </ul>
            </div>
          </div>

          <!-- Current Step Event Detail Card -->
          <div v-if="currentEvent" class="bg-dark-900 border border-dark-700 p-5 space-y-4 font-mono">
            <div class="flex items-center justify-between pb-3 border-b border-dark-700 flex-wrap gap-2">
              <div class="flex items-center gap-2">
                <span
                  class="px-2 py-0.5 text-[10px] uppercase font-bold"
                  :class="currentEvent.is_error ? 'bg-red-950 text-red-400 border border-red-800' : 'bg-dark-800 text-gold-400 border border-dark-700'"
                >
                  {{ currentEvent.event_type }}
                </span>
                <span class="text-sm font-bold text-white">{{ currentEvent.tool_name || 'Agent Hook Event' }}</span>
              </div>

              <div class="flex items-center gap-2">
                <!-- Event Token Weight Badge -->
                <span
                  class="px-2 py-0.5 text-[10px] font-mono border flex items-center gap-1"
                  :class="currentEventTokens > 1000 ? 'bg-amber-950/80 text-amber-300 border-amber-800' : 'bg-dark-950 text-gold-400 border-dark-800'"
                >
                  <Coins class="w-3 h-3" />
                  <span>🪙 {{ formatTokens(currentEventTokens) }} tokens</span>
                  <span v-if="currentEventTokens > 1000" class="text-amber-400 font-bold">(Context Heavy)</span>
                </span>

                <span class="text-xs text-gray-500">{{ formatTime(currentEvent.timestamp) }}</span>
              </div>
            </div>

            <!-- Tool Arguments -->
            <div v-if="currentEvent.tool_args" class="space-y-1">
              <div class="text-[10px] text-gray-500 uppercase flex items-center justify-between">
                <span>Input / Parameters</span>
                <span class="text-gray-600">~{{ estimateTokens(currentEvent.tool_args) }} tokens</span>
              </div>
              <pre class="p-3 bg-dark-950 border border-dark-800 text-xs text-gray-300 whitespace-pre-wrap overflow-x-auto max-h-48">{{ formatPayload(currentEvent.tool_args) }}</pre>
            </div>

            <!-- Tool Output -->
            <div v-if="currentEvent.tool_output" class="space-y-1">
              <div class="text-[10px] uppercase flex items-center justify-between" :class="currentEvent.is_error ? 'text-red-400' : 'text-gray-500'">
                <span>Tool Execution Output</span>
                <span class="text-gray-600">~{{ estimateTokens(currentEvent.tool_output) }} tokens</span>
              </div>
              <pre
                class="p-3 bg-dark-950 border text-xs whitespace-pre-wrap overflow-x-auto max-h-64 font-mono"
                :class="currentEvent.is_error ? 'border-red-900/60 text-red-300' : 'border-dark-800 text-emerald-300'"
              >{{ currentEvent.tool_output }}</pre>
            </div>

            <!-- Raw Event Payload -->
            <div v-if="currentEvent.raw_payload" class="space-y-1">
              <div class="text-[10px] text-gray-500 uppercase">Raw Event Payload</div>
              <pre class="p-3 bg-dark-950 border border-dark-800 text-[10px] text-gray-400 whitespace-pre-wrap overflow-x-auto max-h-40">{{ JSON.stringify(currentEvent.raw_payload, null, 2) }}</pre>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens, formatTokens, estimateTokenCost, calculateTokenSavings } from '~/utils/tokens';
import {
  History,
  Activity,
  AlertCircle,
  Play,
  Pause,
  SkipBack,
  SkipForward,
  Sparkles,
  Coins,
} from 'lucide-vue-next';

const store = useAutoMemoryStore();
const currentStepIndex = ref(0);
const isPlaying = ref(false);
const playbackSpeed = ref(1);
let timer: ReturnType<typeof setInterval> | null = null;

const currentEvent = computed(() => {
  if (store.sessionEvents.length === 0) return null;
  return store.sessionEvents[currentStepIndex.value] || null;
});

const currentEventTokens = computed(() => {
  if (!currentEvent.value) return 0;
  const raw = (currentEvent.value.tool_args || '') + ' ' + (currentEvent.value.tool_output || '');
  return estimateTokens(raw);
});

const totalSessionTokens = computed(() => {
  return store.sessionEvents.reduce((acc, ev) => {
    return acc + estimateTokens((ev.tool_args || '') + ' ' + (ev.tool_output || ''));
  }, 0);
});

const distilledMemoryTokens = computed(() => {
  if (store.distillationReport) {
    const reportText = JSON.stringify(store.distillationReport.golden_steps) + ' ' + (store.distillationReport.lessons_learned?.join(' ') || '');
    return estimateTokens(reportText);
  }
  return Math.max(120, Math.round(totalSessionTokens.value * 0.015));
});

const tokenReductionStats = computed(() => {
  return calculateTokenSavings(totalSessionTokens.value, distilledMemoryTokens.value);
});

const hasErrors = computed(() => {
  return store.sessionEvents.some((e) => e.is_error);
});

watch(
  () => store.sessionEvents,
  (newEvents) => {
    if (newEvents.length > 0) {
      currentStepIndex.value = 0;
    }
  }
);

const stepForward = () => {
  if (currentStepIndex.value < store.sessionEvents.length - 1) {
    currentStepIndex.value++;
  } else {
    isPlaying.value = false;
    stopPlayback();
  }
};

const stepBackward = () => {
  if (currentStepIndex.value > 0) {
    currentStepIndex.value--;
  }
};

const togglePlay = () => {
  isPlaying.value = !isPlaying.value;
  if (isPlaying.value) {
    startPlayback();
  } else {
    stopPlayback();
  }
};

const startPlayback = () => {
  stopPlayback();
  const intervalMs = Math.max(100, 1000 / playbackSpeed.value);
  timer = setInterval(() => {
    stepForward();
  }, intervalMs);
};

const stopPlayback = () => {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
};

watch(playbackSpeed, () => {
  if (isPlaying.value) {
    startPlayback();
  }
});

const jumpToNextError = () => {
  const nextErrIdx = store.sessionEvents.findIndex(
    (e, idx) => idx > currentStepIndex.value && e.is_error
  );
  if (nextErrIdx !== -1) {
    currentStepIndex.value = nextErrIdx;
  } else {
    const firstErrIdx = store.sessionEvents.findIndex((e) => e.is_error);
    if (firstErrIdx !== -1) {
      currentStepIndex.value = firstErrIdx;
    }
  }
};

const formatPayload = (str: string) => {
  try {
    return JSON.stringify(JSON.parse(str), null, 2);
  } catch {
    return str;
  }
};

const formatTime = (iso: string) => {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  } catch {
    return iso;
  }
};

onUnmounted(() => {
  stopPlayback();
});
</script>
