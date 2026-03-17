# Requirements: Schedula Subscription Model

**Defined:** 2026-03-17
**Core Value:** Universities can try Schedula free on one machine; paying unlocks sync and scale, enforced without breaking the offline-first experience.

## v1 Requirements

### Metering (Plan Limits)

- [ ] **MTRG-01**: Hub server rejects batch creation when org exceeds plan limit (Free: 10, Pro: 50, Institution: unlimited)
- [ ] **MTRG-02**: Hub server rejects admin invite when org exceeds plan limit (Free: 1, Pro: 5, Institution: unlimited)
- [ ] **MTRG-03**: Hub server rejects CSP algorithm requests on Free tier
- [ ] **MTRG-04**: Hub server rejects multi-org creation on Free tier
- [ ] **MTRG-05**: Standalone Tauri app enforces same batch and admin limits locally
- [ ] **MTRG-06**: API returns structured `plan_limit_exceeded` error with current plan, limit, and upgrade hint
- [ ] **MTRG-07**: In-app upgrade prompt shown when a limit-exceeded error is received

### Licensing

- [ ] **LICN-01**: Hub server reads license token from `licenses` table in SQLite on startup
- [ ] **LICN-02**: License token is a signed JWT containing `{org_id, plan, expires_at, issued_at}`
- [ ] **LICN-03**: Hub server validates token signature locally (no network call per request)
- [ ] **LICN-04**: Hub re-validates against licensing server every 24h; 7-day grace period if unreachable
- [ ] **LICN-05**: `GET /api/license` endpoint returns current plan, expiry, and feature flags
- [ ] **LICN-06**: Admin can activate a license key via hub admin UI or CLI flag `--license-key`

### Stripe Integration

- [ ] **STRP-01**: Stripe checkout session created for Pro and Institution plans (monthly + annual)
- [ ] **STRP-02**: Stripe webhook `customer.subscription.created/updated/deleted` updates license record
- [ ] **STRP-03**: License JWT re-issued on successful payment; delivered to customer via email
- [ ] **STRP-04**: Customer portal link (`GET /billing/portal`) for self-service upgrade/cancel/billing history
- [ ] **STRP-05**: 14-day free Pro trial activated automatically on first hub deploy (no card required)
- [ ] **STRP-06**: Cancellation downgrades plan to Free at end of billing period (no immediate cutoff)

### Invoice / PO Flow

- [ ] **INVC-01**: Institution tier can be activated via manual invoice (admin marks paid, issues license JWT)
- [ ] **INVC-02**: `POST /billing/invoice-request` captures contact details, org size, plan; notifies sales
- [ ] **INVC-03**: Annual billing option available for all paid tiers (2 months free)

### Paddle (International)

- [ ] **PADL-01**: Paddle checkout available as alternative to Stripe (selected per region or customer preference)
- [ ] **PADL-02**: Paddle webhook mirrors Stripe webhook handling; same license issuance flow
- [ ] **PADL-03**: Tax (VAT/GST) collected and remitted automatically via Paddle MoR

## v2 Requirements

### Analytics & Dunning

- **ANLY-01**: Conversion funnel tracking (Free → trial → Pro → Institution)
- **ANLY-02**: Dunning emails for failed payments (Stripe Radar integration)
- **ANLY-03**: Usage metrics dashboard (batch count, schedule generations per org)

### Enterprise

- **ENTR-01**: SSO / SAML integration for Institution tier
- **ENTR-02**: Custom contract pricing for multi-department deployments
- **ENTR-03**: Dedicated support channel (Slack Connect) for Institution tier

### Education / NGO Discount

- **DISC-01**: Discount code system (30–50% off for verified .edu/.ac domains)
- **DISC-02**: Admin approval flow for discount verification

## Out of Scope

| Feature | Reason |
|---------|--------|
| Per-seat pricing | Doesn't fit university procurement; adds accounting complexity |
| Mobile billing | No mobile app in scope |
| White-labelling | Future milestone, requires separate branding infrastructure |
| Usage-based (metered) billing | Flat tiers are simpler for annual academic budgets |
| Crypto payments | Not relevant to target market |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| MTRG-01 | Phase 1 | Pending |
| MTRG-02 | Phase 1 | Pending |
| MTRG-03 | Phase 1 | Pending |
| MTRG-04 | Phase 1 | Pending |
| MTRG-05 | Phase 1 | Pending |
| MTRG-06 | Phase 1 | Pending |
| MTRG-07 | Phase 1 | Pending |
| LICN-01 | Phase 2 | Pending |
| LICN-02 | Phase 2 | Pending |
| LICN-03 | Phase 2 | Pending |
| LICN-04 | Phase 2 | Pending |
| LICN-05 | Phase 2 | Pending |
| LICN-06 | Phase 2 | Pending |
| STRP-01 | Phase 3 | Pending |
| STRP-02 | Phase 3 | Pending |
| STRP-03 | Phase 3 | Pending |
| STRP-04 | Phase 3 | Pending |
| STRP-05 | Phase 3 | Pending |
| STRP-06 | Phase 3 | Pending |
| INVC-01 | Phase 4 | Pending |
| INVC-02 | Phase 4 | Pending |
| INVC-03 | Phase 4 | Pending |
| PADL-01 | Phase 4 | Pending |
| PADL-02 | Phase 4 | Pending |
| PADL-03 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 25 total
- Mapped to phases: 25
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-17*
*Last updated: 2026-03-17 after initial definition*
