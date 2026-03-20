import { memo } from 'react';

interface CardProps {
  children: React.ReactNode;
  className?: string;
  padding?: 'none' | 'sm' | 'md' | 'lg';
}

const PADDING = {
  none: '',
  sm: 'p-3',
  md: 'p-4',
  lg: 'p-5',
} as const;

export const Card = memo(function Card({
  children,
  className = '',
  padding = 'md',
}: CardProps) {
  return (
    <div
      className={`bg-bg-secondary rounded-lg border border-border overflow-hidden ${PADDING[padding]} ${className}`}
    >
      {children}
    </div>
  );
});
