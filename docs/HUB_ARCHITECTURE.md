# Hub Server Architecture — Control, Licensing & Enforcement

## Overview

Schedula ships in two modes. The same desktop binary handles both:

```
Standalone mode (always free)          Hub mode (Pro / Institution)
─────────────────────────────          ─────────────────────────────
Desktop app ←→ local SQLite            Desktop app ←→ Hub server ←→ SQLite
                                                  ↑
                                       Other desktop apps on other machines
```

The hub server is the authority for any multi-machine deployment. Every
feature gate, plan limit, and license check happens there — the desktop app
trusts whatever the hub tells it.

---

## 1. How the Hub Controls Installed Apps

### 1.1 Connection model

When a desktop app is configured for hub mode it does the following at startup:

```
Desktop App                         Hub Server
    │                                    │
    │── POST /api/auth/login ──────────► │  credentials → session JWT
    │◄─ { token, role, plan, limits } ── │
    │                                    │
    │── WS /ws?token=<jwt> ───────────► │  upgrade to WebSocket
    │◄──────────────── connected ─────── │
    │                                    │
    │   (all subsequent API calls        │
    │    carry Authorization: Bearer)    │
```

The session payload the hub returns includes:

```json
{
  "user_id": 42,
  "username": "alice",
  "role": "admin",
  "org_id": 7,
  "plan": "pro",
  "limits": {
    "max_batches": 50,
    "max_admins": 5,
    "csp_algorithm": true,
    "bulk_import": true,
    "multi_machine_sync": true
  }
}
```

The desktop frontend uses `limits` to show/hide features in the UI. It does NOT
make independent decisions — if the hub says `csp_algorithm: false`, the
algorithm selector is hidden and the request would be rejected server-side
anyway.

### 1.2 Real-time sync via WebSocket

```
Machine A                  Hub Server              Machine B
    │                          │                       │
    │── POST /api/schedules ──►│                       │
    │◄─ 201 Created ───────────│                       │
    │                          │── broadcast event ───►│
    │                          │  { type: "schedule",  │
    │                          │    action: "created", │
    │                          │    data: {...} }       │
```

Every mutating API call (create/update/delete on schedules, batches, etc.)
publishes a typed event to all other WebSocket connections belonging to the
same `org_id`. Connected apps apply optimistic updates locally without polling.

### 1.3 What the hub is authoritative for

| Concern | Where enforced | Notes |
|---------|---------------|-------|
| Authentication | Hub — `/api/auth/login` | bcrypt, session JWT |
| Authorization (role) | Hub — every handler checks `sess.role` | admin vs viewer vs super_admin |
| Plan limits | Hub — before any write operation | described in §2 |
| License validity | Hub — at startup + every 24h | described in §3 |
| Audit log | Hub — every mutation is logged | stored in `audit_log` table |
| Real-time sync | Hub — WebSocket broadcast | same org only |
| Data isolation | Hub — all queries scoped to `org_id` | SQL-level, not app-level |

The desktop app is essentially a thin client that renders data and submits
commands. It cannot bypass any of the above by modifying its local binary.

---

## 2. Plan Limits & Metering

### 2.1 Plan tiers

```
Free                Pro ($29/mo)         Institution ($99/mo)
────────────────    ─────────────────    ────────────────────────
10 batches/sem      50 batches/sem       Unlimited
1 admin             5 admins             Unlimited admins
Greedy scheduler    CSP scheduler        CSP scheduler
No sync             Hub sync             Hub sync
No bulk import      Bulk CSV import      Bulk CSV import
No utilization      Utilization reports  Utilization reports + audit
```

### 2.2 Where limits are enforced

Limits are checked **in the hub handler, before the database write**. There is
no client-side enforcement — the client only reflects what the hub allows.

```rust
// hub-server/src/handlers.rs — batch creation example
pub fn create_batch(conn, sess, batch) -> Result<i64, String> {
    let plan   = get_org_plan(conn, sess.org_id);
    let limits = PlanLimits::for_plan(&plan);

    if limits.max_batches != -1 {
        let current: i64 = conn.query_row(
            "SELECT COUNT(*) FROM batches WHERE org_id=?1 AND semester_id=?2",
            params![sess.org_id, batch.semester_id], |r| r.get(0)
        )?;
        if current >= limits.max_batches {
            return Err(plan_limit_err(PlanLimitError {
                code:        "plan_limit_exceeded",
                plan:        plan,
                feature:     "batches",
                limit:       limits.max_batches,
                current:     current,
                upgrade_url: UPGRADE_URL,
            }));
        }
    }
    // ... proceed with INSERT
}
```

