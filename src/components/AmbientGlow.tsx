// SPDX-License-Identifier: FSL-1.1-Apache-2.0

interface AmbientGlowProps {
  className?: string;
}

/**
 * Subtle warm radial backdrop — a quiet gold ember rising from the base of a panel.
 *
 * Replaces the old `fourda-turing-fire` GAME shader that was used as a low-opacity
 * atmosphere behind briefing empty/warmup states. Pure CSS gradient: no GPU context,
 * no animation loop, renders identically under prefers-reduced-motion. Always absolute
 * inset-0 + pointer-events-none, so it sits behind content without intercepting input.
 */
export function AmbientGlow({ className = '' }: AmbientGlowProps) {
  return (
    <div
      className={`absolute inset-0 rounded-lg overflow-hidden pointer-events-none ${className}`}
      aria-hidden="true"
      style={{
        background:
          'radial-gradient(130% 90% at 50% 110%, rgba(212,175,55,0.12), rgba(212,175,55,0.04) 45%, transparent 70%)',
      }}
    />
  );
}
