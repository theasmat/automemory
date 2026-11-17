import type { Memory } from '~/types';

export function formatType(type?: string): string {
  if (!type) return 'Note';
  const t = type.toLowerCase();
  if (t === 'decision') return 'Decision';
  if (t === 'lesson') return 'Lesson';
  if (t === 'gotcha') return 'Watch out';
  if (t === 'preference') return 'Preference';
  if (t === 'convention') return 'Convention';
  return type.charAt(0).toUpperCase() + type.slice(1);
}

export function formatRelativeTime(dateStr?: string | null): string {
  if (!dateStr) return '';
  const date = new Date(dateStr);
  const diffSec = Math.floor((Date.now() - date.getTime()) / 1000);
  if (diffSec < 60) return 'just now';
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  if (diffSec < 604800) return `${Math.floor(diffSec / 86400)}d ago`;
  return date.toLocaleDateString();
}

/** Notes written by tools are markdown; show them as calm plain text. A decision repeats its topic and wording in the title: keep only what the title does not say. */
export function plainMemoryText(memory: Pick<Memory, 'content' | 'type'>): string {
  const text = (memory.content || '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/^#{1,6}\s*/gm, '')
    .replace(/\*\*|__|`/g, '')
    .replace(/^\s*[-*]\s+/gm, '• ')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
  if (memory.type?.toLowerCase() !== 'decision') return text;
  return text
    .split('\n')
    .filter((l) => !/^(Architectural Decision|Decision):/i.test(l.trim()))
    .map((l) => l.replace(/^Rationale:/i, 'Why:'))
    .join('\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}