### 2.3 Error shape

When a limit is hit, the hub returns HTTP 402 with a structured body:

```json
{
  "error": "plan_limit_exceeded",
  "code": "plan_limit_exceeded",
  "plan": "free",
  "feature": "batches",
  "limit": 10,
  "current": 10,
  "upgrade_url": "https://schedula.app/pricing"
}
```

The desktop frontend intercepts this shape, dismisses the loading state, and
shows the `UpgradePrompt` modal with the feature name, current/max counts, and
a link to the pricing page.

### 2.4 Where the plan value comes from

The `plan` column lives on the `organizations` table:

```sql
-- Added by migrate_v10
ALTER TABLE organizations ADD COLUMN plan TEXT NOT NULL DEFAULT 'free';
```

It is updated in two ways:
1. **License activation** — `POST /api/license/activate` validates the JWT,
   reads the `plan` claim, and writes it to `organizations.plan`.
2. **License expiry/revocation** — the background re-validation task or a
   webhook downgrades the column back to `'free'`.

The column is the single source of truth at runtime. The license JWT is the
proof that the column was legitimately upgraded.

---

## 3. Subscription Enforcement (License Flow)

### 3.1 End-to-end flow

```
Customer                License Server              Hub Server
    │                         │                          │
    │── Stripe/Paddle checkout►│                          │
    │◄─ Payment confirmed ─────│                          │
    │                         │── issue JWT ─────────────│(stored to DB)
    │◄─ Email: license key ────│                          │
    │                         │                          │
    │── POST /api/license/activate  ──────────────────►  │
    │   { token: "<jwt>" }                               │
    │◄─ { plan: "pro", expires_at: "...", ... } ─────────│
    │                         │                          │
    │   (every 24h)           │◄─ POST /v1/validate ─────│
    │                         │   { token: "<jwt>" }     │
    │                         │──► { valid: true } ──────│
```

### 3.2 JWT structure

```json
{
  "sub": "org:7",
  "plan": "pro",
  "org_name": "Acme University",
  "exp": 1780000000,
  "iat": 1750000000,
  "jti": "a1b2c3d4-...",
  "iss": "schedula-license"
}
```

Signed with RS256. The **private key** lives only in the license server.
The **public key** is embedded in the hub binary at compile time
(`hub-server/keys/license_public.pem` → `include_str!`). This means:

- The hub can verify any license JWT **without a network call**.
- An attacker who controls the hub binary still cannot forge tokens (they
  would need the private key, which is never in the hub).

### 3.3 Hub startup sequence

```
open_db()
  → run migrations (v1 … v11)
  → seed super-admin if empty

license::effective_plan(conn)
  → read latest active license from `licenses` table
  → validate JWT signature with embedded public key
  → if valid AND not expired → return plan from JWT
  → if valid BUT within 7-day grace window → return plan, schedule validation
  → if expired/invalid/missing → return "free"

update organizations SET plan = <effective_plan> WHERE id = <org_id>

spawn background task: revalidate every 24h
```

### 3.4 Offline grace period

The 7-day grace period means universities can operate through network outages
or license server downtime without losing Pro features:

```
Day 0: license validated, last_validated_at = now
Day 1: network outage — validation fails, grace period starts
Day 7: still within grace period — plan = pro, warning shown in admin UI
Day 8: grace period exceeded — plan downgraded to free in organizations table
       all connected apps receive a WebSocket event { type: "plan_downgraded" }
       UpgradePrompt shown on next feature-gated action
```

### 3.5 Tamper resistance

If the JWT is modified after issuance:

1. RS256 signature verification fails in `validate_token()`.
2. `effective_plan()` returns `"free"` immediately.
3. `organizations.plan` is set to `"free"`.
4. No feature-gated operations succeed until a valid token is activated.

The `organizations.plan` column can be edited directly in SQLite by someone
with filesystem access to the hub's data directory. This is an accepted
trade-off: the hub is self-hosted and the operator is trusted. If this is a
concern, the plan could be derived entirely from the live JWT on every request
(CPU cost) rather than stored — this is a hardening option for future versions.

---

## 4. Rate Limiting

### 4.1 Current state

