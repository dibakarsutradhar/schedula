# Schedula — Subscription Model

## What This Is

A subscription monetization layer for Schedula, a desktop (Tauri) app for university academic scheduling. The subscription system gates scale and sync features behind paid tiers (Free / Pro / Institution) while keeping the core scheduling experience free. Enforcement is implemented inside the existing hub server binary — no separate SaaS infrastructure needed for Phase 1.

## Core Value

Universities and departments can try Schedula for free on a single machine; paying unlocks multi-machine sync, larger datasets, and advanced features — enforced transparently without degrading the offline-first experience.

## Requirements

### Validated

- ✓ Tauri desktop app with standalone (offline) mode — existing
- ✓ Axum hub server for multi-machine WebSocket sync — existing
- ✓ Role system: super_admin > admin > viewer — existing
- ✓ Organizations, semesters, batches, courses, rooms, lecturers — existing
- ✓ Two scheduling algorithms (Greedy + CSP) — existing
- ✓ Bulk CSV import for lecturers, rooms, courses — existing
- ✓ Audit logs, utilization reports, data health checks — existing
- ✓ Approval workflow for password resets / account unlocks — existing

### Active

- [ ] Plan tier limits enforced in hub server (batch count, admin count, algorithm gate)
- [ ] Plan tier limits enforced in standalone Tauri app
- [ ] License JWT issued and validated (signed, offline-tolerant)
- [ ] Stripe checkout + webhook → license activation
- [ ] Customer self-service portal (upgrade, cancel, billing history)
- [ ] Invoice / purchase-order flow for Institution tier
- [ ] Paddle as international billing fallback (handles VAT/GST)
- [ ] In-app upgrade prompts when limits are hit
- [ ] Free 14-day Pro trial on first hub deploy

### Out of Scope

- Per-seat pricing — complexity not justified; org-based pricing fits university procurement
- Mobile app billing — web/desktop only for now
- SSO / SAML — Enterprise tier, future milestone
- Usage-based pricing — flat tiers are simpler for academic budgets
- White-labelling — future milestone

## Context

- **Architecture**: Hub server is the natural enforcement point for Pro/Institution features (sync, multi-org, large datasets). Standalone app enforces Free-tier limits locally.
- **Target buyers**: Academic departments, IT procurement offices — budget cycles are annual; invoice/PO support is essential for Institution tier.
- **Offline requirement**: Universities often have restricted networks; license validation must tolerate 7-day offline grace period.
- **Existing auth**: Hub uses JWT (24h) stored in SQLite; license tokens will reuse the same signing infrastructure.

## Constraints

- **Tech stack**: Rust (Axum + SQLite) for hub; no new backend language unless strictly necessary
- **Offline-first**: License validation must not require a live network call per request
- **Deployment**: Hub runs on-premise at universities; can't assume outbound HTTPS is always available
- **No separate billing infra in Phase 1**: All Phase 1 work is local enforcement only; billing server comes in Phase 3

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Gate sync (hub) not core scheduling | Keeps Schedula genuinely useful for free; avoids hostile paywall | — Pending |
| Org-based pricing (not per-seat) | Universities buy departmentally; per-seat is hard to procurement | — Pending |
| Paddle as MoR for international | Handles EU VAT, Indian GST automatically; avoids compliance burden | — Pending |
| License JWT (offline-tolerant) | Universities have restrictive network policies | — Pending |
| Phase 1 = limits only, no billing | Ship value fast; validate limits before building payment infra | — Pending |

---
*Last updated: 2026-03-17 after initial subscription model planning*
