import { memo } from 'react';

interface EmptyStateProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}

export const EmptyState = memo(function EmptyState({
  icon,
  title,
  description,
  action,
  className = '',
}: EmptyStateProps) {
  return (
    <div className={`flex flex-col items-center justify-center py-12 px-6 text-center ${className}`}>
      {icon && <div className="mb-4 text-text-muted">{icon}</div>}
      <h3 className="text-sm font-medium text-text-secondary mb-1">{title}</h3>
      {description && (
        <p className="text-xs text-text-muted leading-relaxed max-w-sm">{description}</p>
      )}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
});
