// SPDX-License-Identifier: FSL-1.1-Apache-2.0

interface Bucket {
  tokens: number;
  maxTokens: number;
  refillRate: number; // tokens per second
  lastRefill: number; // timestamp ms
}

export class RateLimiter {
  private buckets: Map<string, Bucket> = new Map();

  constructor(config: Record<string, { maxPerMinute: number }>) {
    for (const [source, { maxPerMinute }] of Object.entries(config)) {
      this.buckets.set(source, {
        tokens: maxPerMinute,
        maxTokens: maxPerMinute,
        refillRate: maxPerMinute / 60,
        lastRefill: Date.now(),
      });
    }
  }

  canProceed(source: string): boolean {
    const bucket = this.buckets.get(source);
    if (!bucket) return true;
    this.refill(bucket);
    return bucket.tokens >= 1;
  }

  consume(source: string): boolean {
    const bucket = this.buckets.get(source);
    if (!bucket) return true;
    this.refill(bucket);
    if (bucket.tokens < 1) return false;
    bucket.tokens -= 1;
    return true;
  }

  waitTimeMs(source: string): number {
    const bucket = this.buckets.get(source);
    if (!bucket) return 0;
    this.refill(bucket);
    if (bucket.tokens >= 1) return 0;
    const deficit = 1 - bucket.tokens;
    return Math.ceil((deficit / bucket.refillRate) * 1000);
  }

  private refill(bucket: Bucket): void {
    const now = Date.now();
    const elapsed = (now - bucket.lastRefill) / 1000;
    bucket.tokens = Math.min(bucket.maxTokens, bucket.tokens + elapsed * bucket.refillRate);
    bucket.lastRefill = now;
  }
}

export const DEFAULT_RATE_LIMITS = {
  osv: { maxPerMinute: 10 },
  hn: { maxPerMinute: 30 },
  npm: { maxPerMinute: 120 },
  crates: { maxPerMinute: 50 },
  pypi: { maxPerMinute: 60 },
  go: { maxPerMinute: 60 },
};
