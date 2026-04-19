// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { forwardRef, type ButtonHTMLAttributes } from 'react';

type ButtonVariant = 'primary' | 'secondary' | 'tertiary' | 'danger' | 'icon';
type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const base =
  'inline-flex items-center justify-center font-medium transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-gold/50 disabled:opacity-50 disabled:cursor-not-allowed';

const variants: Record<ButtonVariant, string> = {
  primary:
    'bg-[var(--color-accent-action)] text-white rounded-lg hover:bg-[var(--color-accent-action-hover)] active:brightness-90',
  secondary:
    'bg-bg-secondary border border-border text-text-secondary rounded-lg hover:text-white hover:border-text-muted active:bg-bg-tertiary',
  tertiary:
    'bg-transparent text-text-secondary rounded hover:bg-white/10 hover:text-white active:bg-white/5',
  danger:
    'bg-error/10 text-error border border-error/20 rounded-lg hover:bg-error/20 active:bg-error/30',
  icon:
    'rounded-lg bg-bg-tertiary text-text-muted hover:text-white hover:bg-border active:bg-bg-secondary',
};

const sizes: Record<ButtonSize, string> = {
  sm: 'px-2.5 py-1.5 text-xs gap-1.5',
  md: 'px-4 py-2 text-sm gap-2',
  lg: 'px-6 py-2.5 text-sm gap-2',
};

const iconSizes: Record<ButtonSize, string> = {
  sm: 'w-7 h-7',
  md: 'w-8 h-8',
  lg: 'w-10 h-10',
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = 'secondary', size = 'md', className = '', ...props }, ref) => {
    const sizeClass = variant === 'icon' ? iconSizes[size] : sizes[size];
    return (
      <button
        ref={ref}
        className={`${base} ${variants[variant]} ${sizeClass} ${className}`}
        {...props}
      />
    );
  },
);

Button.displayName = 'Button';
