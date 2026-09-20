import React, { useState } from 'react';
import { Sliders, Sparkles, Copy, Check, Terminal } from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';

export const ContextTester: React.FC = () => {
  const {
    tokenBudget,
    setTokenBudget,
    assembledContext,
    isAssemblingContext,
    assembleContext,
  } = useAutoMemoryStore();

  const [testQuery, setTestQuery] = useState('');
  const [copied, setCopied] = useState(false);

  const handleAssemble = () => {
    assembleContext(testQuery);
  };

  const handleCopy = () => {
    if (assembledContext) {
      navigator.clipboard.writeText(assembledContext);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  // Approximate tokens by word/char count
  const estimatedTokens = assembledContext
    ? Math.round(assembledContext.length / 3.8)
    : 0;

  return (
    <div className="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full font-mono text-xs">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 border-b border-dark-700">
        <div>
          <div className="flex items-center gap-2">
            <Sliders className="w-5 h-5 text-gold-500" />
            <h2 className="text-xl font-bold font-mono text-white tracking-tight">
              Context Injection Simulator
            </h2>
          </div>
          <p className="text-xs font-mono text-gray-400 mt-1">
            Test and simulate exact bounded prompt assemblies injected into agent system turns under strict token limits.
          </p>
        </div>
      </div>

      {/* Control Panel */}
      <div className="bg-dark-900 border border-dark-700 p-5 space-y-4">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* Query Input */}
          <div>
            <label className="text-[10px] text-gray-400 uppercase tracking-wider block mb-1.5">
              Task Prompt / User Intent Query
            </label>
            <input
              type="text"
              placeholder="e.g. fix websocket connection timeout in axum serve"
              value={testQuery}
              onChange={(e) => setTestQuery(e.target.value)}
              className="w-full bg-dark-950 border border-dark-700 text-white px-3 py-2 text-xs focus:outline-none focus:border-gold-500 placeholder-gray-600"
            />
          </div>

          {/* Token Budget Slider */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <label className="text-[10px] text-gray-400 uppercase tracking-wider">
                Token Budget Limit
              </label>
              <span className="text-xs font-bold text-gold-400 font-mono">
                {tokenBudget} tokens
              </span>
            </div>
            <input
              type="range"
              min="500"
              max="8000"
              step="250"
              value={tokenBudget}
              onChange={(e) => setTokenBudget(Number(e.target.value))}
              className="w-full accent-gold-500 bg-dark-950 cursor-pointer"
            />
          </div>
        </div>

        <div className="flex justify-end pt-2">
          <button
            onClick={handleAssemble}
            disabled={isAssemblingContext}
            className="flex items-center gap-2 px-5 py-2 bg-gold-500 hover:bg-gold-400 text-black font-bold text-xs transition-all active:scale-95 disabled:opacity-50"
          >
            <Sparkles className={`w-4 h-4 text-black ${isAssemblingContext ? 'animate-spin' : ''}`} />
            <span>{isAssemblingContext ? 'Assembling...' : 'Simulate Context Injection'}</span>
          </button>
        </div>
      </div>

      {/* Output Preview */}
      <div className="bg-dark-900 border border-dark-700 p-5 space-y-3">
        <div className="flex items-center justify-between pb-2 border-b border-dark-800">
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-gray-400" />
            <span className="text-xs font-bold text-white uppercase">Assembled Output Payload</span>
            {estimatedTokens > 0 && (
              <span className="px-2 py-0.5 bg-dark-800 text-cyan-400 text-[10px]">
                ~{estimatedTokens} / {tokenBudget} tokens
              </span>
            )}
          </div>

          {assembledContext && (
            <button
              onClick={handleCopy}
              className="flex items-center gap-1.5 text-xs text-gray-400 hover:text-white transition-colors"
            >
              {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
              <span>{copied ? 'Copied' : 'Copy'}</span>
            </button>
          )}
        </div>

        {assembledContext ? (
          <pre className="bg-dark-950 p-4 border border-dark-800 text-gray-200 text-xs leading-relaxed whitespace-pre-wrap overflow-x-auto max-h-[500px]">
            {assembledContext}
          </pre>
        ) : (
          <div className="text-center py-16 text-gray-600 text-xs">
            Click "Simulate Context Injection" to evaluate the bounded memory prompt generated by AutoMemory.
          </div>
        )}
      </div>
    </div>
  );
};
