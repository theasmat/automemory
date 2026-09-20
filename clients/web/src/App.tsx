import React, { useEffect } from 'react';
import { useAutoMemoryStore } from './store/useAutoMemoryStore';
import { Sidebar } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { StatusBar } from './components/layout/StatusBar';
import { OverviewView } from './components/overview/OverviewView';
import { KnowledgeGraph } from './components/graph/KnowledgeGraph';
import { MemoriesView } from './components/memories/MemoriesView';
import { ProposalsView } from './components/proposals/ProposalsView';
import { TracesView } from './components/traces/TracesView';
import { FailuresView } from './components/failures/FailuresView';
import { ContextTester } from './components/context/ContextTester';
import { LiveEventLedger } from './components/events/LiveEventLedger';

export const App: React.FC = () => {
  const { activeTab, init } = useAutoMemoryStore();

  useEffect(() => {
    init();
  }, [init]);

  return (
    <div className="flex h-screen w-screen bg-black text-gray-100 overflow-hidden font-sans">
      {/* Fixed Left Navigation */}
      <Sidebar />

      {/* Main Content Area */}
      <div className="flex flex-col flex-1 h-full overflow-hidden">
        {/* Top Header */}
        <Header />

        {/* Dynamic Tab Body */}
        <main className="flex-1 overflow-hidden relative bg-dark-950">
          {activeTab === 'overview' && <OverviewView />}
          {activeTab === 'graph' && <KnowledgeGraph />}
          {activeTab === 'memories' && <MemoriesView />}
          {activeTab === 'proposals' && <ProposalsView />}
          {activeTab === 'traces' && <TracesView />}
          {activeTab === 'failures' && <FailuresView />}
          {activeTab === 'context' && <ContextTester />}
          {activeTab === 'events' && <LiveEventLedger />}
        </main>

        {/* Bottom Status Bar */}
        <StatusBar />
      </div>
    </div>
  );
};

export default App;
