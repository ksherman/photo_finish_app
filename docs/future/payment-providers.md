# Payment Provider Comparison

**Status:** Research / Future
**Date:** 2026-03-29

---

## Context

PhotoFinish needs payment processing for two channels:

- **Card-present (CP):** ~$250K/season at kiosks during meets. Checkout terminals have internet access.
- **Card-not-present (CNP):** ~$100K/season projected for online photo ordering (future).
- **Total:** ~$350K/season

Average ticket ~$100 at kiosks (~2,500 transactions), ~$80 online (~1,250 transactions).

Current plan (see `docs/08-payments.md`) uses Square POS with a QR code → staff iPhone workflow designed for offline kiosks. With internet-capable terminals, more options are viable.

---

## Pricing Models

Understanding the two models is more important than memorizing rates:

**Flat-rate** (Square, Stripe): You pay the same percentage regardless of card type. Simple, predictable, but you're overpaying on cheap-to-process cards (debit, non-rewards).

**Interchange-plus** (Helcim, Stax, etc.): You pay the actual network cost (interchange, ~1.7% CP / ~2.1% CNP) plus a small fixed markup. Cheaper at volume, but your effective rate varies by card mix.

At $350K/year, interchange-plus saves roughly **$2,000-3,500/season** over flat-rate.

---

## Provider Comparison

*Rates are estimates based on published pricing and available volume tiers. Verify with each provider before committing — rates change.*

### Tier 1: Best Developer Experience

#### Square

- **CP rate:** 2.6% + $0.15 (free plan); negotiable at $300K+
- **CNP rate:** 3.3% + $0.30 (free plan); 2.9% + $0.30 (Plus $49/mo)
- **Custom rates:** Available at your volume — call sales, could get ~2.2% + $0.10 CP
- **Hardware:** Tap to Pay on iPhone (free), Square Reader ($49), Square Terminal ($299)
- **API:** Good. POS API for QR-based checkout, Terminal API for direct hardware control, Online Payments API for web.
- **Elixir ecosystem:** No official SDK; community libraries exist. REST API is clean.
- **Est. annual cost:** ~$10,550 (free plan) → ~$8,650 (custom rates)

**Strengths:** Already designed around; excellent card-present hardware; mature mobile POS. With internet-capable terminals, Terminal API replaces QR workaround for automated payment-to-order linking.

**Weaknesses:** Flat-rate pricing is expensive at this volume unless you negotiate. Online rate on free tier (3.3%) is notably high.

#### Stripe

- **CP rate:** 2.7% + $0.05 (Terminal); negotiable at volume
- **CNP rate:** 2.9% + $0.30; negotiable at volume
- **Custom rates:** Available — could get ~2.2% + $0.05 CP, ~2.5% + $0.25 CNP
- **Hardware:** Stripe Reader S2 (~$349), BBPOS WisePOS E (~$249). Requires internet.
- **API:** Best in class. Excellent documentation, webhooks, unified dashboard.
- **Elixir ecosystem:** `stripity_stripe` is mature and well-maintained.
- **Est. annual cost:** ~$10,150 (standard) → ~$8,440 (custom rates)

**Strengths:** Best API and developer experience by far. Best Elixir library. Stripe Terminal works now that kiosks have internet. Unified CP + CNP in one integration. Excellent dashboard and reporting.

**Weaknesses:** Flat-rate pricing (same as Square). Terminal hardware less battle-tested than Square's. Stripe Terminal is newer and has had some rough edges with offline/intermittent connectivity.

### Tier 2: Best Value (Interchange-Plus)

#### Helcim

- **CP rate:** Interchange + 0.20-0.40% + $0.06-0.08 (rate drops with volume)
- **CNP rate:** Interchange + 0.25-0.50% + $0.15-0.25 (rate drops with volume)
- **At $350K/year:** Likely ~interchange + 0.25% + $0.07 (CP), ~interchange + 0.35% + $0.20 (CNP)
- **Monthly fee:** $0. No setup, cancellation, or hidden fees.
- **Hardware:** Helcim Smart Terminal (~$329), Helcim Card Reader (~$99). Tap/chip/swipe.
- **API:** Solid REST API. Tokenization, recurring billing, CP + CNP unified. No Elixir SDK.
- **Est. annual cost:** ~$7,440

**Strengths:** Cheapest option at this volume. No monthly fees. Transparent interchange-plus pricing that auto-adjusts as volume grows. Clean API. One account for both CP and CNP.

**Weaknesses:** No Elixir SDK (write a thin HTTP wrapper). Hardware less polished than Square/Stripe. Smaller community / fewer integration examples. Less battle-tested for custom kiosk setups.

#### Stax (formerly Fattmerchant)

- **Pricing model:** Monthly subscription + interchange + per-transaction fee (no percentage markup).
- **CP rate:** Interchange + $0.08/transaction
- **CNP rate:** Interchange + $0.15/transaction
- **Subscription:** ~$99/mo (Growth plan, up to $500K/year)
- **Hardware:** Supported third-party terminals (Dejavoo, SwipeSimple). ~$200-400.
- **API:** Functional REST API, not as polished as Stripe/Helcim.
- **Est. annual cost:** ~$7,925 (including $1,188 subscription)

