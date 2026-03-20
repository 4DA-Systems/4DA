import { memo } from 'react';

interface SectionHeaderProps {
  children: React.ReactNode;
  className?: string;
}

export const SectionHeader = memo(function SectionHeader({
  children,
  className = '',
}: SectionHeaderProps) {
  return (
    <h4
      className={`text-xs font-medium text-text-muted uppercase tracking-wider ${className}`}
    >
      {children}
    </h4>
  );
});
