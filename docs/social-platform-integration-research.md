# Social Platform Integration — Feasibility Research

Research for roadmap items **#12 "Post Builder and Publish — YouTube"** and
**#13 "LinkedIn, X, Telegram, Viber, YouTube integration"** (detailed post/content
management, messaging, etc.).

> **TL;DR** — Some of this is genuinely easy and high‑value (Telegram, Discord),
> some is doable with real effort (YouTube video upload, X posting), and some is
> effectively a non‑starter for an open‑source **desktop** tool (LinkedIn at any
> scale, Viber, YouTube *community* posts). The single biggest constraint is not
> any one API — it's that Palworld Server Manager is a **distributed desktop app
> with no always‑on backend and no way to safely ship a client secret**. The
> recommended path is a **"bring‑your‑own‑credentials" model**, an event‑driven
> **notifications** layer hosted in the always‑on **Bridge**, and a manual
> **Post Builder** in the desktop UI — starting with the platforms that don't
> require OAuth or app review.

*Compiled 2026‑07 against then‑current platform terms. API pricing and policies
on these platforms change frequently — re‑verify before committing engineering
time. Sources are listed at the end.*

---

## 1. What is actually being asked for

Reading items #12 and #13 together, there are **three distinct capabilities**
bundled under "integration", and they have very different feasibility profiles.
It's important not to treat them as one feature:

| Capability | What it means for a Palworld server admin | Best‑fit platforms |
| --- | --- | --- |
| **A. Broadcast / content publishing** | Compose a post once (server milestone, event announcement, world‑map screenshot, patch note) and publish/cross‑post it. This is roadmap item #12's "Post Builder". | X, YouTube (video), LinkedIn, Telegram channels |
| **B. Event‑driven notifications** | Automatically push server events ("server back online", "player joined", "crash detected", "scheduled restart in 5 min") to a place the community watches. | Telegram, Discord, Viber |
| **C. Two‑way messaging / chat relay** | Relay in‑game chat ↔ a chat app, or accept admin commands from chat. | Telegram, Discord |

For a **gaming community** tool, **B (notifications)** and **C (chat relay)** are
by far the highest value, and Telegram/Discord serve them best. **A (broadcast)**
to X/YouTube/LinkedIn is real but niche — a server owner promoting a public/paid
community. **LinkedIn in particular has almost no audience fit** for a Palworld
server and is also the hardest to integrate; it's included below for completeness.

---

## 2. The architectural constraint that governs everything

Palworld Server Manager is a **Tauri (Rust + React) desktop application** that is
**installed on each user's machine**, is **only running when the user opens it**,
and deliberately **keeps all credentials local** (see README → Security). Three
consequences fall out of that, and they decide what's possible before any single
API does:

### 2.1 You cannot safely ship a client secret
OAuth "confidential client" flows (LinkedIn, and the classic Google/YouTube web
flow) require a **client secret**. Anything baked into a downloadable, open‑source
binary is **public** — it can be extracted, abused, and will get the app's API
project **suspended**, taking the feature down for *every* user at once. This
rules out any platform that *requires* a confidential client, unless we run a
hosted backend (see 2.4).

The clean answers are:
- **Bot‑token model** (Telegram): the user creates their *own* bot and pastes a
  token. No secret of ours ships. **Trivially compatible with a desktop app.**
- **Public‑client OAuth with PKCE** (X, Google/YouTube): no secret needed; a
  loopback `http://127.0.0.1:<port>` redirect completes the flow natively. Tauri
  can host this. Tokens are stored locally. **Compatible, with effort.**
- **Confidential client** (LinkedIn): needs a secret **and** a server. **Not
  compatible** without a hosted backend.

### 2.2 Scheduling and event‑driven posting need an always‑on process
The desktop UI is closed most of the time, so "post automatically when the server
restarts" or "publish at 6pm" **cannot live in the desktop app**. But this project
*already ships an always‑on component*: the **Bridge (`psm-bridge.exe`)**, which
runs persistently on the server host and already watches the server process and
save files. **The Bridge is the natural home for event‑driven and scheduled
posting.** The desktop app is the right home for *manual*, interactive composing
("Post Builder"). This split is the key design insight of this whole feature area.

### 2.3 "One shared app" vs "bring your own credentials"
If we register a single X/Google/LinkedIn app and ship its credentials, **all
users share one rate‑limit/quota pool and one point of failure/suspension**. For
an open‑source tool the sustainable model is **BYO credentials**: each admin
registers their own app/bot and pastes their own tokens. Trade‑off:

