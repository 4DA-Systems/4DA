export default function(eleventyConfig) {
  // Pass through static assets from site root to output root
  eleventyConfig.addPassthroughCopy({ "hero-bg.mp4": "hero-bg.mp4" });
  eleventyConfig.addPassthroughCopy({ "hero-sun.jpg": "hero-sun.jpg" });
  eleventyConfig.addPassthroughCopy({ "og-image.png": "og-image.png" });
  eleventyConfig.addPassthroughCopy({ "robots.txt": "robots.txt" });
  eleventyConfig.addPassthroughCopy({ "screenshots": "screenshots" });
  eleventyConfig.addPassthroughCopy({ "media": "media" });
  eleventyConfig.addPassthroughCopy({ "merch-designs": "merch-designs" });
  eleventyConfig.addPassthroughCopy({ "merch-photos": "merch-photos" });
  eleventyConfig.addPassthroughCopy({ "api": "api" });
  eleventyConfig.addPassthroughCopy({ "shopify-theme.css": "shopify-theme.css" });
  eleventyConfig.addPassthroughCopy({ "test-e2e-stripe.mjs": "test-e2e-stripe.mjs" });
  // Stack Scan — self-contained static app (no analytics by design; must NOT go through
  // base.njk, which injects PostHog — the page's promise is a clean Network tab)
  eleventyConfig.addPassthroughCopy({ "scan": "scan" });

  // Exclude utility files from processing
  eleventyConfig.ignores.add("src/og-image.html");

  return {
    dir: {
      input: "src",
      output: "_site",
      includes: "_includes"
    }
  };
}