The hub does not implement per-IP or per-request HTTP rate limiting today. The
metering limits (§2) are the only enforcement mechanism. For most university
deployments this is sufficient — the hub is internal, behind a VPN or
Cloudflare Tunnel, and not exposed to the open internet.

### 4.2 Recommended additions (not yet implemented)

**Layer 1 — Cloudflare in front of the hub (zero-code)**

If the hub is exposed publicly via Cloudflare Tunnel or a reverse proxy, use
Cloudflare's WAF + Rate Limiting rules. Example rule:

```
URI path: /api/*
Rate: 100 requests per 10 seconds per IP
Action: Block (429)
```

This handles brute-force login attempts and API abuse without touching the hub
codebase.

**Layer 2 — tower-governor middleware (Axum-native)**

Add per-IP rate limiting directly in the hub:

```toml
# hub-server/Cargo.toml
tower-governor = "0.4"
```

```rust
// hub-server/src/main.rs
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

let governor_conf = GovernorConfigBuilder::default()
    .per_second(10)   // 10 requests per second per IP
    .burst_size(50)   // allow burst of 50
    .finish().unwrap();

let app = Router::new()
    // ...routes...
    .layer(GovernorLayer { config: Arc::new(governor_conf) });
```

**Layer 3 — per-org API quotas (future)**

For usage-based billing or abuse prevention, add a quota table:

```sql
CREATE TABLE org_api_usage (
    org_id    INTEGER NOT NULL,
    day       TEXT    NOT NULL,  -- YYYY-MM-DD
    req_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (org_id, day)
);
```

Check and increment atomically in a middleware layer before routing.

---

## 5. Deployment

### 5.1 Recommended platforms

**Self-hosted (primary use case)**

The hub is designed to run on a machine inside the university network. This is
the zero-cost option and keeps all data on-premises:

```bash
# Build
cd hub-server && cargo build --release

# Run
./target/release/schedula-hub \
  --db-path /var/lib/schedula/hub.db \
  --port 3001 \
  --admin-key "$(openssl rand -hex 32)"
```

Expose externally (optional) via **Cloudflare Tunnel**:

```bash
cloudflared tunnel --url http://localhost:3001
# → https://random-name.trycloudflare.com (free, no firewall changes)
```

**Cloud deployment (for Schedula as a SaaS hub)**

| Platform | Why | Command |
|----------|-----|---------|
| **Fly.io** | Persistent volume, WebSockets, global | `fly deploy` |
| **Railway** | Simplest Docker deploy | `railway up` |
| **Render** | Free tier with disk | Deploy from Dockerfile |

All three support:
- Persistent SQLite via a mounted volume (`/data/hub.db`)
- WebSocket connections without configuration
- Environment variables for secrets

> ⚠️ **Why not Cloudflare Workers**: Workers are stateless, request-scoped
> processes. They do not support persistent WebSocket connections without
> Durable Objects (different programming model + significant extra cost),
> cannot run background tasks, and do not support `rusqlite`. The hub would
> need to be completely rewritten to target Workers.

### 5.2 Dockerfile

```dockerfile
FROM rust:1.75-slim AS builder
WORKDIR /app
COPY hub-server/ ./hub-server/
WORKDIR /app/hub-server
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/hub-server/target/release/schedula-hub /usr/local/bin/
VOLUME ["/data"]
EXPOSE 3001
ENV DB_PATH=/data/hub.db
CMD ["schedula-hub"]
```

### 5.3 Environment variables

```bash
HUB_ADMIN_KEY=<secret>           # protects /admin routes
HUB_PORT=3001
HUB_DB_PATH=/data/hub.db
LICENSE_SERVER_URL=https://license.schedula.app   # for re-validation calls
```

---

## 6. Security Considerations

| Threat | Mitigation |
|--------|-----------|
| Forged license JWT | RS256 — private key never leaves license server |
| Stolen session JWT | Short expiry (24h) + re-login on expiry |
| SQL injection | All queries use `rusqlite` parameterized statements |
| Unauthorized org data access | All queries include `AND org_id = ?` |
| Admin key brute-force | Use a 256-bit random key; rate-limit `/admin` |
| Direct SQLite manipulation | Accepted trade-off for self-hosted model |
| Replay of revoked license | License server maintains revocation list; hub checks on 24h cycle |

---

*Last updated: 2026-03-17*
*Covers: hub-server v2.0, license-server v1.0*
