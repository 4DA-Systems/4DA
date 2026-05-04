# Module E: Execution Playbook

**STREETS Developer Income Playbook**
*Weeks 9-10 | 6 Lessons | Deliverable: Your First Product, Live and Accepting Payments*

> "From idea to deployed in 48 hours. No overthinking."

---

You have the infrastructure (Module S). You have the moat (Module T). You have the revenue engine designs (Module R). Now it is time to ship.

This module is the one most developers never reach — not because it is hard, but because they are still polishing their codebase, refactoring their architecture, tweaking their color palette. They are doing everything except the thing that matters: putting a product in front of a human being who can pay for it.

Shipping is a skill. Like any skill, it gets easier with practice and worse with delay. The longer you wait, the harder it becomes. The more you ship, the less scary it feels. Your first launch will be messy. That is the point.

By the end of these two weeks, you will have:

- A validated product idea tested against real demand signals
- A live, deployed product accessible via a real domain
- Payment processing accepting real money
- At least one public launch on a platform where your target audience gathers
- A post-launch metrics system to guide your next moves

No hypotheticals. No "in theory." A real product, live on the internet, capable of generating revenue.

Let's build it.

---

## Lesson 1: The 48-Hour Sprint

*"Saturday morning to Sunday night. One product. Zero excuses."*

### Why 48 Hours

Parkinson's Law says work expands to fill the time available. Give yourself 6 months to build a product and you will spend 5 months deliberating and 1 month in a stressed frenzy. Give yourself 48 hours and you will make decisions, cut scope ruthlessly, and ship something real.

The 48-hour constraint is not about building something perfect. It is about building something that exists. Existence beats perfection every time, because a live product generates data — who visits, who clicks, who pays, who complains — and data tells you what to build next.

Every successful developer product I have studied followed this pattern: ship fast, learn fast, iterate fast. The ones that failed? They all have beautiful README files and zero users.

Here is your minute-by-minute playbook.

### Day 1 — Saturday

#### Morning Block (4 hours): Validate Demand

Before you write a single line of code, you need evidence that someone besides you wants this thing. Not certainty — evidence. The difference matters. Certainty is impossible. Evidence is achievable in 4 hours.

**Step 1: Search Volume Check (45 minutes)**

Go to these sources and search for your product idea and related terms:

