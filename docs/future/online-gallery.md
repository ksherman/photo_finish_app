# Online Photo Gallery — Future Feature Design

**Status:** Future / Design
**Date:** 2026-03-29

---

## Problem

PhotoFinish is currently optimized for event-day sales at kiosks. ~70-80% of revenue happens on-site. But there's a significant opportunity in post-event online ordering — parents who didn't buy at the event, want additional prints, or want to revisit photos later.

The current system has three gaps that prevent this:

1. **No online access path.** Photos are served from a local NAS on a local network. There's no way for parents to view or order photos after the event.

2. **Event-location-centric organization.** Photos are organized by Gym → Session → Group → Apparatus. This works for admin workflows and kiosk browsing, but online the natural view is "show me all of my kid's photos."

3. **No cross-event competitor identity.** Each roster import creates a fresh Competitor record. The same gymnast at 5 different meets has 5 disconnected identities. There's no way to build a season gallery.

---

## Design: Code-Based Photo Access

### Principles

- **Safety first.** Photos are of minors. No public profiles, no searchable galleries, no way for a stranger to find a specific child's photos.
- **No account friction.** Parents shouldn't need to create an account to view their photos.
- **Code = access.** Physical possession of an access code (from a receipt or card) is the authentication mechanism.

### Access Code Generation

When a parent purchases photos at the kiosk, their order receipt includes a unique **access code**.

- **Format:** Short, human-readable alphanumeric (e.g., `STV-7K3X` or similar — event prefix + random)
- **Scope:** Each code unlocks one `EventCompetitor`'s full photo gallery for that event
- **Printed on:** Receipt (text + QR code)
- **Stored as:** `AccessCode` record linked to `EventCompetitor` and the originating `Order`

### Online Flow

```
Parent visits site → enters access code → sees competitor's full event gallery
                                        → can browse all photos (not just purchased)
                                        → can add new photos to cart and checkout
```

1. Parent visits `yoursite.com/photos` (or scans QR from receipt)
2. Enters access code
3. Sees their competitor's gallery for that event — organized by apparatus/session
4. Can browse, select, and purchase additional photos
5. Digital download or print fulfillment (future: define fulfillment method)

### Multi-Code Linking (Season View)

Parents who attend multiple meets accumulate codes. The system allows linking:

1. Parent enters a code → sees one event's photos
2. "Add another event" → enters a second code → combined view
3. Linked codes stored in browser session (or optional lightweight server session)
4. Combined view groups photos by event, then by apparatus

This creates the **season gallery experience** without requiring:
- Account creation
- Public competitor profiles
- Cross-event identity resolution in the database

The parent's collection of codes IS their identity.

---

## Design: Competitor-Centric Organization (Online)

### Event Day (Unchanged)

The kiosk experience stays location-centric:
- Search by competitor number → see photos → order
- Admin uses Gym/Session/Group/Apparatus hierarchy for organization
- Folder association workflow unchanged

### Online (New)

The online experience is person-centric:
- Access code resolves to a specific competitor at a specific event
- Gallery shows all of that competitor's photos, organized by apparatus
- Multi-code linking adds a cross-event dimension

### Data Model Considerations

**Current state:**
- `Competitor` records are created fresh on every roster import (no dedup)
- Same person at different events = different `Competitor` records
- `external_id` field exists on `Competitor` but is never used

**For code-based access (MVP):**
- New `AccessCode` resource: `code`, `event_competitor_id`, `order_id`, `expires_at`
- No changes to Competitor dedup needed — access is per-EventCompetitor, not per-person
- Multi-code linking is client-side aggregation, not a database join

**For future cross-event analytics (optional):**
- Implement `external_id` dedup during roster import (USAG numbers, gym member IDs)
- Or fuzzy match on name + team for suggested merges
- This is a nice-to-have for business intelligence, not a product requirement

---

## Future Considerations

### Universal Browse Codes

Currently: access codes are purchase-gated (you buy at the event to get a code).

Future option: every competitor gets a browse code (printed on a card, distributed by the gym). This code shows watermarked previews — purchasing removes watermarks and unlocks downloads. This dramatically increases the online conversion funnel but adds distribution complexity.

### Infrastructure: Tiered Storage with NAS ↔ Cloud Sync

Event-day photos are served from direct attached storage on a local network. Post-event, they need to be internet-accessible for online ordering.

**Architecture: NAS as working primary, cloud as sync target and fulfillment source.**

```
Event Day:
  Camera cards → Direct Attached Storage → local Phoenix server (kiosks)

Post-Event:
  DAS → Home NAS (working copy, admin's primary)
       ↓ (background sync via rclone or NAS cloud sync)
  Cloud Hot (Cloudflare R2):  thumbnails + watermarked previews  (~35GB/meet)
  Cloud Cold (Backblaze B2):  originals                          (~700GB/meet)

Online Browsing:
  Parent enters access code → browses thumbnails/previews from R2 CDN

Online Order Fulfillment:
  Parent orders → system pulls original from B2 → generates deliverable
  (Home NAS does NOT need to be online for fulfillment)

Local Work:
  Admin needs to reprocess/edit/QC → works directly from NAS
```

**Why this split:**
- Thumbnails (320px, ~50KB) and previews (1280px, ~300KB) are all that's needed for browsing/shopping
- Originals (5-8MB) are only needed when a photo is actually ordered
- Home NAS is the working copy for local tasks; cloud is the fulfillment source and backup
- Home internet outage doesn't block online sales

**Estimated costs (20-meet season, ~100K photos/meet):**

| Tier | Service | Storage | Monthly cost |
|------|---------|---------|-------------|
| Hot (browse) | Cloudflare R2 | ~700GB | ~$10 |
| Cold (originals) | Backblaze B2 | ~14TB | ~$84 |
| **Total** | | | **~$94/month** |

B2 retrieval on order is negligible (~$0.01/GB, a single original is fractions of a penny).

**Sync tooling:**
- `rclone` (free, supports B2/R2/S3/Glacier) as a post-event batch job or cron
- Synology/QNAP built-in cloud sync apps as an alternative
- Thumbnail/preview upload can be automated as part of the post-event processing pipeline

### Season Passes & Bundle Pricing

With multi-code linking, season packages become possible:
- "Buy all 4 meets for $X" (discount vs. individual)
- "Digital season pass" — all photos from all meets for a flat rate
- Requires pricing model design beyond per-photo/per-event

### Re-Engagement

If parents optionally provide an email when using their access code:
- "12 new photos from last weekend's meet" notification
- Seasonal summary emails
- Must be opt-in and clearly scoped

---

## What This Does NOT Change

- **Ingestion workflow:** Tauri app, card readers, folder copy — all unchanged
- **Folder organization:** Gym/Session/Group/Apparatus hierarchy on NAS — unchanged
- **Folder association:** Admin mapping folders to competitors — unchanged
- **Kiosk experience:** Search by number, browse, order at the event — unchanged
- **Photo processing:** Oban workers, thumbnail/preview generation — unchanged

This feature is additive — it creates a new online access path on top of the existing system.
