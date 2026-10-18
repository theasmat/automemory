<template>
  <div class="flex flex-col md:flex-row h-full min-h-[600px] bg-bg rounded-card border border-border overflow-hidden select-none">
    <!-- Left Session Selector Sidebar -->
    <div class="w-full md:w-80 border-b md:border-b-0 md:border-r border-border bg-surface flex flex-col justify-between shrink-0">
      <div>
        <div class="p-4 border-b border-border flex items-center justify-between">
          <div class="flex items-center gap-2">
            <History class="w-4 h-4 text-accent" />
            <span class="text-xs font-semibold text-text uppercase tracking-wider">Recorded sessions</span>
          </div>
          <Badge variant="neutral">{{ store.sessions.length }}</Badge>
        </div>

        <div class="overflow-y-auto max-h-48 md:max-h-[calc(100vh-14rem)] p-2 space-y-1">
          <div v-if="store.sessions.length === 0" class="text-center py-8 text-text-muted text-xs">
            No recorded sessions yet. Execute tasks to record event streams!
          </div>

          <button
            v-for="s in store.sessions"
            :key="s.session_id"
            type="button"
            @click="store.selectSession(s.session_id)"
            class="w-full text-left p-3 rounded-card border transition-all duration-150 text-xs space-y-1.5"
            :class="[
              store.activeSessionId === s.session_id
                ? 'bg-surface-2 border-accent text-text ring-1 ring-accent/30 font-semibold'
                : 'bg-surface border-border text-text-muted hover:text-text hover:bg-surface-2'
            ]"
          >
            <div class="flex items-center justify-between">
              <span class="truncate max-w-[170px]" :class="store.activeSessionId === s.session_id ? 'text-accent' : 'text-text'">
                {{ s.session_id }}
              </span>
              <Badge variant="neutral">{{ s.adapter_id || 'agy' }}</Badge>
            </div>

            <div class="flex items-center justify-between text-[11px] text-text-muted">
              <span class="flex items-center gap-1">
                <Activity class="w-3 h-3 text-info" /> {{ s.event_count }} events
              </span>
              <span v-if="s.error_count > 0" class="text-bad font-semibold flex items-center gap-0.5">
                <AlertCircle class="w-3 h-3" /> {{ s.error_count }} err
              </span>
            </div>
          </button>
        </div>
      </div>
    </div>

    <!-- Right Replay & Distillation Stage -->
    <div class="flex-1 flex flex-col min-w-0 bg-surface">
      <!-- Empty State -->
      <div v-if="!store.activeSessionId || store.sessionEvents.length === 0" class="flex-1 flex flex-col items-center justify-center p-8 text-center">
        <History class="w-10 h-10 text-text-muted mb-3 opacity-40" />
        <div class="text-sm font-semibold text-text">Select a session from the list to begin replay</div>
        <p class="text-xs text-text-muted mt-1 max-w-sm">
          Scrub through tool actions, inspect diffs, test variable playback speeds, and inspect session history.
        </p>
      </div>

      <div v-else class="flex-1 flex flex-col min-h-0 overflow-hidden">
        <!-- Replay Control Bar & Token Reduction Header -->
        <div class="p-4 border-b border-border bg-surface-2 flex flex-col gap-3 shrink-0">
          <!-- Token Reduction Ribbon for Active Session -->
          <div class="flex flex-wrap items-center justify-between gap-2 pb-2 border-b border-border text-xs">
            <div class="flex items-center gap-3 flex-wrap">
              <span class="text-text-muted">
                Raw session: <strong class="text-text">~{{ formatTokens(totalSessionTokens) }} tokens</strong>
              </span>
              <span class="text-border">•</span>
              <span class="text-text-muted">
                Distilled memory: <strong class="text-info">~{{ formatTokens(distilledMemoryTokens) }} tokens</strong>
              </span>
              <span class="text-border">•</span>
              <Badge variant="ok">
                {{ tokenReductionStats.reductionPercentage }}% reduction (~{{ formatTokens(tokenReductionStats.savedTokens) }} saved)
              </Badge>
            </div>

            <div class="text-xs text-text-muted">
              Est. context cost: <span class="text-text">{{ estimateTokenCost(totalSessionTokens) }}</span>
            </div>
          </div>

          <div class="flex items-center justify-between gap-4 flex-wrap">
            <!-- Playback Controls -->
            <div class="flex items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                @click="stepBackward"
                title="Step backward"
                aria-label="Step backward"
              >
                <SkipBack class="w-3.5 h-3.5" />
              </Button>

              <Button
                variant="primary"
                size="sm"
                @click="togglePlay"
              >
                <component :is="isPlaying ? Pause : Play" class="w-3.5 h-3.5 mr-1" />
                <span>{{ isPlaying ? 'Pause' : 'Play' }}</span>
              </Button>

              <Button
                variant="secondary"
                size="sm"
                @click="stepForward"
                title="Step forward"
                aria-label="Step forward"
              >
                <SkipForward class="w-3.5 h-3.5" />
              </Button>

              <!-- Speed Multipliers -->
              <div class="flex items-center bg-surface border border-border rounded-card p-0.5 ml-2">
                <button
                  v-for="s in [1, 2, 5, 10]"
                  :key="s"
                  type="button"
                  @click="playbackSpeed = s"
                  class="px-2 py-0.5 text-xs rounded transition-colors"
                  :class="playbackSpeed === s ? 'bg-accent text-accent-fg font-bold' : 'text-text-muted hover:text-text'"
                >
                  {{ s }}x
                </button>
              </div>

              <!-- Jump to Error -->
              <button
                v-if="hasErrors"
                type="button"
                @click="jumpToNextError"
                class="flex items-center gap-1 px-2.5 py-1 text-xs bg-bad/10 text-bad rounded border border-bad/30 hover:bg-bad/20 transition-colors ml-2"
              >
                <AlertCircle class="w-3 h-3" />
                <span>Jump to error</span>
              </button>
            </div>

            <!-- Distillation Button -->
            <Button
              variant="secondary"
              size="sm"
              :disabled="store.isDistilling"
              @click="store.distillSession(store.activeSessionId!)"
            >
              <Sparkles class="w-3.5 h-3.5 mr-1.5" :class="{ 'animate-spin': store.isDistilling }" />
              <span>{{ store.isDistilling ? 'Distilling...' : 'Distill session' }}</span>
            </Button>
          </div>

          <!-- Scrubber Timeline Slider -->
          <div class="space-y-1">
            <div class="flex items-center justify-between text-xs text-text-muted font-mono">
              <span>Step {{ currentStepIndex + 1 }} of {{ store.sessionEvents.length }}</span>
              <span>{{ currentEvent ? formatTime(currentEvent.timestamp) : '' }}</span>
            </div>
            <input
              v-model.number="currentStepIndex"
              type="range"
              min="0"
              :max="Math.max(0, store.sessionEvents.length - 1)"
              class="w-full accent-accent bg-surface h-1.5 rounded cursor-pointer"
            />
          </div>
        </div>

        <!-- Event Inspector Body -->
        <div class="flex-1 overflow-y-auto p-4 sm:p-6 space-y-6">
          <!-- Distillation Report Card (if generated) -->
          <Card v-if="store.distillationReport">
            <template #header>
              <div class="flex items-center justify-between flex-wrap gap-2">
                <div class="flex items-center gap-2">
                  <Sparkles class="w-4 h-4 text-accent" />
                  <h3 class="text-xs font-semibold text-text uppercase tracking-wider">
                    Distilled golden path ({{ store.distillationReport.noise_pruned_percentage }}% noise pruned)
                  </h3>
                </div>
                <div class="flex items-center gap-2">
                  <Badge variant="neutral">
                    {{ store.distillationReport.distilled_events }} steps / {{ store.distillationReport.total_events }} raw
                  </Badge>
                  <Badge variant="ok">
                    ~{{ formatTokens(distilledMemoryTokens) }} tokens
                  </Badge>
                </div>
              </div>
            </template>

            <!-- Golden Steps -->
            <div class="space-y-2">
              <div
                v-for="step in store.distillationReport.golden_steps"
                :key="step.step_index"
                class="p-3 bg-surface-2 rounded-card border border-border text-xs space-y-1.5"
              >
                <div class="flex items-center justify-between">
                  <span class="font-semibold text-accent">Step {{ step.step_index + 1 }}: {{ step.action_name }}</span>
                  <div class="flex items-center gap-1.5">
                    <span class="text-xs text-text-muted">~{{ estimateTokens(step.summary + ' ' + (step.command || '')) }} tokens</span>
                    <Badge v-if="step.is_verification" variant="ok">VERIFICATION</Badge>
                  </div>
                </div>
                <p class="text-text text-xs">{{ step.summary }}</p>
                <div v-if="step.command" class="bg-surface p-2 rounded border border-border font-mono text-xs text-info">
                  $ {{ step.command }}
                </div>
                <div v-if="step.diff_preview" class="bg-surface p-2 rounded border border-border font-mono text-xs text-text-muted whitespace-pre-wrap">
                  {{ step.diff_preview }}
                </div>
              </div>
            </div>

            <!-- Lessons Learned -->
            <div v-if="store.distillationReport.lessons_learned?.length" class="space-y-1.5 pt-3 border-t border-border mt-4">
              <div class="text-xs text-ok font-semibold uppercase">Extracted durable lessons:</div>
              <ul class="list-disc list-inside text-xs text-text-muted space-y-1">
                <li v-for="(l, i) in store.distillationReport.lessons_learned" :key="i">
                  {{ l }}
                </li>
              </ul>
            </div>
          </Card>

          <!-- Current Step Event Detail Card -->
          <Card v-if="currentEvent">
            <template #header>
              <div class="flex items-center justify-between flex-wrap gap-2">
                <div class="flex items-center gap-2">
                  <Badge :variant="currentEvent.is_error ? 'bad' : 'neutral'">
                    {{ currentEvent.event_type }}
                  </Badge>
                  <span class="text-sm font-semibold text-text">{{ currentEvent.tool_name || 'Agent Hook Event' }}</span>
                </div>

                <div class="flex items-center gap-2">
                  <Badge variant="neutral">
                    {{ formatTokens(currentEventTokens) }} tokens
                  </Badge>
                  <span class="text-xs text-text-muted">{{ formatTime(currentEvent.timestamp) }}</span>
                </div>
              </div>
            </template>

            <div class="space-y-4">
              <!-- Tool Arguments -->
              <div v-if="currentEvent.tool_args" class="space-y-1">
                <div class="text-xs text-text-muted font-medium flex items-center justify-between">
                  <span>Input / parameters</span>
                  <span>~{{ estimateTokens(currentEvent.tool_args) }} tokens</span>
                </div>
                <pre class="p-3 bg-surface-2 rounded-card border border-border text-xs text-text font-mono whitespace-pre-wrap overflow-x-auto max-h-48">{{ formatPayload(currentEvent.tool_args) }}</pre>
              </div>

              <!-- Tool Output -->
              <div v-if="currentEvent.tool_output" class="space-y-1">
                <div class="text-xs font-medium flex items-center justify-between" :class="currentEvent.is_error ? 'text-bad' : 'text-text-muted'">
                  <span>Tool output</span>
                  <span>~{{ estimateTokens(currentEvent.tool_output) }} tokens</span>
                </div>
                <pre
                  class="p-3 bg-surface-2 rounded-card border text-xs whitespace-pre-wrap overflow-x-auto max-h-64 font-mono"
                  :class="currentEvent.is_error ? 'border-bad/40 text-bad' : 'border-border text-text'"
                >{{ currentEvent.tool_output }}</pre>
              </div>

              <!-- Raw Event Payload -->
              <div v-if="currentEvent.raw_payload" class="space-y-1">
                <div class="text-xs text-text-muted font-medium">Raw event payload</div>
                <pre class="p-3 bg-surface-2 rounded-card border border-border text-xs text-text-muted whitespace-pre-wrap overflow-x-auto max-h-40 font-mono">{{ JSON.stringify(currentEvent.raw_payload, null, 2) }}</pre>
              </div>
            </div>
          </Card>
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
