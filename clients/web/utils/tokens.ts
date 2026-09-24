// Utility functions for LLM token estimation and formatting in AutoMemory Pro

/**
 * Estimates token count using standard byte-pair encoding / character heuristics (~3.8 chars per token for code & English text).
 */
export function estimateTokens(text: string | null | undefined): number {
  if (!text) return 0;
  const trimmed = text.trim();
  if (!trimmed) return 0;
  // Account for words and special tokens in code / json
  const charLength = trimmed.length;
  return Math.max(1, Math.ceil(charLength / 3.8));
}

/**
 * Formats token count with human-readable suffix (e.g. "142", "1.4k", "24.5k").
 */
export function formatTokens(count: number | null | undefined): string {
  if (count === null || count === undefined || isNaN(count)) return '0';
  if (count >= 1_000_000) {
    return (count / 1_000_000).toFixed(1) + 'M';
  }
  if (count >= 1_000) {
    return (count / 1_000).toFixed(1) + 'k';
  }
  return count.toLocaleString();
}

/**
 * Estimates monetary cost of token consumption ($0.003 / 1k tokens for standard reasoning model).
 */
export function estimateTokenCost(tokens: number, costPer1k: number = 0.003): string {
  if (!tokens || tokens <= 0) return '$0.000';
  const cost = (tokens / 1000) * costPer1k;
  if (cost < 0.0001) return '<$0.0001';
  return `$${cost.toFixed(4)}`;
}

/**
 * Computes trajectory noise pruning and token compression ratio.
 */
export function calculateTokenSavings(rawTokens: number, distilledTokens: number): {
  savedTokens: number;
  reductionPercentage: number;
} {
  if (rawTokens <= 0) return { savedTokens: 0, reductionPercentage: 0 };
  const saved = Math.max(0, rawTokens - distilledTokens);
  const reduction = Math.min(99.9, ((saved / rawTokens) * 100));
  return {
    savedTokens: saved,
    reductionPercentage: Number(reduction.toFixed(1)),
  };
}
