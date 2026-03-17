# Project State

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-03-17)

**Core value:** Universities get Schedula free on one machine; paying unlocks sync and scale, enforced without breaking offline-first.
**Current milestone:** v2.0 — Subscription & Monetization
**Current focus:** v2.0 milestone complete ✓

## Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 1 — Plan Limits & Metering | ✓ Complete | All 7 requirements implemented |
| 2 — Licensing Server | ✓ Complete | RS256 JWT, license-server binary, hub integration, admin UI |
| 3 — Stripe Checkout | ✓ Complete | Checkout, portal, webhooks, email delivery, landing page pricing |
| 4 — Invoice & Paddle | ✓ Complete | invoice-request endpoint, admin dashboard, Paddle webhooks, landing page Paddle+invoice UI |

## Last Session

2026-03-17 — All 4 phases complete. v2.0 milestone delivered.

Phase 4 implemented:
- `license-server/src/billing.rs`: `invoice_request_handler`, `list_invoices_handler`, `issue_invoice_handler`, `admin_handler`, `paddle_webhook_handler`, `notify_sales`
- `invoice_requests` table, `customers.paddle_customer_id` column migration
- Paddle webhook: `subscription.created/updated/canceled`, `transaction.payment_failed`; sig verification `ts:body` HMAC-SHA256
- Admin dashboard HTML (`GET /admin?key=...`): invoice management, license revocation
- Landing page: Paddle buttons (shown when `PADDLE_CLIENT_TOKEN` is set), invoice request modal with full form, success/error states, `openPaddleCheckout()`, `openInvoiceRequest()`
- `.env.example`: added `PADDLE_*`, `SALES_EMAIL`, `SLACK_WEBHOOK_URL` vars
- CLI args: `paddle_api_key`, `paddle_webhook_secret`, `paddle_price_*` (4), `paddle_client_token`, `sales_email`, `slack_webhook_url`

2026-03-17 — Phase 2 complete. Implemented:
- `license-server/` standalone Axum binary (issue, validate, list, revoke)
- RS256 key pair: private in license-server, public embedded in hub at compile time
- `hub-server/src/license.rs`: validate_token, effective_plan, background re-validation loop, 7-day grace period
- `licenses` table (migrate_v11), `LicenseClaims`, `LicenseInfo` models
- Hub routes: GET /api/license (public), POST /api/license/activate, POST /api/license/deactivate
- Hub admin UI: license status card, activate/deactivate flow
- Standalone Tauri: license commands stubbed (always returns "none")

Key decisions confirmed:
- Gate sync (hub) not core scheduling
- Org-based pricing (Free / Pro $29/mo / Institution $99/mo)
- Phase 1 = limits only, no billing server dependency
- License JWT approach for offline-tolerant validation (Phase 2+)
- Private key never in hub binary; embedded public key for local validation