- **BYO is easy for Telegram** (make a bot in 30 seconds via `@BotFather`).
- **BYO is painful but possible for X and YouTube** (create a developer account,
  a project, enable APIs, paste keys — a multi‑step wizard we'd document).
- **BYO is essentially impossible for LinkedIn** (see §3.3 — requires a
  registered legal entity and app review per app).

### 2.4 The "do we need a backend?" question
A small **optional hosted relay** would unlock the hard platforms (LinkedIn, a
shared‑credential X app, scheduled posting without the Bridge) and centralise
OAuth secrets. But it directly contradicts the project's "local & private, nothing
phoned home" promise, adds hosting cost/maintenance, and becomes a data‑handling
liability. **Recommendation: avoid a mandatory backend.** Use the Bridge for
always‑on needs and BYO credentials for auth. Revisit a backend only if
broadcast‑to‑LinkedIn/X becomes a genuinely requested priority.

---

## 3. Per‑platform feasibility

Legend: 🟢 good fit / feasible · 🟡 feasible with real effort or limits · 🔴 not
practical for this app.

### 3.1 Telegram — 🟢 **Start here**
- **What's possible:** Full Bot API, free, HTTP/JSON. Send text, photos
  (world‑map screenshots), documents, albums; post to **channels** and **groups**;
  rich HTML/Markdown formatting; buttons; and **two‑way** (webhook or long‑poll to
  receive commands). Covers notifications **(B)**, broadcast **(A)** to a channel,
  and chat relay **(C)**.
- **Auth:** User creates a bot with `@BotFather`, pastes the token. **No OAuth,
  no secret of ours, no review.** Perfectly BYO.
- **Cost / limits:** Free. ~30 msg/sec overall, ~20 msg/min to one group — far
  above anything a server manager needs.
- **Desktop fit:** Excellent. The desktop app can send directly; the Bridge can
  own event‑driven/scheduled sends.
- **Verdict:** Highest value‑to‑effort ratio of everything here. Ship first.

### 3.2 X / Twitter — 🟡 **Feasible, watch the cost & churn**
- **What's possible:** Post tweets (text + media) via API v2. Good fit for
  broadcast **(A)**: milestones, event announcements, screenshots. Not a
  notifications channel.
- **Auth:** OAuth 2.0 **user‑context with PKCE** — a **public client, no secret**,
  works natively in Tauri via a loopback redirect. BYO app is doable but the
  onboarding is heavy (developer account + app + tokens).
- **Cost / limits (the catch):** As of **6 Feb 2026**, X made **pay‑per‑use** the
  default for new developers — there is **no free tier** and new signups **cannot**
  get the old Basic/Pro plans. Posting is **~$0.015 per post** (**~$0.20 if the
  post contains a link** — relevant, since server posts often link to a Discord/
  invite). The legacy **Free** tier historically allowed ~1,500 posts/month at
  $0, but is **closed to new developers**; legacy Basic ($200/mo) is being
  migrated onto pay‑per‑use after 1 Jun 2026. **This is volatile — re‑check at
  build time.**
- **Desktop fit:** OK. PKCE flow + local token storage; posting from either the
  app or Bridge.
- **Verdict:** Technically feasible and reasonable for *occasional* broadcast, but
  the pricing/policy churn is a real risk. Treat as Phase 2, BYO‑credentials, and
  make the per‑post cost visible to the user.

### 3.3 LinkedIn — 🔴 **Not practical for this app**
- **What's possible in theory:** The **Community Management API** (Posts API) can
  publish text/image/video/document posts to a **personal profile or company
  Page** via `/rest/posts`.
- **Why it's blocked for us:**
  1. **Confidential client** — requires a **client secret**, i.e. a hosted backend
     we don't have (see §2.1).
  2. **Access approval is gated to legal entities** — LinkedIn states the
     Community Management API is available **only to registered legal entities
     (LLC, Corp, 501(c), …), not individual developers**, and each app goes
     through a **multi‑day review**. BYO credentials is therefore off the table
     for the typical hobbyist admin.
  3. **Audience mismatch** — a Palworld server community does not live on LinkedIn.
- **Verdict:** Skip unless a hosted backend is built *and* there is real demand.
  Even then, the entity/review requirement makes it a poor fit. Lowest priority.

### 3.4 Viber — 🔴 **Effectively closed**
- **What changed:** Since **5 Feb 2024**, Viber bots can only be created on
  **commercial terms** — you apply through Rakuten Viber or a verified partner
  (i.e. a paid business relationship). The old "anyone can spin up a public
  account bot for free" path is gone. Channels are a business‑solutions product.
- **Desktop fit / BYO:** A hobby server admin can't realistically get commercial
  bot terms, so BYO fails; and we won't run a paid commercial account on their
  behalf.
- **Verdict:** Not viable for this app's audience. Document it as "not supported —
  Viber is now commercial‑only" and move on. (If a specific commercial customer
  needs it, it can be handled as a bespoke Bridge plugin.)

