import type { ComponentType } from 'react';

export type ToolCategory = 'formatters' | 'encoders' | 'generators' | 'system' | 'intelligence' | 'capture';

export interface ToolDescriptor {
  id: string;
  name: string;
  description: string;
  icon: string;  // SVG path or emoji fallback
  category: ToolCategory;
  pro: boolean;
  component: React.LazyExoticComponent<ComponentType>;
  keywords: string[];  // for search/filter
}