- **Google Trends** (https://trends.google.com) — Free. Shows relative search interest over time. You want to see a flat or rising line, not a declining one.
- **Ahrefs Free Webmaster Tools** (https://ahrefs.com/webmaster-tools) — Free with site verification. Shows keyword volumes.
- **Ubersuggest** (https://neilpatel.com/ubersuggest/) — Free tier gives 3 searches/day. Shows search volume, difficulty, and related terms.
- **AlsoAsked** (https://alsoasked.com) — Free tier. Shows "People Also Ask" data from Google. Reveals what questions people are actually asking.

What you are looking for:

```
GOOD signals:
- 500+ monthly searches for your core keyword
- Rising trend over the last 12 months
- Multiple "People Also Ask" questions with no good answers
- Related long-tail keywords with low competition

BAD signals:
- Declining search interest
- Zero search volume (nobody is looking for this)
- Dominated by massive companies on page 1
- No variation in search terms (too narrow)
```

Real example: Suppose your Module R revenue engine idea is a "Tailwind CSS component library for SaaS dashboards."

```
Search: "tailwind dashboard components" — 2,900/month, rising trend
Search: "tailwind admin template" — 6,600/month, stable
Search: "react dashboard template tailwind" — 1,300/month, rising
Related: "shadcn dashboard", "tailwind analytics components"

Verdict: Strong demand. Multiple keyword angles. Proceed.
```

Another example: Suppose your idea is a "Rust-based log file anonymizer."

```
Search: "log file anonymizer" — 90/month, flat
Search: "anonymize log files" — 140/month, flat
Search: "PII removal from logs" — 320/month, rising
Related: "GDPR log compliance", "scrub PII from logs"

Verdict: Niche but growing. The "PII removal" angle has more volume
than the "anonymizer" angle. Reframe your positioning.
```

**Step 2: Community Thread Mining (60 minutes)**

Go to where developers ask for things and search for your problem space:

- **Reddit:** Search in r/webdev, r/reactjs, r/selfhosted, r/SideProject, r/programming, and niche subreddits relevant to your domain
- **Hacker News:** Use https://hn.algolia.com to search past discussions
- **GitHub Issues:** Search for issues in popular repos related to your space
- **Stack Overflow:** Search for questions with many upvotes but unsatisfying accepted answers
- **Discord servers:** Check relevant developer community servers

What you are documenting:

```markdown
## Thread Mining Results

### Thread 1
- **Source:** Reddit r/reactjs
- **URL:** [link]
- **Title:** "Is there a good Tailwind dashboard kit that isn't $200?"
- **Upvotes:** 147
- **Comments:** 83
- **Key quotes:**
  - "Everything on the market is either free and ugly, or $200+ and overkill"
  - "I just need 10-15 well-designed components, not 500"
  - "Would pay $49 for something that actually looks good out of the box"
- **Takeaway:** Price sensitivity at $200+, willingness to pay at $29-49

### Thread 2
- ...
```

Find at least 5 threads. If you cannot find 5 threads where people are asking for something in your product's space, that is a serious warning sign. Either the demand does not exist, or you are searching with the wrong terms. Try different keywords before giving up on the idea.

**Step 3: Competitor Audit (45 minutes)**

Search for what already exists. This is not discouraging — it is validating. Competitors mean there is a market. No competitors usually means there is no market, not that you found a blue ocean.

For each competitor, document:

```markdown
## Competitor Audit

### Competitor 1: [Name]
- **URL:** [link]
- **Price:** $XX
- **What they do well:** [specific things]
- **What sucks about it:** [specific complaints from reviews/threads]
- **Their reviews:** [check G2, ProductHunt reviews, Reddit mentions]
- **Your angle:** [how you would do it differently]

### Competitor 2: [Name]
- ...
```

The gold is in "what sucks about it." Every complaint about a competitor is a feature request for your product. People literally telling you what to build and what to charge.

**Step 4: The "10 People Would Pay" Test (30 minutes)**

This is the final validation gate. You need to find evidence that at least 10 people would pay money for this. Not "expressed interest." Not "said it was cool." Would pay.

Evidence sources:
- Reddit threads where people say "I would pay for X" (strongest signal)
- Competitor products with paying customers (proves the market pays)
- Gumroad/Lemon Squeezy products in your space with visible sales counts
- GitHub repos with 1,000+ stars that solve a related problem (people value this enough to star)
- Your own audience if you have one (tweet it, DM 10 people, ask directly)

If you pass this test: proceed. Build it.

If you fail this test: pivot your angle, not your entire idea. The demand might exist in an adjacent space. Try different positioning before abandoning.

> **Real Talk:** Most developers skip validation entirely because they want to code. They will spend 200 hours building something nobody asked for, then wonder why nobody buys it. These 4 hours of research will save you 196 hours of wasted effort. Do not skip this. The code is the easy part.

#### Afternoon Block (4 hours): Build the MVP

You have validated demand. You have competitor research. You know what people want and what existing solutions lack. Now build the minimum version that solves the core problem.

**The 3-Feature Rule**

Your v0.1 has exactly 3 features. Not 4. Not 7. Three.

How to pick them:
1. What is the ONE thing your product does? (Feature 1 — the core)
2. What makes it usable? (Feature 2 — usually auth, or save/export, or configuration)
3. What makes it worth paying for over alternatives? (Feature 3 — your differentiator)

Everything else goes on a "v0.2" list that you do not touch this weekend.

Real example — a Tailwind dashboard component library:
1. **Core:** 12 production-ready dashboard components (charts, tables, stat cards, nav)
2. **Usable:** Copy-paste code snippets with live preview
3. **Differentiator:** Dark mode built-in, components designed to work together (not a random collection)

Real example — a PII log scrubber CLI tool:
1. **Core:** Detect and redact PII from log files (emails, IPs, names, SSNs)
2. **Usable:** Works as a CLI pipe (`cat logs.txt | pii-scrub > clean.txt`)
3. **Differentiator:** Configurable rules file, handles 15+ log formats automatically

**Scaffold the Project**

Use LLMs to accelerate, not replace, your work. Here is the practical workflow:

```bash
# Scaffold a web app (SaaS tool, component library with docs site, etc.)
pnpm create vite@latest my-product -- --template react-ts
cd my-product
pnpm install

# Add Tailwind CSS (most common for developer products)
pnpm install -D tailwindcss @tailwindcss/vite

# Add routing if you need multiple pages
pnpm install react-router-dom

# Project structure — keep it flat for a 48-hour build
mkdir -p src/components src/pages src/lib
```

```bash
# Scaffold a CLI tool (for developer utilities)
cargo init my-tool
cd my-tool

# Common dependencies for CLI tools
cargo add clap --features derive    # Argument parsing
cargo add serde --features derive   # Serialization
cargo add serde_json                # JSON handling
cargo add anyhow                    # Error handling
cargo add regex                     # Pattern matching
```

```bash
# Scaffold an npm package (for libraries/utilities)
mkdir my-package && cd my-package
pnpm init
pnpm install -D typescript tsup vitest
mkdir src
```

**The LLM Workflow for Building**

Do not ask the LLM to build your entire product. That produces generic, fragile code. Instead:

1. **You** write the architecture: file structure, data flow, key interfaces
2. **LLM** generates boilerplate: repetitive components, utility functions, type definitions
3. **You** write the core logic: the part that makes your product different
4. **LLM** generates tests: unit tests, edge cases, integration tests
5. **You** review and edit everything: your name is on this product

Parallel work while you code: open a second LLM chat and have it draft your landing page copy, README, and documentation. You will edit these in the evening, but the first drafts will be ready.

**Time Discipline**

```
2:00 PM — Feature 1 (core functionality): 2 hours
           If it is not working by 4 PM, cut scope.
4:00 PM — Feature 2 (usability): 1 hour
           Keep it simple. Ship polish later.
5:00 PM — Feature 3 (differentiator): 1 hour
           This is what makes you worth paying for. Focus here.
6:00 PM — STOP CODING. It does not need to be perfect.
```

> **Common Mistake:** "Just one more feature before I stop." This is how weekend projects become month-long projects. The 3 features are your scope. If you think of a great idea during the build, write it on your v0.2 list and keep moving. You can add it next week after you have paying customers.

#### Evening Block (2 hours): Write the Landing Page

Your landing page has one job: convince a visitor to pay. It does not need to be beautiful. It needs to be clear.

**The 5-Section Landing Page**

Every successful developer product landing page follows this structure. Do not reinvent it:

```
Section 1: HEADLINE + SUBHEADLINE
  - What it does in 8 words or less
  - Who it is for and what outcome they get

Section 2: THE PROBLEM
  - 3 pain points your target customer recognizes
  - Use their exact language from your thread mining

Section 3: THE SOLUTION
  - Screenshots or code examples of your product
  - 3 features mapped to the 3 pain points above

Section 4: PRICING
  - One or two tiers. Keep it simple for v0.1.
  - Annual billing option if it's a subscription.

Section 5: CTA (Call to Action)
  - One button. "Get Started", "Buy Now", "Download".
  - Repeat the core benefit.
```

**Real Copy Example — Tailwind Dashboard Kit:**

```markdown
# Section 1
## DashKit — Production Tailwind Dashboard Components
Ship your SaaS dashboard in hours, not weeks.
12 copy-paste components. Dark mode. $29.

# Section 2
## The Problem
- Generic UI kits give you 500 components but zero cohesion
- Building dashboard UIs from scratch takes 40+ hours
- Free options look like Bootstrap from 2018

# Section 3
## What You Get
- **12 components** designed to work together (not a random collection)
- **Dark mode** built-in — toggle with one prop
- **Copy-paste code** — no npm install, no dependencies, no lock-in
[screenshot of component examples]

# Section 4
## Pricing
**DashKit** — $29 one-time
- All 12 components with source code
- Free updates for 12 months
- Use in unlimited projects

**DashKit Pro** — $59 one-time
- Everything in DashKit
- 8 full-page templates (analytics, CRM, admin, settings)
- Figma design files
- Priority feature requests

# Section 5
## Ship your dashboard this weekend.
[Buy DashKit — $29]
```

**Real Copy Example — PII Log Scrubber:**

```markdown
# Section 1
## ScrubLog — Strip PII from Log Files in Seconds
GDPR compliance for your logs. One command.

# Section 2
## The Problem
- Your logs contain emails, IPs, and names you should not be storing
- Manual redaction takes hours and misses things
- Enterprise tools cost $500/month and require a PhD to configure

# Section 3
## How It Works
```bash
cat server.log | scrublog > clean.log
```
- Detects 15+ PII patterns automatically
- Custom rules via YAML config
- Handles JSON, Apache, Nginx, and plaintext formats
[terminal screenshot showing before/after]

# Section 4
## Pricing
**Personal** — Free
- 5 PII patterns, 1 log format

**Pro** — $19/month
- All 15+ PII patterns
- All log formats
- Custom rules
- Team config sharing

# Section 5
## Stop storing PII you do not need.
[Get ScrubLog Pro — $19/month]
```

**LLM Workflow for Copy:**

1. Feed the LLM your competitor audit and thread mining results
2. Ask it to draft landing page copy using the 5-section template
3. Edit ruthlessly: replace every vague phrase with a specific one
4. Read it aloud. If any sentence makes you cringe, rewrite it.

**Building the Landing Page:**

For a 48-hour sprint, do not build a custom landing page from scratch. Use one of these:

- **Your product's own site** — If it is a web app, make the landing page the logged-out homepage
- **Astro + Tailwind** — Static site, deploys to Vercel in 2 minutes, extremely fast
- **Next.js** — If your product is already React, add a marketing page route
- **Framer** (https://framer.com) — Visual builder, exports clean code, free tier available
- **Carrd** (https://carrd.co) — $19/year, dead simple one-page sites

```bash
# The fastest path: Astro static site
pnpm create astro@latest my-product-site
cd my-product-site
pnpm install
# Add Tailwind
pnpm astro add tailwind
```

You should have a landing page with copy by the end of Saturday. It does not need custom illustrations. It does not need animations. It needs clear words and a buy button.

### Day 2 — Sunday

#### Morning Block (3 hours): Deploy

Your product needs to be live on the internet at a real URL. Not localhost. Not a Vercel preview URL with a random hash. A real domain, with HTTPS, that you can share and people can visit.

**Step 1: Deploy the Application (60 minutes)**

Choose your deployment platform based on what you built:

**Static site / SPA (component library, landing page, docs site):**
```bash
# Vercel — the fastest path for static sites and Next.js
pnpm install -g vercel
vercel

# It will ask you questions. Say yes to everything.
# Your site is live in ~60 seconds.
```

**Web app with a backend (SaaS tool, API service):**
```bash
# Railway — simple, good free tier, handles databases
# https://railway.app
# Connect your GitHub repo and deploy.

# Or Fly.io — more control, global edge deployment
# https://fly.io
curl -L https://fly.io/install.sh | sh
fly launch
fly deploy
```

**CLI tool / npm package:**
```bash
# npm registry
npm publish

# Or distribute as a binary via GitHub Releases
# Use cargo-dist for Rust projects
cargo install cargo-dist
cargo dist init
cargo dist build
# Upload binaries to GitHub release
```

**Step 2: Buy a Domain (30 minutes)**

A real domain costs $12/year. If you cannot invest $12 in your business, you are not serious about having a business.

**Where to buy:**
- **Namecheap** (https://namecheap.com) — $8-12/year for .com, good DNS management
- **Cloudflare Registrar** (https://dash.cloudflare.com) — At-cost pricing (often $9-10/year for .com), excellent DNS
- **Porkbun** (https://porkbun.com) — Often cheapest for first year, good UI

**Domain naming tips:**
- Shorter is better. 2 syllables ideal, 3 max.
- `.com` still wins for trust. `.dev` and `.io` are fine for developer tools.
- Check availability on your registrar, not on GoDaddy (they front-run searches).
- Do not spend more than 15 minutes choosing. The name matters less than you think.

```bash
# Point your domain to Vercel
# In Vercel dashboard: Settings > Domains > Add your domain
# Then in your registrar DNS settings, add:
# A record: @ -> 76.76.21.21
# CNAME record: www -> cname.vercel-dns.com

# Or if using Cloudflare for DNS:
# Just add the same records in Cloudflare's DNS panel
# SSL is automatic with both Vercel and Cloudflare
```

**Step 3: Basic Monitoring (30 minutes)**

You need to know two things: is the site up, and are people visiting.

**Uptime monitoring (free):**
- **Better Uptime** (https://betteruptime.com) — Free tier monitors 10 URLs every 3 minutes
- **UptimeRobot** (https://uptimerobot.com) — Free tier monitors 50 URLs every 5 minutes

```
Set up monitoring for:
1. Your landing page URL
2. Your app's health endpoint (if applicable)
3. Your payment webhook URL (critical — you need to know if payments break)
```

**Analytics (privacy-respecting):**

Do not use Google Analytics. Your developer audience blocks it, it is overkill for a new product, and it is a privacy liability.

- **Plausible** (https://plausible.io) — $9/month, privacy-first, one-line script
- **Fathom** (https://usefathom.com) — $14/month, privacy-first, lightweight
- **Umami** (https://umami.is) — Free and self-hosted, or $9/month cloud

```html
<!-- Plausible — one line in your <head> -->
<script defer data-domain="yourdomain.com"
  src="https://plausible.io/js/script.js"></script>

<!-- Umami — one line in your <head> -->
<script defer
  src="https://your-umami-instance.com/script.js"
  data-website-id="your-website-id"></script>
```

> **Real Talk:** Yes, $9/month for analytics on a product that has not made money yet feels unnecessary. But you cannot improve what you cannot measure. The first month of analytics data will tell you more about your market than a month of guessing. If $9/month breaks your budget, self-host Umami for free on Railway.

#### Afternoon Block (2 hours): Set Up Payments

If your product cannot accept money, it is a hobby project. Setting up payments takes less time than most developers think — about 20-30 minutes for the basic flow.

**Option A: Lemon Squeezy (Recommended for Digital Products)**

Lemon Squeezy (https://lemonsqueezy.com) handles payment processing, sales tax, VAT, and digital delivery in one platform. It is the fastest path from zero to accepting payments.

Why Lemon Squeezy over Stripe for your first product:
- Acts as Merchant of Record — they handle sales tax, VAT, and compliance for you
- Built-in checkout pages — no frontend work needed
- Digital delivery built in — upload your files, they handle access
- 5% + $0.50 per transaction (higher than Stripe, but saves you hours of tax headaches)

Setup walkthrough:
1. Sign up at https://app.lemonsqueezy.com
2. Create a Store (your business name)
3. Add a Product:
   - Name, description, price
   - Upload files for digital delivery (if applicable)
   - Set up license keys (if selling software)
4. Get your checkout URL — this is what your "Buy" button links to
5. Set up a webhook for post-purchase automation

```javascript
// Lemon Squeezy webhook handler (Node.js/Express)
// POST /api/webhooks/lemonsqueezy

import crypto from 'crypto';

const WEBHOOK_SECRET = process.env.LEMONSQUEEZY_WEBHOOK_SECRET;

export async function handleLemonSqueezyWebhook(req, res) {
  // Verify webhook signature
  const signature = req.headers['x-signature'];
  const hmac = crypto.createHmac('sha256', WEBHOOK_SECRET);
  const digest = hmac.update(JSON.stringify(req.body)).digest('hex');

  if (signature !== digest) {
    return res.status(401).json({ error: 'Invalid signature' });
  }

  const event = req.body;

  switch (event.meta.event_name) {
    case 'order_created': {
      const order = event.data;
      const customerEmail = order.attributes.user_email;
      const productId = order.attributes.first_order_item.product_id;
      const orderId = order.id;

      console.log(`New order: ${orderId} from ${customerEmail}`);

      // Send welcome email, grant access, create license key, etc.
      await grantProductAccess(customerEmail, productId);
      await sendWelcomeEmail(customerEmail, orderId);

      break;
    }

    case 'subscription_created': {
      const subscription = event.data;
      const customerEmail = subscription.attributes.user_email;

      console.log(`New subscription from ${customerEmail}`);
      await createSubscription(customerEmail, subscription);

      break;
    }

    case 'subscription_cancelled': {
      const subscription = event.data;
      const customerEmail = subscription.attributes.user_email;

      console.log(`Subscription cancelled: ${customerEmail}`);
      await revokeAccess(customerEmail);

      break;
    }

    default:
      console.log(`Unhandled event: ${event.meta.event_name}`);
  }

  return res.status(200).json({ received: true });
}
```

**Option B: Stripe (More Control, More Work)**

Stripe (https://stripe.com) gives you more control but requires you to handle tax compliance separately. Better for SaaS with complex billing.

```javascript
// Stripe Checkout session (Node.js)
// Creates a hosted checkout page

import Stripe from 'stripe';

const stripe = new Stripe(process.env.STRIPE_SECRET_KEY);

export async function createCheckoutSession(req, res) {
  const session = await stripe.checkout.sessions.create({
    payment_method_types: ['card'],
    line_items: [
      {
        price_data: {
          currency: 'usd',
          product_data: {
            name: 'DashKit Pro',
            description: '12 Tailwind dashboard components + 8 templates + Figma files',
          },
          unit_amount: 5900, // $59.00 in cents
        },
        quantity: 1,
      },
    ],
    mode: 'payment', // 'subscription' for recurring
    success_url: `${process.env.DOMAIN}/success?session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${process.env.DOMAIN}/pricing`,
    customer_email: req.body.email, // Pre-fill if you have it
  });

  return res.json({ url: session.url });
}

// Stripe webhook handler
export async function handleStripeWebhook(req, res) {
  const sig = req.headers['stripe-signature'];

  let event;
  try {
    event = stripe.webhooks.constructEvent(
      req.body, // raw body, not parsed JSON
      sig,
      process.env.STRIPE_WEBHOOK_SECRET
    );
  } catch (err) {
    console.error(`Webhook signature verification failed: ${err.message}`);
    return res.status(400).send(`Webhook Error: ${err.message}`);
  }

  switch (event.type) {
    case 'checkout.session.completed': {
      const session = event.data.object;
      await fulfillOrder(session);
      break;
    }
    case 'customer.subscription.deleted': {
      const subscription = event.data.object;
      await revokeSubscriptionAccess(subscription);
      break;
    }
  }

  return res.json({ received: true });
}
```

**For Both Platforms — Test Before You Launch:**

```bash
# Lemon Squeezy: Use test mode in the dashboard
# Toggle "Test mode" in the top-right of the Lemon Squeezy dashboard
# Use card number: 4242 4242 4242 4242, any future expiry, any CVC

# Stripe: Use test mode API keys
# Test card: 4242 4242 4242 4242
# Test declining card: 4000 0000 0000 0002
# Test card requiring auth: 4000 0025 0000 3155
```

Run through the entire purchase flow yourself in test mode. Click the buy button, complete the checkout, verify the webhook fires, verify access is granted. If any step fails in test mode, it will fail for real customers.

> **Common Mistake:** "I'll set up payments later, after I get some users." This is backwards. Setting up payments is not about collecting money today — it is about validating whether anyone will pay. A product without a price is a free tool. A product with a price is a business test. The price itself is part of the validation.

#### Evening Block (3 hours): Launch

Your product is live. Payments work. The landing page is clear. Now you need humans to see it.

**The Soft Launch Strategy**

Do not do a "big launch" for your first product. Big launches create pressure to be perfect, and your v0.1 is not perfect. Instead, do a soft launch: share it in a few places, gather feedback, fix critical issues, then do the big launch in 1-2 weeks.

**Launch Platform 1: Reddit (30 minutes)**

Post in r/SideProject and one niche subreddit relevant to your product.

Reddit post template:

```markdown
Title: I built [what it does] in a weekend — [key benefit]

Body:
Hey [subreddit],

I've been frustrated with [the problem] for a while, so I built
[product name] this weekend.

**What it does:**
- [Feature 1 — the core value]
- [Feature 2]
- [Feature 3]

**What makes it different from [competitor]:**
[One honest paragraph about your differentiator]

**Pricing:**
[Be transparent. "$29 one-time" or "Free tier + $19/mo Pro"]

I'd love feedback. What am I missing? What would make this
useful for your workflow?

[Link to product]
```

Rules for Reddit posts:
- Be genuinely helpful, not salesy
- Respond to every single comment (this is not optional)
- Accept criticism gracefully — negative feedback is the most valuable kind
- Do not astroturf (fake upvotes, multiple accounts). You will get caught and banned.

**Launch Platform 2: Hacker News (30 minutes)**

If your product is technical and interesting, post a Show HN.

Show HN template:

```markdown
Title: Show HN: [Product Name] – [what it does in <70 characters]

Body:
[Product name] is [one sentence explaining what it does].

I built this because [genuine motivation — what problem you were solving
for yourself].

Technical details:
- Built with [stack]
- [Interesting technical decision and why]
- [What makes the implementation noteworthy]

Try it: [URL]

Feedback welcome. I'm particularly interested in [specific question for
the HN audience].
```

HN tips:
- Post between 7-9 AM US Eastern Time (highest traffic)
- The title matters more than anything. Be specific and technical.
- HN readers respect technical substance over marketing polish
- Respond to comments immediately in the first 2 hours. Comment velocity affects ranking.
- Do not beg for upvotes. Just post and engage.

**Launch Platform 3: Twitter/X (30 minutes)**

Write a build-in-public launch thread:

```
Tweet 1 (Hook):
I built [product] in 48 hours this weekend.

It [solves specific problem] for [specific audience].

Here's what I shipped, what I learned, and the real numbers. Thread:

Tweet 2 (The Problem):
The problem:
[Describe the pain point in 2-3 sentences]
[Include a screenshot or code example showing the pain]

Tweet 3 (The Solution):
So I built [product name].

[Screenshot/GIF of the product in action]

It does three things:
1. [Feature 1]
2. [Feature 2]
3. [Feature 3]

Tweet 4 (Technical Detail):
Tech stack for the nerds:
- [Frontend]
- [Backend]
- [Hosting — mention specific platform]
- [Payments — mention Lemon Squeezy/Stripe]
- Total cost to run: $XX/month

Tweet 5 (Pricing):
Pricing:
[Clear pricing, same as landing page]
[Link to product]

Tweet 6 (Ask):
Would love feedback from anyone who [describes target user].

What am I missing? What would make this a must-have for you?
```

**Launch Platform 4: Relevant Communities (30 minutes)**

Identify 2-3 communities where your target audience hangs out:

- Discord servers (developer communities, framework-specific servers)
- Slack communities (many niche dev communities have Slack groups)
- Dev.to / Hashnode (write a short "I built this" post)
- Indie Hackers (https://indiehackers.com) — specifically designed for this
- Relevant Telegram or WhatsApp groups

**First 48 Hours After Launch — What to Watch:**

```
Metrics to track:
1. Unique visitors (from analytics)
2. Landing page → checkout click rate (should be 2-5%)
3. Checkout → purchase conversion rate (should be 1-3%)
4. Bounce rate (above 80% means your headline/hero is wrong)
5. Traffic sources (where are your visitors coming from?)
6. Comments and feedback (qualitative — what are people saying?)

Sample math:
- 500 visitors in 48 hours (reasonable from Reddit + HN + Twitter)
- 3% click "Buy" = 15 checkout visits
- 10% complete purchase = 1-2 sales
- At $29/sale = $29-58 in your first weekend

That is not retirement money. It is VALIDATION money.
$29 from a stranger on the internet proves your product has value.
```

Do not panic if you get zero sales in the first 48 hours. Look at your funnel:
- Zero visitors? Your distribution is the problem, not your product.
- Visitors but zero clicks on "Buy"? Your copy or price is the problem.
- Clicks on "Buy" but zero completions? Your checkout flow is broken or your price is too high for the perceived value.

Each of these has a different fix. That is why metrics matter.

### Your Turn

1. **Block the time.** Open your calendar right now and block out next Saturday 8 AM to 8 PM and Sunday 8 AM to 8 PM. Label it "48-Hour Sprint." Treat it like a flight you cannot reschedule.

2. **Pick your idea.** Choose one revenue engine from Module R. Write down the 3-feature scope for your v0.1. If you cannot pick one, pick the one you could explain to a non-developer in one sentence.

3. **Pre-work.** Before Saturday, create accounts on:
   - Vercel, Railway, or Fly.io (deployment)
   - Lemon Squeezy or Stripe (payments)
   - Namecheap, Cloudflare, or Porkbun (domain)
   - Plausible, Fathom, or Umami (analytics)
   - Better Uptime or UptimeRobot (monitoring)

   Do this on a weeknight so Saturday is pure building, not account creation.

4. **Prepare your launch platforms.** If you do not have a Reddit account with some karma, start participating in relevant subreddits this week. Accounts that only post self-promotion get flagged. If you do not have a Hacker News account, create one and participate in a few discussions first.

---

## Lesson 2: The "Ship, Then Improve" Mindset

*"v0.1 with 3 features beats v1.0 that never ships."*

### The Perfectionism Trap

Developers are uniquely susceptible to a specific failure mode: building in private forever. We know what "good code" looks like. We know our v0.1 is not good code. So we refactor. We add error handling. We write more tests. We improve the architecture. We do everything except the one thing that matters: showing it to humans.

Here is a truth that will save you thousands of hours: **your customers do not read your source code.** They do not care about your architecture. They do not care about your test coverage. They care about one thing: does this solve my problem?

A product with spaghetti code that solves a real problem will make money. A product with beautiful architecture that solves no problem will make nothing.

This is not an excuse for writing bad code. It is a priority statement. Ship first. Refactor second. The refactoring will be better informed by real usage data anyway.

### How "Ship, Then Improve" Plays Out

Consider this scenario: a developer launches a Notion template pack for software engineering managers. Here is what it looks like at launch:

- 5 templates (not 50)
- A Gumroad page with a paragraph of description and 3 screenshots
- No custom website
- No email list
- No social media following
- Price: $29

They post it on Reddit and Twitter. That is the entire marketing strategy.

Month 1 results:
- ~170 sales at $29 = ~$5,000
- After Gumroad's cut (10%): ~$4,500
- Time invested: ~30 hours total (building templates + writing descriptions)
- Effective hourly rate: ~$150/hour

Was it "perfect"? No. The templates had formatting inconsistencies. Some of the descriptions were generic. Customers did not care. They cared that it saved them from building the templates themselves.

By month 3, based on customer feedback, the developer:
- Fixed the formatting issues
- Added more templates (the ones customers specifically asked for)
- Raised the price to $39 (existing customers got updates free)
- Created a "Pro" tier with an accompanying video walkthrough

The product they launched was worse in every way than the product they had 90 days later. But the 90-day version only existed because the launch version generated the feedback and revenue to guide development.

> **NOTE:** For real-world validation of the "ship ugly, improve fast" model: Josh Comeau pre-sold $550K of his CSS for JavaScript Developers course in the first week (Source: failory.com). Wes Bos has generated $10M+ in total developer course sales using iterative launches (Source: foundershut.com). Both started with imperfect v1 products and iterated based on real customer feedback.

### First 10 Customers Tell You Everything

Your first 10 paying customers are the most important people in your business. Not because of their money — 10 sales at $29 is $290, which buys you groceries. They are important because they are volunteers for your product development team.

What to do with your first 10 customers:

1. **Send a personal thank-you email.** Not automated. Personal. "Hey, I saw you purchased [product]. Thank you. I'm actively developing this — is there anything you wish it did that it doesn't?"

2. **Read every reply.** Some will not reply. Some will reply with "looks great, thanks." But 2-3 out of 10 will write paragraphs about what they want. Those paragraphs are your roadmap.

3. **Look for patterns.** If 3 out of 10 people ask for the same feature, build it. That is a 30% demand signal from paying customers. No survey will give you data that good.

4. **Ask about their willingness to pay more.** "I'm planning a Pro tier with [feature X]. Would that be worth $49 to you?" Direct. Specific. Gives you pricing data.

```
Email template for first 10 customers:

Subject: Quick question about [product name]

Hey [name],

I noticed you picked up [product name] — thanks for being
one of the first customers.

I'm building this actively and shipping updates weekly.
Quick question: what's the ONE thing you wish it did that
it doesn't?

No wrong answers. Even if it seems like a big ask,
I want to hear it.

Thanks,
[Your name]
```

### How to Handle Negative Feedback

Your first piece of negative feedback will feel personal. It is not personal. It is data.

**Framework for processing negative feedback:**

```
1. PAUSE. Do not respond for 30 minutes. Your emotional reaction
   is not useful.

2. CATEGORIZE the feedback:
   a) Bug report — fix it. Thank them.
   b) Feature request — add to backlog. Thank them.
   c) Pricing complaint — note it. Check if it's a pattern.
   d) Quality complaint — investigate. Is it valid?
   e) Troll/unreasonable — ignore. Move on.

3. RESPOND (for a, b, c, d only):
   "Thanks for the feedback. [Acknowledge the specific issue].
   I'm [fixing it now / adding it to the roadmap / looking into this].
   I'll let you know when it's addressed."

4. ACT. If you promised to fix something, fix it within a week.
   Nothing builds loyalty faster than showing customers their
   feedback leads to real changes.
```

> **Real Talk:** You will get someone who says your product is garbage. It will sting. But if your product is live and making money, you have already done something most developers never do. The person criticizing from the comments section has not shipped anything. You have. Keep shipping.

### The Weekly Iteration Cycle

After launch, your workflow becomes a tight loop:

```
Monday:    Review last week's metrics and customer feedback
Tuesday:   Plan this week's improvement (ONE thing, not five)
Wednesday: Build the improvement
Thursday:  Test and deploy the improvement
Friday:    Write a changelog/update post
Weekend:   Marketing — one blog post, one social post, one community interaction

Repeat.
```

The key word is ONE improvement per week. Not a feature overhaul. Not a redesign. One thing that makes the product slightly better for your existing customers. Over 12 weeks, that is 12 improvements guided by real usage data. Your product after 12 weeks of this cycle will be dramatically better than anything you could have designed in isolation.

### Revenue Validates Faster Than Surveys

Surveys lie. Not intentionally — people are just bad at predicting their own behavior. "Would you pay $29 for this?" gets easy "yes" answers. But "here is the checkout page, enter your credit card" gets honest answers.

This is why you launch with payments from day one:

| Validation Method | Time to Signal | Signal Quality |
|---|---|---|
| Survey / poll | 1-2 weeks | Low (people lie) |
| Landing page with email signup | 1-2 weeks | Medium (interest, not commitment) |
| Landing page with price but no checkout | 1 week | Medium-High (price acceptance) |
| **Live product with real checkout** | **48 hours** | **Highest (actual purchasing behavior)** |

The $0 price reveals nothing. The $29 price reveals everything.

### Your Turn

1. **Write your "ugly launch" commitment.** Open a text file and write: "I will launch [product name] on [date] even if it is not perfect. v0.1 scope: [3 features]. I will not add Feature 4 before launch." Sign it (metaphorically). Refer to it when the urge to polish strikes.

2. **Draft your first-10-customers email.** Write the personal thank-you email template now, before you have customers. When the first sale comes in, you want to send it within the hour.

3. **Set up your iteration tracker.** Create a simple spreadsheet or Notion page with columns: Week | Improvement Made | Metric Impact | Customer Feedback. This becomes your decision log for what to build next.

---

## Lesson 3: Pricing Psychology for Developer Products

*"$0 is not a price. It is a trap."*

### Why Free Is Expensive

The most counterintuitive truth in selling developer products: **free users cost you more than paying customers.**

Free users:
- File more support requests (they have no skin in the game)
- Demand more features (they feel entitled because they are not paying)
- Provide less useful feedback ("it's cool" is not actionable)
- Churn at higher rates (there is no switching cost)
- Tell fewer people about your product (free things have low perceived value)

Paying customers:
- Are invested in your success (they want their purchase to be a good decision)
- Provide specific, actionable feedback (they want the product to improve)
- Are easier to retain (they already decided to pay; inertia works in your favor)
- Refer others more often (recommending something you paid for validates your purchase)
- Respect your time (they understand you are running a business)

The only reason to offer a free tier is as a lead generation mechanism for the paid tier. If your free tier is good enough that people never upgrade, you do not have a free tier — you have a free product with a donation button.

> **Common Mistake:** "I'll make it free to get users first, then charge later." This almost never works. The users you attract at $0 expect $0 forever. When you add a price, they leave. The users who would have paid $29 from day one never found your product because you positioned it as a free tool. You attracted the wrong audience.

### The Developer Product Pricing Tiers

After analyzing hundreds of successful developer products, these price points consistently work:

**Tier 1: $9-29 — Developer Tools and Utilities**

Products in this range solve a specific, narrow problem. A single purchase, use it today.

```
Examples:
- VS Code extension with premium features: $9-15
- CLI tool with pro features: $15-19
- Single-purpose SaaS tool: $9-19/month
- Small component library: $19-29
- Browser DevTools extension: $9-15

Buyer psychology: Impulse purchase territory. Developer sees it,
recognizes the problem, buys it without asking their manager.
No budget approval needed. Credit card → done.

Key insight: At this price, your landing page must convert in
under 2 minutes. The buyer will not read a long feature list.
Show the problem, show the solution, show the price.
```

**Tier 2: $49-99 — Templates, Kits, and Comprehensive Tools**

Products in this range save significant time. Multiple components working together.

```
Examples:
- Full UI template kit: $49-79
- SaaS boilerplate with auth, billing, dashboards: $79-99
- Comprehensive icon/illustration set: $49-69
- Multi-purpose CLI toolkit: $49
- API wrapper library with extensive docs: $49-79

Buyer psychology: Considered purchase. Developer evaluates for
5-10 minutes. Compares to alternatives. Calculates time saved.
"If this saves me 10 hours and I value my time at $50/hour,
$79 is a no-brainer."

Key insight: You need a comparison point. Show the time/effort
it takes to build this from scratch vs. buying your kit.
Include testimonials if you have them.
```

**Tier 3: $149-499 — Courses, Comprehensive Solutions, Premium Templates**

Products in this range transform a skill or provide a complete system.

```
Examples:
- Video course (10+ hours): $149-299
- SaaS starter kit with full source + video walkthrough: $199-299
- Enterprise component library: $299-499
- Comprehensive developer toolkit (multiple tools): $199
- "Build X from Scratch" complete codebase + lessons: $149-249

Buyer psychology: Investment purchase. Buyer needs to justify
the expense (to themselves or their manager). They need social
proof, detailed previews, and a clear ROI narrative.

Key insight: At this tier, offer a money-back guarantee.
It reduces purchase anxiety and increases conversions. Refund
rates for digital developer products are typically 3-5%.
The increased conversions far outweigh the refunds.
```

### The 3-Tier Pricing Strategy

If your product supports it, offer three pricing tiers. This is not random — it exploits a well-documented cognitive bias called the "center stage effect." When presented with three options, most people choose the middle one.

```
Tier structure:

BASIC          PRO (highlighted)     TEAM/ENTERPRISE
$29             $59                   $149
Core features   Everything in Basic   Everything in Pro
                + premium features    + team features
                + priority support    + commercial license

Conversion distribution (typical):
- Basic: 20-30%
- Pro: 50-60% ← this is your target
- Team: 10-20%
```

**How to design the tiers:**

1. Start with the **Pro** tier. This is the product you actually want to sell, at the price that reflects its value. Design this first.

2. Create the **Basic** tier by removing features from Pro. Remove enough that Basic solves the problem but Pro solves it *well*. Basic should feel slightly frustrating — usable, but clearly limited.

3. Create the **Team** tier by adding features to Pro. Multi-seat licensing, commercial use rights, priority support, custom branding, source code access, Figma files, etc.

**Real pricing page example:**

```
DashKit

STARTER — $29                    PRO — $59                        TEAM — $149
                                 ★ Most Popular                   Best for agencies

✓ 12 core components            ✓ Everything in Starter           ✓ Everything in Pro
✓ React + TypeScript             ✓ 8 full-page templates           ✓ Up to 5 team members
✓ Dark mode                      ✓ Figma design files              ✓ Commercial license
✓ npm install                    ✓ Advanced data table              (unlimited client projects)
✓ 6 months of updates           ✓ Chart library integration       ✓ Priority support
                                 ✓ 12 months of updates            ✓ Lifetime updates
                                 ✓ Priority feature requests       ✓ Custom branding options

[Get Starter]                    [Get Pro]                         [Get Team]
```

### Pricing Anchoring

Anchoring is the cognitive bias where the first number people see influences their perception of subsequent numbers. Use it ethically:

1. **Show the expensive option first** (on the right in Western layouts). Seeing $149 makes $59 feel reasonable.

2. **Show "hours saved" calculations.**
   ```
   "Building these components from scratch takes ~40 hours.
   At $50/hour, that's $2,000 of your time.
   DashKit Pro: $59."
   ```

3. **Use "per day" reframing for subscriptions.**
   ```
   "$19/month" → "Less than $0.63/day"
   "$99/year" → "$8.25/month" or "$0.27/day"
   ```

4. **Annual billing discount.** Offer 2 months free on annual plans. This is standard and expected. Annual billing reduces churn by 30-40% because cancellation requires a conscious decision at a single renewal point, not an ongoing monthly decision.

```
Monthly: $19/month
Annual: $190/year (save $38 — 2 months free)

Display as:
Monthly: $19/month
Annual: $15.83/month (billed annually at $190)
```

### A/B Testing Prices

Testing prices is valuable but tricky. Here is how to do it without being dishonest:

**Acceptable approaches:**
- Test different prices on different launch channels (Reddit gets $29, Product Hunt gets $39, see which converts better)
- Change your price after 2 weeks and compare conversion rates
- Offer a launch discount ("$29 this week, $39 after") and see if the urgency changes behavior
- Test different tier structures (2 tiers vs 3 tiers) across time periods

**Not acceptable:**
- Showing different prices to different visitors on the same page at the same time (price discrimination, erodes trust)
- Charging more based on location or browser detection (people talk, and you will get caught)

### When to Raise Prices

Raise your prices when any of these are true:

1. **Conversion rate is above 5%.** You are too cheap. A healthy conversion rate for a developer product landing page is 1-3%. Above 5% means almost everyone who sees the price agrees it is a good deal — which means you are leaving money on the table.

2. **No one has complained about the price.** If zero people out of 100 say it is too expensive, it is too cheap. A healthy product has about 20% of visitors thinking the price is high. That means 80% think it is fair or a good deal.

3. **You have added significant features since launch.** You launched at $29 with 3 features. You now have 8 features and better documentation. The product is worth more. Charge more.

4. **You have testimonials and social proof.** Perceived value increases with social proof. Once you have 5+ positive reviews, your product is worth more in the buyer's mind.

**How to raise prices:**
- Announce the price increase 1-2 weeks in advance ("Price going from $29 to $39 on [date]")
- Grandfather existing customers at the old price
- This is not shady — it is standard practice and also creates urgency for fence-sitters

> **Real Talk:** Most developers underprice by 50-200%. Your $29 product is probably worth $49. Your $49 product is probably worth $79. I know this because developers anchor to their own willingness to pay (low — we are cheapskates about tooling) rather than the customer's willingness to pay (higher — they are buying a solution to a problem that costs them time). Raise your prices sooner than you think.

### Your Turn

1. **Price your product.** Based on the tier analysis above, pick a price point for your v0.1 launch. Write it down. If you feel uncomfortable because it seems "too high," you are probably in the right range. If it feels comfortable, add 50%.

2. **Design your pricing page.** Using the 3-tier template, design your pricing page copy. Identify which features go in each tier. Identify your "highlighted" tier (the one you want most people to buy).

3. **Calculate your math.** Fill in:
   - Price per sale: $___
   - Target monthly revenue: $___
   - Number of sales needed per month: ___
   - Estimated landing page visitors needed (at 2% conversion): ___
   - Is that visitor count achievable with your distribution plan? (Yes/No)

---

## Lesson 4: Legal Minimum Viable Setup

*"30 minutes of legal setup now saves you 30 hours of panic later."*

### The Honest Truth About Legal Setup

Most developers either ignore legal entirely (risky) or get paralyzed by it (wasteful). The right approach is a minimum viable legal setup: enough protection to operate legitimately, without spending $5,000 on a lawyer before you have made $5.

Here is what you actually need before your first sale, what you need before your 100th sale, and what you do not need until much later.

### Before Your First Sale (Do This Weekend)

**1. Check Your Employment Contract (30 minutes)**

If you have a full-time job, read your employment contract's IP clause before building anything. Specifically look for:

- **Assignment of inventions clauses:** Some contracts say everything you create while employed — including on your own time — belongs to your employer.
- **Non-compete clauses:** Some restrict you from working in the same industry, even as a side project.
- **Moonlighting policies:** Some require written approval for outside business activities.

```
What you are looking for:

SAFE: "Inventions made on company time or using company resources
belong to the company." → Your weekend project on your personal
machine is yours.

MURKY: "All inventions related to the company's current or
anticipated business." → If your side project is in the same
domain as your employer, get legal advice.

RESTRICTIVE: "All inventions conceived during the period of
employment belong to the company." → This is aggressive but
common at some companies. Get legal advice before proceeding.
```

States like California, Delaware, Illinois, Minnesota, Washington, and others have laws that limit how broadly employers can claim your personal inventions. But the specific language of your contract matters.

> **Common Mistake:** "I'll just keep it secret." If your product gets successful enough to matter, someone will notice. If it violates your employment contract, you could lose the product AND your job. 30 minutes of reading your contract now prevents this.

**2. Privacy Policy (15 minutes)**

If your product collects any data — even just an email address for purchase — you need a privacy policy. This is a legal requirement in the EU (GDPR), California (CCPA), and increasingly everywhere else.

Do not write one from scratch. Use a generator:

- **Termly** (https://termly.io/products/privacy-policy-generator/) — Free tier, answer questions, get a policy
- **Avodocs** (https://www.avodocs.com) — Free, open-source legal templates
- **Iubenda** (https://www.iubenda.com) — Free tier, auto-generates based on your tech stack

Your privacy policy must cover:

```markdown
# Privacy Policy for [Product Name]
Last updated: [Date]

## What We Collect
- Email address (for purchase confirmation and product updates)
- Payment information (processed by [Lemon Squeezy/Stripe],
  we never see or store your card details)
- Basic usage analytics (page views, feature usage — via
  [Plausible/Fathom/Umami], privacy-respecting, no cookies)

## What We Do NOT Collect
- We do not track you across the web
- We do not sell your data to anyone
- We do not use advertising cookies

## How We Use Your Data
- To deliver the product you purchased
- To send product updates and important notices
- To improve the product based on aggregate usage patterns

## Data Storage
- Your data is stored on [hosting provider] servers in [region]
- Payment data is handled entirely by [Lemon Squeezy/Stripe]

## Your Rights
- You can request a copy of your data at any time
- You can request deletion of your data at any time
- Contact: [your email]

## Changes
- We will notify you of significant changes via email
```

Put this at `yourdomain.com/privacy`. Link to it from your checkout page footer.

**3. Terms of Service (15 minutes)**

Your terms of service protect you from unreasonable claims. For a digital product, they are straightforward.

```markdown
# Terms of Service for [Product Name]
Last updated: [Date]

## License
When you purchase [Product Name], you receive a license to use
it for [personal/commercial] purposes.

- **Single license:** Use in your own projects (unlimited)
- **Team license:** Use by up to [N] team members
- You may NOT redistribute, resell, or share access credentials

## Refunds
- Digital products: [30-day / 14-day] money-back guarantee
- If you are not satisfied, email [your email] for a full refund
- No questions asked within the refund window

## Liability
- [Product Name] is provided "as is" without warranty
- We are not liable for damages arising from use of the product
- Maximum liability is limited to the amount you paid

## Support
- Support is provided via email at [your email]
- We aim to respond within [48 hours / 2 business days]

## Modifications
- We may update these terms with notice
- Continued use constitutes acceptance of updated terms
```

Put this at `yourdomain.com/terms`. Link to it from your checkout page footer.

### Before Your 100th Sale (First Few Months)

**4. Business Entity (1-3 hours + processing time)**

Operating as a sole proprietor (the default when you sell things without forming a business) works for your first sales. But as revenue grows, you want liability protection and tax advantages.

**United States — LLC:**

An LLC (Limited Liability Company) is the standard choice for solo developer businesses.

```
Cost: $50-500 depending on state (filing fee)
Time: 1-4 weeks for processing
Where to file: Your home state, unless there's a specific reason
to use Delaware or Wyoming

DIY filing (cheapest):
1. Go to your state's Secretary of State website
2. File "Articles of Organization" (the form is usually 1-2 pages)
3. Pay the filing fee ($50-250 depending on state)
4. Get your EIN (tax ID) from IRS.gov — free, instant online

State comparison for solo developers:
- Wyoming: $100 filing, $60/year annual report. No state income tax.
             Good for privacy (no public member info required).
- Delaware: $90 filing, $300/year annual tax. Popular but not
            necessarily better for solo developers.
- New Mexico: $50 filing, no annual report. Cheapest to maintain.
- California: $70 filing, $800/year minimum franchise tax.
              Expensive. You pay this even if you make $0.
```

**Stripe Atlas (if you want it done for you):**

Stripe Atlas (https://atlas.stripe.com) costs $500 and sets up a Delaware LLC, US bank account (via Mercury), Stripe account, and provides tax and legal guides. If you are non-US or just want someone else to handle the paperwork, it is worth the $500.

**United Kingdom — Ltd Company:**

```
Cost: GBP 12 at Companies House (https://www.gov.uk/set-up-limited-company)
Time: Usually 24-48 hours
Ongoing: Annual confirmation statement (GBP 13), annual accounts filing

For solo developers: A Ltd company gives you liability protection
and tax efficiency once profits exceed ~GBP 50,000/year.
Below that, sole trader is simpler.
```

**European Union:**

Each country has its own structure. Common options:
- **Germany:** GmbH (expensive to set up) or freelancer registration (cheap)
- **Netherlands:** BV or eenmanszaak (sole proprietorship)
- **France:** auto-entrepreneur (micro-enterprise) — very common for solo developers, simple flat tax
- **Estonia:** e-Residency + Estonian OUe (popular with digital nomads, full EU company for ~EUR 190)

**Australia:**

```
Sole trader: Free to register via ABN application (https://www.abr.gov.au)
Company (Pty Ltd): AUD 538 registration with ASIC
For solo developers: Start as sole trader. Register a company
when revenue justifies the accounting overhead (~AUD 100K+/year).
```

**5. Tax Obligations**

If you are using Lemon Squeezy as your payment platform, they handle sales tax and VAT as the Merchant of Record. This is a massive simplification.

If you are using Stripe directly, you are responsible for:
- **US sales tax:** Varies by state. Use Stripe Tax ($0.50/transaction) or TaxJar to automate.
- **EU VAT:** 20-27% depending on country. Required for digital sales to EU customers regardless of where you are based. Lemon Squeezy handles this; Stripe Tax can automate it.
- **UK VAT:** 20%. Required if your UK sales exceed GBP 85,000/year.
- **Digital Services Taxes:** Various countries imposing these. Another reason to use Lemon Squeezy until your volume justifies managing this yourself.

> **Real Talk:** The single biggest advantage of Lemon Squeezy over Stripe for a solo developer is not the checkout page or the features. It is that they handle tax compliance globally. International sales tax is a nightmare. Lemon Squeezy takes 5% + $0.50 per transaction and makes the nightmare go away. Until you are making $5,000+/month, the 5% is worth it. After that, evaluate whether managing taxes yourself with Stripe + TaxJar saves you money and sanity.

**6. Intellectual Property Basics**

What you need to know:

- **Your code is automatically copyrighted** the moment you write it. No registration needed. But registration (US: $65 at copyright.gov) gives you stronger legal standing in disputes.
- **Your product name can be trademarked.** Not required for launch, but consider it if the product takes off. US trademark filing: $250-350 per class.
- **Open-source licenses in your dependencies matter.** If you use MIT-licensed code, you are fine. If you use GPL-licensed code in a commercial product, you may need to open-source your product. Check your dependency licenses before selling.

```bash
# Check your project's dependency licenses (Node.js)
npx license-checker --summary

# Check for problematic licenses specifically
npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-3.0"

# For Rust projects
cargo install cargo-license
cargo license
```

**7. Insurance**

You do not need insurance for a $29 component library. You do need insurance if:
- You are providing services (consulting, data processing) where errors could cause client losses
- Your product handles sensitive data (healthcare, financial)
- You are signing contracts with enterprise customers (they will require it)

When you need it, professional liability insurance (errors and omissions / E&O) costs $500-1,500/year for a solo developer business.

### Your Turn

1. **Read your employment contract.** If you are employed, find the IP clause and non-compete clause. Categorize them: Safe / Murky / Restrictive. If Murky or Restrictive, consult an employment lawyer before launching (many offer free 30-minute consultations).

2. **Generate your legal documents.** Go to Termly or Avodocs and generate a privacy policy and terms of service for your product. Save them as HTML or Markdown. Deploy them to `/privacy` and `/terms` on your product domain.

3. **Make your entity decision.** Based on the guidance above and your country of residence, decide: launch as sole proprietor (fastest) or form an LLC/Ltd/equivalent first (more protection). Write down your decision and timeline.

4. **Check your dependencies.** Run the license checker on your project. Resolve any GPL/AGPL dependencies before selling a commercial product.

---

## Lesson 5: Distribution Channels That Work in 2026

*"Building it is 20% of the work. Getting it in front of people is the other 80%."*

### The Distribution Reality

Most developer products fail not because they are bad, but because nobody knows they exist. Distribution — getting your product in front of potential customers — is the skill most developers are weakest at. And it is the skill that matters most.

Here are seven distribution channels ranked by effort, timeline, and expected return. You do not need all seven. Pick 2-3 that match your strengths and your audience.

### Channel 1: Hacker News

**Effort:** High | **Timeline:** Instant (0-48 hours) | **Nature:** Feast-or-famine

Hacker News (https://news.ycombinator.com) is the highest-leverage single-event distribution channel for developer products. A front-page Show HN post can send 5,000-30,000 visitors in 24 hours. But it is unpredictable — most posts get zero traction.

**What works on HN:**
- Technical products with interesting implementation details
- Privacy-focused tools (HN audience cares deeply about privacy)
- Open-source tools with a paid tier
- Novel solutions to known problems
- Products with live demos

**What does not work on HN:**
- Marketing-heavy launches ("Revolutionary AI-powered...")
- Products that are wrappers around other products with no original value
- Anything that feels like an ad

**The Show HN Playbook:**

```
BEFORE POSTING:
1. Study recent successful Show HN posts in your category
   https://hn.algolia.com — filter by "Show HN", sort by points
2. Prepare your post title: "Show HN: [Name] – [what it does, <70 chars]"
   Good: "Show HN: ScrubLog – Strip PII from Log Files in One Command"
   Bad: "Show HN: Introducing ScrubLog, the AI-Powered Log Anonymization Platform"
3. Have a live demo ready (HN readers want to try, not read about it)
4. Prepare answers to likely questions (technical decisions, pricing rationale)

POSTING:
5. Post between 7-9 AM US Eastern Time, Tuesday through Thursday
   (highest traffic, highest chance of traction)
6. Your post body should be 4-6 paragraphs:
   - What it is (1 paragraph)
   - Why you built it (1 paragraph)
   - Technical details (1-2 paragraphs)
   - What you're looking for (feedback, specific questions)

AFTER POSTING:
7. Stay online for 4 hours after posting. Respond to EVERY comment.
8. Be humble and technical. HN rewards honesty about limitations.
9. If someone finds a bug, fix it live and reply "Fixed, thanks."
10. Do not ask friends to upvote. HN has vote-ring detection.
```

**Expected results (realistic):**
- 70% of Show HN posts: <10 points, <500 visitors
- 20% of Show HN posts: 10-50 points, 500-3,000 visitors
- 10% of Show HN posts: 50+ points, 3,000-30,000 visitors

It is a lottery with effort-loaded odds. A great product with a great post has maybe a 30% chance of meaningful traction. Not guaranteed. But the upside is enormous.

### Channel 2: Reddit

**Effort:** Medium | **Timeline:** 1-7 days | **Nature:** Sustainable, repeatable

Reddit is the most consistent distribution channel for developer products. Unlike HN (one shot), Reddit has hundreds of niche subreddits where your product is relevant.

**Subreddit selection:**

```
General developer subreddits:
- r/SideProject (140K+ members) — built for this
- r/webdev (2.4M members) — huge, competitive
- r/programming (6.3M members) — very competitive, news-focused
- r/selfhosted (400K+ members) — if your product is self-hostable

Framework/language-specific:
- r/reactjs, r/nextjs, r/sveltejs, r/vuejs — for frontend tools
- r/rust, r/golang, r/python — for language-specific tools
- r/node — for Node.js tools and packages

Domain-specific:
- r/devops — for infrastructure/deployment tools
- r/machinelearning — for AI/ML tools
- r/datascience — for data tools
- r/sysadmin — for admin/monitoring tools

The long tail:
- Search for subreddits related to your specific niche
- Smaller subreddits (10K-50K members) often have better
  conversion rates than huge ones
```

**Reddit engagement rules:**

1. **Have a real Reddit history** before posting your product. Accounts that only post self-promotion get flagged and shadowbanned.
2. **Follow each subreddit's rules** about self-promotion. Most allow it as long as you are a contributing member.
3. **Engage genuinely.** Answer questions, provide value, be helpful in comments on other posts. Then share your product.
4. **Post at different times** for different subreddits. Check https://later.com/reddit or similar tools for peak activity times.

**Expected results (realistic):**
- r/SideProject post: 20-100 upvotes, 200-2,000 visitors
- Niche subreddit (50K members): 10-50 upvotes, 100-1,000 visitors
- Front page of r/webdev: 100-500 upvotes, 2,000-10,000 visitors

### Channel 3: Twitter/X

**Effort:** Medium | **Timeline:** 2-4 weeks to build momentum | **Nature:** Compounds over time

Twitter is a slow-build channel. Your first launch tweet will get 5 likes from your friends. But if you consistently share your build process, your audience compounds.

**The Build-in-Public Strategy:**

```
Week 1: Start sharing your build process (before launch)
- "Working on a [product type]. Here's the problem I'm solving: [screenshot]"
- "Day 3 of building [product]. Got [feature] working: [GIF/screenshot]"

Week 2: Share technical insights from the build
- "TIL you need to [technical lesson] when building [product type]"
- "Architecture decision: chose [X] over [Y] because [reason]"

Week 3: Launch
- Launch thread (format from Lesson 1)
- Share specific metrics: "Day 1: X visitors, Y signups"

Week 4+: Ongoing
- Share customer feedback (with permission)
- Share revenue milestones (people love real numbers)
- Share challenges and how you solved them
```

**Who to engage with:**
- Follow and interact with developers in your niche
- Reply to tweets from larger accounts with thoughtful comments (not self-promotion)
- Join Twitter Spaces about your topic area
- Quote-tweet relevant discussions with your perspective

**Expected results (realistic):**
- 0-500 followers: Launch tweets get 5-20 likes, <100 visitors
- 500-2,000 followers: Launch tweets get 20-100 likes, 100-500 visitors
- 2,000-10,000 followers: Launch tweets get 100-500 likes, 500-5,000 visitors

Twitter is a 6-month investment, not a launch-day strategy. Start now, even before your product is ready.

### Channel 4: Product Hunt

**Effort:** High | **Timeline:** 1 day of intense activity | **Nature:** One-time boost

Product Hunt (https://producthunt.com) is a dedicated launch platform. A top-5 daily finish can send 3,000-15,000 visitors. But it requires preparation.

**Product Hunt Launch Checklist:**

```
2 WEEKS BEFORE:
- [ ] Create a Product Hunt maker profile
- [ ] Build your PH listing: tagline, description, images, video
- [ ] Prepare 4-5 high-quality screenshots/GIFs
- [ ] Write a "first comment" that explains your motivation
- [ ] Line up 10-20 people to support on launch day (not fake votes —
      real people who will try the product and leave genuine comments)
- [ ] Find a "hunter" (someone with a large PH following to submit your product)
      or submit yourself

LAUNCH DAY (12:01 AM Pacific Time):
- [ ] Be online from midnight PT. PH resets at midnight.
- [ ] Post your "first comment" immediately
- [ ] Share the PH link on Twitter, LinkedIn, email, Discord
- [ ] Respond to EVERY comment on your PH listing
- [ ] Post updates throughout the day ("Just shipped a fix for [X]!")
- [ ] Monitor all day until midnight PT

AFTER:
- [ ] Thank everyone who supported
- [ ] Write a "lessons learned" post (good for Twitter/blog content)
- [ ] Embed the PH badge on your landing page (social proof)
```

> **Common Mistake:** Launching on Product Hunt before your product is ready. PH gives you one shot. Once you launch a product, you cannot re-launch it. Wait until your product is polished, your landing page converts, and your payment flow works. PH should be your "big launch" — not your soft launch.

**Expected results (realistic):**
- Top 5 daily: 3,000-15,000 visitors, 50-200 upvotes
- Top 10 daily: 1,000-5,000 visitors, 20-50 upvotes
- Below top 10: <1,000 visitors. Minimal lasting impact.

### Channel 5: Dev.to / Hashnode / Technical Blog Posts

**Effort:** Low-medium | **Timeline:** SEO results in 1-3 months | **Nature:** Long-tail, compounds forever

Write technical blog posts that solve problems related to your product, and mention your product as the solution.

**Content strategy:**

```
For each product, write 3-5 blog posts:

1. "How to [solve the problem your product solves] in 2026"
   - Teach the manual approach, then mention your product as the shortcut

2. "I built [product] in 48 hours — here's what I learned"
   - Build-in-public content. Technical details + honest reflection.

3. "[Competitor] vs [Your Product]: Honest Comparison"
   - Be genuinely fair. Mention where the competitor wins.
   - This captures comparison-shopping search traffic.

4. "[Technical concept related to your product] explained"
   - Pure education. Mention your product once at the end.

5. "The tools I use for [your product's domain] in 2026"
   - Listicle format. Include your product alongside others.
```

**Where to publish:**
- **Dev.to** (https://dev.to) — Large developer audience, good SEO, free
- **Hashnode** (https://hashnode.com) — Good SEO, custom domain option, free
- **Your own blog** — Best for long-term SEO, you own the content
- **Cross-post everywhere.** Write once, publish on all three. Use canonical URLs to avoid SEO penalties.

**Expected results per post:**
- Day 1: 100-1,000 views (platform distribution)
- Month 1-3: 50-200 views/month (search traffic building)
- Month 6+: 100-500 views/month (compounding search traffic)

A single well-written blog post can drive 200+ visitors per month for years. Five posts drive 1,000+/month. This compounds.

### Channel 6: Direct Outreach

**Effort:** High | **Timeline:** Immediate | **Nature:** Highest conversion rate

Cold email and DMs have the highest conversion rate of any channel — but also the highest effort per lead. Use this for higher-priced products ($99+) or B2B sales.

**Email template for reaching potential customers:**

```
Subject: Quick question about [their specific pain point]

Hi [name],

I saw your [tweet/post/comment] about [specific problem they mentioned].

I built [product name] specifically for this — it [one-sentence
description of what it does].

Would you be open to trying it? Happy to give you free access
for feedback.

[Your name]
[Link to product]
```

**Rules for cold outreach:**
- Only reach out to people who have publicly expressed the problem your product solves
- Reference their specific post/comment (proves you are not mass-emailing)
- Offer value (free access, discount) rather than asking for money immediately
- Keep it under 5 sentences
- Send from a real email address (you@yourdomain.com, not gmail)
- Follow up once after 3-4 days. If no response, stop.

**Expected results:**
- Response rate: 10-20% (cold email to relevant recipients)
- Conversion from response to trial: 30-50%
- Conversion from trial to paid: 20-40%
- Effective conversion: 1-4% of people emailed become customers

For a $99 product, emailing 100 people = 1-4 sales = $99-396. Not scalable, but excellent for getting early customers and feedback.

### Channel 7: SEO

**Effort:** Low ongoing | **Timeline:** 3-6 months for results | **Nature:** Compounds forever

SEO is the best long-term distribution channel. It is slow to start but once it works, it sends free traffic indefinitely.

**Developer-focused SEO strategy:**

```
1. Target long-tail keywords (easier to rank for):
   Instead of: "dashboard components"
   Target: "tailwind dashboard components react typescript"

2. Create one page per keyword:
   Each blog post or docs page targets one specific search query

3. Technical implementation:
   - Use static site generation (Astro, Next.js SSG) for fast loads
   - Add meta descriptions to every page
   - Use semantic HTML (h1, h2, h3 hierarchy)
   - Add alt text to every image
   - Submit sitemap to Google Search Console

4. Content that ranks for developer tools:
   - Documentation pages (surprisingly good for SEO)
   - Comparison pages ("X vs Y")
   - Tutorial pages ("How to do X with Y")
   - Changelog pages (fresh content signals to Google)
```

```bash
# Submit your sitemap to Google Search Console
# 1. Go to https://search.google.com/search-console
# 2. Add your property (domain or URL prefix)
# 3. Verify ownership (DNS TXT record or HTML file)
# 4. Submit your sitemap URL: yourdomain.com/sitemap.xml

# If using Astro:
pnpm add @astrojs/sitemap
# Sitemap is auto-generated at /sitemap.xml

# If using Next.js, add to next-sitemap.config.js:
# pnpm add next-sitemap
```

**Expected results:**
- Month 1-3: Minimal organic traffic (<100/month)
- Month 3-6: Growing traffic (100-500/month)
- Month 6-12: Significant traffic (500-5,000/month)
- Month 12+: Compounding traffic that grows without effort

### Channel Selection Framework

You cannot do all seven well. Pick 2-3 based on this matrix:

| If you are... | Prioritize | Skip |
|---|---|---|
| Launching this weekend | Reddit + HN | SEO, Twitter (too slow) |
| Building an audience first | Twitter + Blog posts | Direct outreach, PH |
| Selling a $99+ product | Direct outreach + HN | Dev.to (audience expects free) |
| Playing the long game | SEO + Blog posts + Twitter | PH (one-shot, use later) |
| Non-English speaking | Dev.to + Reddit (global) | HN (US-centric) |

### Your Turn

1. **Pick your 2-3 channels.** Based on the matrix above and your product type, choose the channels you will focus on. Write them down with your planned timeline for each.

2. **Write your Reddit post.** Using the template from Lesson 1, write your r/SideProject post draft right now. Save it. You will post it on launch day.

3. **Write your first blog post.** Draft a "How to [solve problem your product solves]" post. This goes on Dev.to or your blog within the first week of launching. Aim for 1,500-2,000 words.

4. **Set up Google Search Console.** This takes 5 minutes and gives you SEO data from day one. Do it before you launch so you have baseline data.

---

## Lesson 6: Your Launch Checklist

*"Hope is not a launch strategy. Checklists are."*

### The Pre-Launch Checklist

Go through every item. Do not launch until every "Required" item is checked. "Recommended" items can be done in Week 1 if needed.

**Product (Required):**

```
- [ ] Core feature works as described on the landing page
- [ ] No critical bugs in the purchase → delivery flow
- [ ] Works in Chrome, Firefox, and Safari (for web products)
- [ ] Mobile-responsive landing page (50%+ traffic is mobile)
- [ ] Error messages are helpful, not stack traces
- [ ] Loading states for any async operations
```

**Landing Page (Required):**

```
- [ ] Clear headline: what it does in 8 words or less
- [ ] Problem statement: 3 pain points in customer language
- [ ] Solution section: screenshots or demos of the product
- [ ] Pricing: visible, clear, with buy button
- [ ] Call to action: one primary button, visible above the fold
- [ ] Privacy policy linked in footer
- [ ] Terms of service linked in footer
```

**Payments (Required):**

```
- [ ] Checkout flow tested end-to-end in test mode
- [ ] Checkout flow tested end-to-end in live mode ($1 test purchase)
- [ ] Webhook receives payment confirmation
- [ ] Customer receives product access after payment
- [ ] Refund process documented (you WILL get refund requests)
- [ ] Receipt/invoice sent automatically
```

**Infrastructure (Required):**

```
- [ ] Custom domain pointing to live site
- [ ] HTTPS working (green padlock)
- [ ] Uptime monitoring active
- [ ] Analytics script installed and receiving data
- [ ] Contact email working (you@yourdomain.com)
```

**Distribution (Required):**

```
- [ ] Reddit post drafted and ready
- [ ] Show HN post drafted and ready (if applicable)
- [ ] Twitter launch thread drafted
- [ ] 2-3 communities identified for sharing
```

**Recommended (Week 1):**

```
- [ ] OpenGraph meta tags for social sharing previews
- [ ] Custom 404 page
- [ ] FAQ page or section
- [ ] Customer onboarding email sequence (welcome + getting started)
- [ ] Changelog page (even if empty — shows commitment to updates)
- [ ] Blog post: "I built [product] in 48 hours"
- [ ] Google Search Console verified and sitemap submitted
```

### Post-Launch Action Items

**Day 1 (Launch Day):**

```
Morning:
- [ ] Post on Reddit (r/SideProject + 1 niche subreddit)
- [ ] Post Show HN (if applicable)
- [ ] Post Twitter launch thread

All day:
- [ ] Respond to EVERY comment on Reddit, HN, and Twitter
- [ ] Monitor error logs and analytics in real-time
- [ ] Fix any bugs discovered by users immediately
- [ ] Send personal thank-you email to every customer

Evening:
- [ ] Check metrics: visitors, conversion rate, revenue
- [ ] Screenshot your analytics dashboard (you will want this later)
- [ ] Write down the 3 most common pieces of feedback
```

**Week 1:**

```
- [ ] Respond to all feedback and support requests within 24 hours
- [ ] Fix the top 3 bugs/issues identified during launch
- [ ] Write and publish your first blog post
- [ ] Send a follow-up email to all customers asking for feedback
- [ ] Review analytics: which pages have highest bounce rates?
- [ ] Set up a simple feedback collection method (email, Typeform, or Canny)

Weekly metrics to record:
| Metric              | Target    | Actual |
|---------------------|-----------|--------|
| Unique visitors     | 500+      |        |
| Checkout click rate | 2-5%      |        |
| Purchase conversion | 1-3%      |        |
| Revenue             | $50+      |        |
| Support requests    | <10       |        |
| Refund requests     | <2        |        |
```

**Month 1:**

```
- [ ] Ship 4 weekly improvements based on customer feedback
- [ ] Publish 2+ blog posts (SEO building)
- [ ] Collect 3+ testimonials from customers
- [ ] Add testimonials to landing page
- [ ] Evaluate pricing: too high? too low? (review conversion data)
- [ ] Plan your "big launch" on Product Hunt (if applicable)
- [ ] Start building email list for future product launches
- [ ] Review and adjust your distribution channel strategy

Monthly financial review:
| Category           | Amount    |
|--------------------|-----------|
| Gross revenue      | $         |
| Payment processor fees | $     |
| Hosting/infra costs | $        |
| API costs          | $         |
| Net profit         | $         |
| Hours invested     |           |
| Effective hourly rate | $      |
```

### The Metrics Dashboard

Set up a simple metrics dashboard you check daily. This does not need to be fancy — a spreadsheet works.

```
=== DAILY METRICS (check every morning) ===

Date: ___
Visitors yesterday: ___
New customers yesterday: ___
Revenue yesterday: $___
Support requests: ___
Uptime: ___%

=== WEEKLY METRICS (check every Monday) ===

Week of: ___
Total visitors: ___
Total customers: ___
Total revenue: $___
Conversion rate: ___% (customers / visitors)
Most visited page: ___
Top traffic source: ___
Top feedback theme: ___

=== MONTHLY METRICS (check on 1st of month) ===

Month: ___
Total revenue: $___
Total expenses: $___
Net profit: $___
Total customers: ___
Refunds: ___
Churn rate (subscriptions): ___%
MRR (Monthly Recurring Revenue): $___
Growth rate vs. last month: ___%
```

**Privacy-respecting analytics setup:**

```javascript
// If using Plausible, you get most of this in their dashboard.
// For custom event tracking:

// Track checkout clicks
document.querySelector('#buy-button').addEventListener('click', () => {
  plausible('Checkout Click', {
    props: { tier: 'pro', price: '59' }
  });
});

// Track successful purchases (call from your webhook success handler)
plausible('Purchase', {
  props: { tier: 'pro', revenue: '59' }
});
```

### When to Double Down, Pivot, or Kill

After 30 days of data, you have enough signal to make a decision:

**Double Down (keep going, invest more):**

```
Signals:
- Revenue is growing week over week (even if slowly)
- Customers are providing specific feature requests (they want MORE)
- Conversion rate is steady or improving
- You are getting organic traffic (people finding you without your posts)
- At least one customer said "this saved me [time/money]"

Actions:
- Increase distribution efforts (add a channel)
- Ship the top-requested feature
- Raise prices slightly
- Start building an email list for future launches
```

**Pivot (change the angle, keep the core):**

```
Signals:
- Visitors but no sales (people are interested but not buying)
- Sales from unexpected audience (different people than you targeted)
- Customers use the product differently than you expected
- Feedback consistently points to a different problem than you're solving

Actions:
- Rewrite landing page for the actual audience/use case
- Adjust pricing based on the real audience's willingness to pay
- Reprioritize features toward what people actually use
- Keep the code, change the positioning
```

**Kill (stop, learn, build something else):**

```
Signals:
- No visitors despite distribution efforts (demand problem)
- Visitors but zero checkout clicks (positioning/pricing problem
  that persists after adjustments)
- Revenue stagnant for 4+ weeks with no growth trend
- You dread working on it (motivation matters for solo products)
- The market has shifted (competitor launched, tech changed)

Actions:
- Write a post-mortem: what worked, what didn't, what you learned
- Save the code — pieces might be useful in your next product
- Take a week off from building
- Start the validation process for a new idea
- This is not failure. This is data. Most products don't work.
  The developers who make money are the ones who ship 5 products,
  not the ones who spend a year on one.
```

### The Launch Document Template

This is your deliverable for Module E. Create this document and fill it in as you execute your launch.

```markdown
# Launch Document: [Product Name]

## Pre-Launch

### Validation Summary
- **Search volume:** [numbers from Google Trends/Ahrefs]
- **Thread evidence:** [links to 5+ threads showing demand]
- **Competitor audit:** [3+ competitors with strengths/weaknesses]
- **"10 people would pay" evidence:** [how you validated this]

### Product
- **URL:** [live product URL]
- **Domain:** [purchased domain]
- **Hosting:** [platform]
- **Core features (v0.1):**
  1. [Feature 1]
  2. [Feature 2]
  3. [Feature 3]

### Pricing
- **Price:** $[amount]
- **Tier structure:** [Basic/Pro/Team or single tier]
- **Payment platform:** [Lemon Squeezy/Stripe]
- **Checkout URL:** [link]

### Legal
- **Privacy policy:** [URL]
- **Terms of service:** [URL]
- **Business entity:** [type or "sole proprietor"]

## Launch

### Distribution Channels
| Channel | Post URL | Date Posted | Results |
|---------|----------|-------------|---------|
| Reddit  | [link]   | [date]      | [visitors, upvotes] |
| HN      | [link]   | [date]      | [visitors, points] |
| Twitter | [link]   | [date]      | [impressions, clicks] |

### Day 1 Metrics
- Visitors: ___
- Checkout clicks: ___
- Purchases: ___
- Revenue: $___

### Week 1 Metrics
- Total visitors: ___
- Total purchases: ___
- Total revenue: $___
- Conversion rate: ___%
- Top feedback: ___

### Month 1 Metrics
- Total revenue: $___
- Total expenses: $___
- Net profit: $___
- Total customers: ___
- Decision: [ ] Double down [ ] Pivot [ ] Kill

## Post-Launch Roadmap
- Week 2: [planned improvement]
- Week 3: [planned improvement]
- Week 4: [planned improvement]
- Month 2: [planned feature/expansion]

## Lessons Learned
- What worked: ___
- What didn't work: ___
- What I'd do differently: ___
```

### 4DA Integration

> **4DA Integration:** 4DA's actionable signals classify content by urgency. A "critical" signal about a vulnerability in a popular package means: build the fix or migration tool NOW, before anyone else. A "rising trend" signal about a new framework means: build the starter kit this weekend while competition is near zero. The 48-hour sprint from Lesson 1 works best when your idea comes from a time-sensitive signal. Connect your 4DA intelligence feed to your sprint calendar — when a high-urgency opportunity appears, block the next weekend and execute. The difference between developers who capture opportunities and those who miss them is not talent. It is speed. 4DA gives you the radar. This module gives you the launch sequence. Together, they turn signals into revenue.

### Your Turn

1. **Complete the pre-launch checklist.** Go through every item. Mark each one done or schedule when you will do it. Do not skip the "Required" items.

2. **Create your Launch Document.** Copy the template above into your preferred document tool. Fill in everything you know now. Leave blanks for metrics you will fill in during and after launch.

3. **Set your launch date.** Open your calendar. Pick a specific Saturday within the next 2 weeks. Write it down. Tell someone — a friend, a partner, a Twitter follower. Accountability makes it real.

4. **Set your kill criteria.** Before you launch, decide: "If I have fewer than [X] sales after 30 days despite [Y] distribution effort, I will [pivot/kill]." Write this in your Launch Document. Having pre-committed criteria prevents you from pouring months into a dead product because of sunk cost fallacy.

5. **Ship it.** You have the playbook. You have the tools. You have the knowledge. The only thing left is the act. The internet is waiting.

---

## Module E: Complete

### What You Have Built in Two Weeks

Look at what you now have that you did not have when you started this module:

1. **A 48-hour execution framework** you can repeat for every product you build — validated idea to live product in one weekend.
2. **A shipping mindset** that prioritizes existence over perfection, data over guessing, and iteration over planning.
3. **A pricing strategy** grounded in real psychology and real numbers, not hope and undercharging.
4. **A legal foundation** that protects you without paralyzing you — privacy policy, terms, entity plan.
5. **A distribution playbook** with specific templates, timing, and expected results for seven channels.
6. **A launch checklist and tracking system** that turns chaos into process — repeatable, measurable, improvable.
7. **A live product, accepting payments, with real humans visiting it.**

That last one is the one that matters. Everything else is preparation. The product is the proof.

### What Comes Next: Module E2 — Evolving Edge

Module E1 got you to launch. Module E2 keeps you ahead.

Here is what Module E2 covers:

- **Trend detection systems** — how to spot opportunities 2-4 weeks before they become obvious
- **Competitive monitoring** — tracking what others in your space are building and pricing
- **Technology wave riding** — when to adopt new tech in your products and when to wait
- **Customer development** — turning your first 10 customers into your product advisory board
- **The second product decision** — when to build product #2 vs. improving product #1

The developers who make consistent income are not the ones who launch once. They are the ones who launch, iterate, and stay ahead of the market. Module E2 gives you the system for staying ahead.

### The Full STREETS Roadmap

| Module | Title | Focus | Duration |
|--------|-------|-------|----------|
| **S** | Sovereign Setup | Infrastructure, legal, budget | Weeks 1-2 |
| **T** | Technical Moats | Defensible advantages, proprietary assets | Weeks 3-4 |
| **R** | Revenue Engines | Specific monetization playbooks with code | Weeks 5-8 |
| **E** | Execution Playbook | Launch sequences, pricing, first customers | Weeks 9-10 (complete) |
| **E** | Evolving Edge | Staying ahead, trend detection, adaptation | Weeks 11-12 |
| **T** | Tactical Automation | Automating operations for passive income | Weeks 13-14 |
| **S** | Stacking Streams | Multiple income sources, portfolio strategy | Weeks 15-16 |

You are past the halfway point. You have a live product. That puts you ahead of 95% of developers who want to build independent income but never get this far.

Now make it grow.

---

**Your product is live. Your checkout works. Humans can pay you money.**

**Everything after this is optimization. And optimization is the fun part.**

*Your rig. Your rules. Your revenue.*
