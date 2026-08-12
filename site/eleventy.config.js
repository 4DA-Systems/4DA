export default function(eleventyConfig) {
  // Pass through static assets from site root to output root
  eleventyConfig.addPassthroughCopy({ "hero-bg.mp4": "hero-bg.mp4" });
  eleventyConfig.addPassthroughCopy({ "hero-sun.jpg": "hero-sun.jpg" });
  eleventyConfig.addPassthroughCopy({ "og-image.png": "og-image.png" });
  // robots.txt lives in src/ and .txt is not an Eleventy template format, so it
  // needs an explicit passthrough. (This previously pointed at a site-root
  // "robots.txt" that does not exist, so no robots.txt was emitted at all.)
  eleventyConfig.addPassthroughCopy({ "src/robots.txt": "robots.txt" });
  eleventyConfig.addPassthroughCopy({ "screenshots": "screenshots" });
  // NOTE: nothing internal ships here. merch-photos/ are Shopify upload masters
  // (merch.njk renders product images from the Shopify Storefront API, not from
  // this repo), shopify-theme.css belongs in the Shopify admin, and
  // test-e2e-stripe.mjs is an internal harness. None of them are passed through.
  //
  // The API handlers are NOT copied to the static output either. They run as
  // Cloudflare Pages Functions from site/functions/, which Pages picks up
  // directly from the project root — copying source into _site would expose it
  // publicly AND collide with Functions routing.
  // _redirects + _headers must live in the OUTPUT dir to take effect on Pages.
  eleventyConfig.addPassthroughCopy({ "_redirects": "_redirects" });
  eleventyConfig.addPassthroughCopy({ "_headers": "_headers" });
  // Stack Scan — self-contained static app (no analytics by design; must NOT go through
  // base.njk, which injects PostHog — the page's promise is a clean Network tab)
  eleventyConfig.addPassthroughCopy({ "scan": "scan" });

  return {
    dir: {
      input: "src",
      output: "_site",
      includes: "_includes"
    }
  };
}
