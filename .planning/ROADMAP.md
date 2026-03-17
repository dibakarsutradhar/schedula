# Roadmap: Schedula Subscription Model

**Milestone:** v2.0 — Subscription & Monetization
**Requirements:** 25 v1 requirements across 4 phases
**Coverage:** 100% ✓

---

## Phase 1: Plan Limits & Metering

**Goal:** Enforce tier-based limits (batch count, admin count, algorithm access) in the hub server and standalone app, with structured error responses and in-app upgrade prompts. No billing infrastructure — this phase makes the product subscription-ready without requiring a payment processor.

**Requirements:** MTRG-01, MTRG-02, MTRG-03, MTRG-04, MTRG-05, MTRG-06, MTRG-07

**Deliverables:**
- `plan` column on `organizations` table (`free` | `pro` | `institution`)
- `PlanLimits` struct in `models.rs` with per-plan constants
- Limit checks in hub `handlers.rs` for batch creation, admin invite, algorithm selection, org creation
- Matching limit checks in Tauri `commands.rs` for standalone mode
- `plan_limit_exceeded` error shape: `{ code, plan, limit, current, feature, upgrade_url }`
- `GET /api/plan` endpoint returning current plan + all feature flags
- Upgrade prompt component in Svelte frontend (shown on limit-exceeded errors)

**Success Criteria:**
1. Creating more batches than the Free limit returns a structured error, not a crash
2. Inviting a second admin on Free tier is blocked with an error message containing an upgrade hint
3. Selecting CSP algorithm on Free tier is rejected at the API layer
4. Standalone app enforces the same batch/admin limits as the hub
5. Upgrade prompt appears in the UI when any limit is hit
6. All existing Pro/Institution features work without restriction when plan = `pro` or `institution`

---

## Phase 2: Licensing Server & Token Validation

**Goal:** Build a small licensing server (Cloudflare Worker + KV, or minimal Axum service) that issues signed JWT license tokens. Hub server validates tokens locally (no network call per request) with a 7-day offline grace period. Admins can activate a license key via the hub admin UI.

**Requirements:** LICN-01, LICN-02, LICN-03, LICN-04, LICN-05, LICN-06

**Deliverables:**
- `licenses` table in hub SQLite: `{org_id, plan, token, expires_at, last_validated_at}`
- License JWT schema: `{sub: org_id, plan, exp, iat, iss: "schedula-license"}` signed with RS256 private key
- Hub startup: reads license from DB, validates signature with embedded public key
- Background task: re-validates against `https://license.schedula.app/v1/validate` every 24h
- Grace period logic: if validation fails and `last_validated_at` < 7 days ago → downgrade to Free
- `GET /api/license` endpoint: returns `{plan, expires_at, features, days_until_expiry}`
- `POST /api/license/activate` endpoint: accepts license key, fetches token from licensing server, stores in DB
- Hub admin UI: License status card with key input field
- Licensing server: `POST /v1/issue` (called by billing webhook), `POST /v1/validate` (called by hub)

**Success Criteria:**
1. Hub correctly identifies plan from license token without a network call
2. Tampering with the token causes immediate downgrade to Free
3. Hub continues working at the licensed plan for 7 days without network access
4. After 7 days offline, hub downgrades to Free plan
5. Admin can enter a license key and activate it within the hub admin UI
6. `GET /api/license` returns accurate feature flags matching the current plan

---

## Phase 3: Stripe Checkout & Subscriptions

**Goal:** Integrate Stripe for self-service subscription management. Customers can subscribe to Pro or Institution plans via hosted Stripe checkout, manage their subscription via the Stripe Customer Portal, and get a 14-day free trial on first hub deploy. Webhooks trigger license re-issuance.

**Requirements:** STRP-01, STRP-02, STRP-03, STRP-04, STRP-05, STRP-06

**Deliverables:**
- Stripe product/price catalog: Pro Monthly, Pro Annual, Institution Monthly, Institution Annual
- `POST /billing/checkout` → creates Stripe Checkout Session, redirects customer
- Stripe webhook handler: `customer.subscription.{created,updated,deleted}` → calls licensing server `POST /v1/issue` → emails new token to customer
- `GET /billing/portal` → creates Stripe Customer Portal session, redirects
- 14-day trial: `trial_period_days: 14` on first subscription, no card required
- Cancellation grace: on `customer.subscription.deleted`, schedule plan downgrade to end of period
- Landing page updated: pricing section links to Stripe checkout
- Email templates: trial started, payment received (with license key), cancellation confirmed, trial expiring in 3 days

**Success Criteria:**
1. Customer can complete Pro checkout in under 2 minutes from the pricing page
2. License token delivered via email within 60 seconds of successful payment
3. Cancellation schedules downgrade (not immediate cutoff) with confirmation email
4. Customer Portal allows plan upgrade, downgrade, and billing history access
5. 14-day trial activates on first hub deploy with no credit card required
6. Webhook retries correctly handle temporary licensing server unavailability

---

## Phase 4: Invoice Flow & Paddle Integration

**Goal:** Add invoice/purchase-order support for Institution tier (universities can't use credit cards) and Paddle as an international payment option that handles VAT/GST automatically. Annual billing option added to all paid tiers.

**Requirements:** INVC-01, INVC-02, INVC-03, PADL-01, PADL-02, PADL-03

**Deliverables:**
- `POST /billing/invoice-request` endpoint: captures org name, contact email, plan, user count, country → notifies sales (email + Slack)
- Manual license issuance flow: admin dashboard to mark invoice paid and issue license JWT
- Annual billing: Stripe annual prices created; toggle on pricing page (monthly ↔ annual, showing 2-months-free saving)
- Paddle product catalog mirroring Stripe (Pro + Institution, monthly + annual)
- Paddle checkout integration (`paddle.js` embedded on pricing page, checkout overlay)
- Paddle webhook handler: mirrors Stripe handler → same license issuance flow
- Paddle Retain for tax collection (VAT, GST, etc.)

**Success Criteria:**
1. University procurement officer can submit invoice request and receive a quote email within 1 business day
2. After manual invoice payment confirmation, license token is issued and emailed within 5 minutes
3. Annual billing shows correct "2 months free" discount vs monthly price
4. Customer in EU (VAT) and India (GST) is charged correct tax via Paddle
5. Paddle checkout completes and issues license within 60 seconds of payment
6. Paddle and Stripe flows produce identical license tokens (same JWT schema)

---

## Phase Summary

| # | Phase | Goal | Requirements | Status |
|---|-------|------|--------------|--------|
| 1 | Plan Limits & Metering | Enforce tier limits, no billing needed | MTRG-01–07 | ✓ Complete |
| 2 | Licensing Server | Signed JWT tokens, offline-tolerant validation | LICN-01–06 | ✓ Complete |
| 3 | Stripe Checkout | Self-service subscriptions, trials, webhooks | STRP-01–06 | ✓ Complete |
| 4 | Invoice & Paddle | PO flow for universities, international billing | INVC-01–03, PADL-01–03 | ✓ Complete |

---

*Roadmap created: 2026-03-17*
*Milestone: v2.0 Subscription & Monetization*