### 3.5 YouTube — 🟡 (video upload) / 🔴 (community posts) — **the split behind item #12**
This is two very different things, and item #12 ("Post Builder and Publish —
YouTube") must pick which one it means.

- **Video upload — 🟡 feasible:** `videos.insert` on **Data API v3** uploads a
  video (e.g. a recorded event, a montage). **Good news:** on **4 Dec 2025**
  Google cut the upload quota cost from ~1,600 units to **~100 units**, so the
  default **10,000‑unit/day** quota now allows **~100 uploads/day** (was ~6).
  - **Auth:** OAuth 2.0 with PKCE (Google supports the loopback native flow, no
    secret required for a public client) — Tauri‑compatible.
  - **The real gate:** upload uses a **sensitive/restricted scope**, so a public
    production app needs Google's **OAuth verification** (branding + possibly a
    security review). In **BYO** mode each admin uses their *own* Google Cloud
    project and stays in "testing" mode (their own account only) — which sidesteps
    verification. That's the pragmatic path.
  - **Cost:** No per‑call fee; you're constrained by the daily quota, which is
    ample here.
- **Community posts (text/image/poll on the channel feed) — 🔴 not possible:**
  **There is no official YouTube API to create community posts.** Third‑party
  "solutions" are **scrapers**, not supported posting APIs, and would be fragile
  and ToS‑risky. If item #12's "post" meant a community/text post, **that
  specific thing cannot be built** on a supported API.
- **Verdict:** "Publish to YouTube" is realistic **only as video upload**, BYO
  Google project, Phase 2+. Community‑feed posting is out. Set expectations
  accordingly in the roadmap.

---

## 4. Feasibility summary

| Platform | Broadcast (A) | Notifications (B) | Chat relay (C) | Auth model | Cost | Desktop fit | Verdict |
| --- | :---: | :---: | :---: | --- | --- | --- | --- |
| **Telegram** | ✅ (channel) | ✅ | ✅ | Bot token (BYO) | Free | 🟢 | **Ship first** |
| **Discord** *(not in list — see §5)* | ✅ | ✅ | ✅ | Webhook / bot token (BYO) | Free | 🟢 | **Strongly recommend adding** |
| **X / Twitter** | ✅ | ➖ | ➖ | OAuth2 PKCE (BYO) | Pay‑per‑use (~$0.015–$0.20/post) | 🟡 | Phase 2, cost‑aware |
| **YouTube (video)** | ✅ upload | ➖ | ➖ | OAuth2 PKCE (BYO Google project) | Free (quota) | 🟡 | Phase 2+ |
| **YouTube (community post)** | ❌ | ❌ | ❌ | — | — | 🔴 | **No supported API** |
| **LinkedIn** | ⚠️ needs backend+entity | ➖ | ➖ | Confidential client + review | — | 🔴 | Skip |
| **Viber** | ⚠️ commercial‑only | ⚠️ commercial‑only | ⚠️ | Commercial bot terms | Paid/commercial | 🔴 | Skip |

---

## 5. Discord — the obvious omission
Item #13 doesn't list **Discord**, but a Palworld community almost certainly lives
there, and it is the **single easiest and highest‑value integration** available:
- **Incoming webhooks** need *no OAuth and no bot* — the admin pastes a webhook
  URL and we `POST` JSON (embeds, screenshots, event notifications). Perfect for
  notifications **(B)** and broadcast **(A)**.
- A **bot** additionally enables **two‑way chat relay (C)** and slash‑command
  admin actions.
- Free, well‑documented, Tauri‑ and Bridge‑friendly, BYO by design.

**Recommendation: add Discord to the scope.** It likely delivers more real value
to this audience than X, YouTube, LinkedIn, and Viber combined.

---

## 6. Item #12 — the "Post Builder"
The **Post Builder** is the UI half: a composer where the admin writes copy once,
attaches media (a **world‑map screenshot** — the app already renders the live
map to canvas, so exporting a PNG is straightforward — or an uploaded image/
video), and **fans it out** to the selected connected platforms. Design notes:

