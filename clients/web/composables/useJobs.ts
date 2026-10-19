import { computed, ref } from 'vue';
import { request } from '~/utils/api';

export interface Job {
  id: string;
  kind: string;
  title: string;
  state: 'running' | 'done' | 'failed' | 'cancelled';
  done: number;
  total: number;
  message: string | null;
  started_at: number;
}

const jobs = ref<Job[]>([]);
let timer: ReturnType<typeof setTimeout> | null = null;

async function refresh() {
  const res = await request<Job[]>('/api/jobs');
  if (res.ok && Array.isArray(res.data)) jobs.value = res.data;
}

/** Poll quickly while something runs, slowly otherwise; the tray and the Tools page share one timer. */
function loop() {
  if (timer) return;
  const tick = async () => {
    await refresh();
    timer = setTimeout(tick, jobs.value.some((j) => j.state === 'running') ? 1000 : 8000);
  };
  tick();
}

export function useJobs() {
  loop();
  const running = computed(() => jobs.value.filter((j) => j.state === 'running'));
  const latest = (kind: string) => jobs.value.find((j) => j.kind === kind) || null;
  return { jobs, running, latest, refresh };
}
