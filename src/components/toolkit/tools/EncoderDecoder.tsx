import { useState } from 'react';

import { SUB_TABS } from './encoder-decoder-utils';
import type { SubTab } from './encoder-decoder-utils';
import { Base64Tab, UrlTab, JwtTab, HashTab } from './encoder-decoder-tabs';

export default function EncoderDecoder() {
  const [activeTab, setActiveTab] = useState<SubTab>('base64');

  return (
    <div className="space-y-4">
      <nav className="flex items-center gap-1 bg-bg-secondary rounded-lg p-1 border border-border w-fit">
        {SUB_TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-1.5 text-xs font-medium rounded-md transition-colors ${
              activeTab === tab.id
                ? 'bg-bg-tertiary text-white border border-border'
                : 'text-text-muted hover:text-text-secondary border border-transparent'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </nav>
      <div className="bg-bg-secondary border border-border rounded-lg p-4">
        {activeTab === 'base64' && <Base64Tab />}
        {activeTab === 'url' && <UrlTab />}
        {activeTab === 'jwt' && <JwtTab />}
        {activeTab === 'hash' && <HashTab />}
      </div>
    </div>
  );
}