**Strengths:** The subscription model means zero percentage markup — you pay interchange and a flat per-transaction fee. Very transparent. At high volumes and larger ticket sizes, this model is extremely efficient.

**Weaknesses:** $99/mo floor regardless of volume (hurts in off-season). API and developer experience are weaker. Hardware options less integrated. Less ideal for a custom kiosk workflow.

### Tier 3: Worth Knowing About

#### PayJunction

- **CP rate:** Interchange + ~0.15-0.30% + per-transaction fee (quote-based)
- **CNP rate:** Interchange + higher markup (quote-based)
- **Monthly fee:** ~$25-45/mo
- **Hardware:** PayJunction Smart Terminal (Android-based, NFC/chip/swipe, ~$300-500)
- **API:** REST API (Trinity API). Adequate documentation. No Elixir SDK.
- **Est. annual cost:** Requires quote — likely in the $7,000-8,500 range

**Strengths:** Good smart terminal hardware. Interchange-plus pricing. Reasonable API.

**Weaknesses:** Pricing not transparent (must get a quote). Smaller developer community. API is functional but not modern.

#### Adyen

- **Pricing:** Interchange++ (interchange + scheme fee + Adyen markup, ~$0.10-0.12/txn)
- **API:** World-class. Best unified commerce platform available.
- **Hardware:** Excellent terminals.
- **Accessibility:** Historically requires $1M+ annual volume. Moving downmarket but **$350K/year is likely too small for a direct account.**

Mentioning for awareness only — revisit if volume grows significantly.

#### Dharma Merchant Services

- **CP rate:** Interchange + 0.15% + $0.07
- **Monthly fee:** ~$25/mo
- **API:** Weak. Uses backend processors, no robust direct API for custom integration.

Not a fit for a custom Elixir kiosk integration — rates are competitive but you'd be stuck with their terminal UI, not your own.

---

## Estimated Annual Cost Summary

| Provider | Model | Est. CP Cost | Est. CNP Cost | Fees | **Total** |
|----------|-------|-------------|--------------|------|-----------|
| Square (free) | Flat-rate | $6,875 | $3,675 | $0 | **$10,550** |
| Stripe (standard) | Flat-rate | $6,875 | $3,275 | $0 | **$10,150** |
| Square (custom) | Flat-rate negotiated | $5,750 | $2,913 | $0 | **~$8,650** |
| Stripe (custom) | Flat-rate negotiated | $5,625 | $2,813 | $0 | **~$8,440** |
| Stax | Subscription + I/C | $4,450 | $2,288 | $1,188 | **~$7,925** |
| Helcim | Interchange-plus | $4,900 | $2,538 | $0 | **~$7,440** |

*Interchange estimated at 1.7% CP, 2.1% CNP (typical blended rate for consumer credit/debit mix).*

---

## Key Considerations

### Internet at Terminals Changes the Picture

The original Square QR → POS app design (see `docs/08-payments.md`) was built around kiosks having no internet. With internet-capable terminals:

- **Square Terminal API** becomes viable — direct hardware integration, automated payment-to-order linking, no staff phone needed
- **Stripe Terminal** becomes viable — reader connects via internet, best API integration
- **Helcim Smart Terminal / readers** work — connected card readers with their API
- The QR workflow is still a good fallback for venues where internet is unreliable

### Split Provider Strategy

One option: use different providers for each channel to optimize both.

- **Card-present:** Helcim or Stax (interchange-plus, lowest fees for the bulk of volume)
- **Online:** Stripe (best API, best Elixir library, easiest to build checkout UX)

Trade-off: two integrations to maintain, two dashboards for reconciliation. But optimizes cost on the high-volume CP side while getting the best developer experience for the web integration.

### What to Do Next

1. **Get quotes** from Helcim and Stax with your actual volume numbers
2. **Call Square sales** to get custom rates at $300K+ — this is the lowest-effort path since you've already designed around Square
3. **Evaluate Stripe Terminal** for the kiosk workflow — if it fits, it gives you the best single-provider experience for both CP and CNP
4. Decide: single provider (simpler) vs. split (cheaper)

---

## Factors Beyond Price

| Factor | Square | Stripe | Helcim | Stax |
|--------|--------|--------|--------|------|
| API quality | Good | Excellent | Good | Adequate |
| Elixir library | Community | Mature (`stripity_stripe`) | None (HTTP wrapper) | None |
| Card-present hardware | Excellent | Good | Good | Adequate |
| Offline/fallback capability | QR → phone | Limited | Limited | Limited |
| Single CP+CNP integration | Yes | Yes | Yes | Yes |
| Dashboard/reporting | Good | Excellent | Good | Good |
| Payout speed | 1-2 business days | 2 business days | 2 business days | 2 business days |
| Contract/lock-in | None | None | None | Monthly sub |
