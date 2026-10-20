import { ref } from 'vue';

export interface PolicyRule { path: string; tier: 'safe' | 'protected' | 'effect' | 'terminal'; description: string }

const rules = ref<PolicyRule[]>([]);
let loading: Promise<void> | null = null;

function load() {
  loading ||= fetch('/api/policy')
    .then((r) => (r.ok ? r.json() : []))
    .then((data) => { rules.value = Array.isArray(data) ? data : []; })
    .catch(() => {});
  return loading;
}

/** The tier of an API path from the server's own table (`:id` segments match anything). */
export function usePolicy() {
  load();
  const tierOf = (path: string): PolicyRule['tier'] | null => {
    const hit = rules.value.find((r) => new RegExp('^' + r.path.replace(/:[^/]+/g, '[^/]+') + '$').test(path));
    return hit ? hit.tier : null;
  };
  return { rules, tierOf };
}
