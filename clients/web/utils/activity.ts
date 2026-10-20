/** Plain-language lines for audit rows (`/api/activity`), shared by Settings and the Activity page. */
export function describeActivity(a: any): string {
  if (a.action === 'hook.deny') return `Blocked ${a.details?.agent || 'an agent'}: ${a.target}`;
  if (a.action === 'hook.ask') return `Asked before ${a.target} (${a.details?.agent || 'agent'})`;
  if (a.action === 'ui.action') return `Dashboard: ${a.target}`;
  return a.action;
}

export function ago(iso: string): string {
  const m = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 60000));
  if (m < 1) return 'just now';
  if (m < 60) return `${m} min ago`;
  if (m < 1440) return `${Math.round(m / 60)} h ago`;
  return `${Math.round(m / 1440)} d ago`;
}