- **Composer**: title/body, media picker, per‑platform preview, and per‑platform
  overrides (X's length limits, link‑vs‑no‑link cost awareness, Telegram
  caption formatting).
- **Fan‑out**: each platform is a small "connector" behind a common interface
  (`compose → validate → publish`), so platforms can be added incrementally and
  failures are isolated per‑connector.
- **Templates & variables**: `{{serverName}}`, `{{playersOnline}}`, `{{uptime}}`,
  `{{eventName}}` — huge for recurring event/announcement posts.
- **Scheduling / automation** must run in the **Bridge** (§2.2), not the desktop
  app. Triggers worth exposing: server online/offline, scheduled restart
  countdown, crash detected, player‑count milestone, new day/wipe.
- **History/log**: what was posted where, success/failure, and (for X) the
  incurred cost.

---

## 7. Recommended phased plan

**Phase 0 — Foundation (design once, reuse everywhere)**
- A `SocialConnector` abstraction (`connect / validate / publish(text, media) /
  test`) plus a local, encrypted credential store (reuse the existing local‑secret
  pattern; never phone home).
- Decide the **Bridge‑owns‑automation, app‑owns‑composing** split up front.

**Phase 1 — Zero‑OAuth, highest value (weeks, not months)**
- **Telegram** bot‑token connector + **Discord** webhook connector.
- Event‑driven **notifications** in the Bridge (online/offline, restart, crash,
  milestones) → covers capability **B** immediately.
- A minimal **Post Builder** that fans out text + a map screenshot to Telegram/
  Discord.

**Phase 2 — OAuth broadcast (opt‑in, BYO credentials)**
- **X** connector via PKCE, with a setup wizard and **visible per‑post cost**.
- **YouTube video upload** via PKCE + BYO Google project.
- Scheduling & templates in the Post Builder.

**Phase 3 — Reassess the hard cases**
- Only build a hosted relay (and therefore **LinkedIn**, shared‑credential X) if
  there's demonstrated demand and a decision to relax "nothing phoned home".
- **Viber**: revisit only for a specific commercial customer.

---

## 8. Bottom line
- **Yes, meaningful social/messaging integration is possible** — but the winning
  version is **Telegram + Discord for notifications/relay** and a **Post Builder**
  that fans out to them, with **X and YouTube‑video** as opt‑in, bring‑your‑own‑
  credentials broadcast add‑ons.
- **Set expectations on the hard items:** **LinkedIn** (backend + legal entity +
  review), **Viber** (commercial‑only since 2024), and **YouTube *community*
  posts** (no supported API) are not realistic for an open‑source desktop tool as
  it stands.
- **The architecture matters more than any single API:** BYO credentials (no
  shipped secret), the **Bridge as the always‑on automation host**, and no
  mandatory backend keep the feature aligned with the project's local‑and‑private
  promise.

---

## Sources
- X (Twitter) API pricing 2026 (pay‑per‑use default, tiers closed to new signups):
  [Postproxy](https://postproxy.dev/blog/x-api-pricing-2026/) ·
  [Blotato](https://www.blotato.com/blog/twitter-api-pricing) ·
  [socialcrawl](https://www.socialcrawl.dev/blog/x-twitter-api-2026)
- YouTube Data API v3 quota / `videos.insert` cost reduction (Dec 2025):
  [Google quota calculator](https://developers.google.com/youtube/v3/determine_quota_cost) ·
  [getphyllo](https://www.getphyllo.com/post/youtube-api-limits-how-to-calculate-api-usage-cost-and-fix-exceeded-api-quota)
- No official YouTube community‑posts API (scrapers only):
  [postpeer](https://www.postpeer.dev/blog/best-youtube-posting-api) ·
  [Apify community‑posts scraper](https://apify.com/scrapestorm/youtube-community-posts-scraper/api)
- LinkedIn Community Management / Posts API + legal‑entity & review gating:
  [Microsoft Learn — Community Management](https://learn.microsoft.com/en-us/linkedin/marketing/community-management/community-management-overview) ·
  [Posts API](https://learn.microsoft.com/en-us/linkedin/marketing/community-management/shares/posts-api) ·
  [product catalog](https://developer.linkedin.com/product-catalog/marketing/community-management-api)
- Telegram Bot API (free, channels/photos/two‑way):
  [core.telegram.org/bots/api](https://core.telegram.org/bots/api)
- Viber bots commercial‑only since Feb 2024:
  [Viber REST API docs](https://developers.viber.com/docs/api/rest-bot-api/) ·
  [API access white paper](https://developers.viber.com/docs/general/api-access-white-paper/)
