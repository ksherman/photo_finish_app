# On-Site Digital Delivery & Connectivity

**Status:** Future / Design
**Date:** 2026-03-29

---

## Context

Currently, on-site sales are USB-only. This doc explores adding digital delivery as an option at the event, the connectivity infrastructure needed to make photos browsable online during the meet, and how the pricing model should work.

---

## Delivery Model & Pricing

### Packages

| Package | Price | What they get |
|---------|-------|---------------|
| **Digital** | $75 | Download link (24-48 hrs) + permanent online access code |
| **USB** | $85 | USB drive at the event + online access code |
| **USB + Digital** | $95 | USB now + immediate download link + online access |

Every purchase includes an **access code** (see `docs/future/online-gallery.md`) — this is the bridge to post-event reordering and season accumulation. Even USB buyers get online access.

### Why Digital Is Cheaper

- No physical media cost (~$3-5/drive saved)
- No labor for loading, labeling, handing off USB drives
- Fulfillment is automated

### Why USB Still Has Value

- **Instant gratification** — walk away with your photos today
- **No internet dependency** — works regardless of venue connectivity
- **Tangible product** — parents feel they "got something"
- **No tech friction** — no "check your email" or download steps

USB should remain available even once digital is mature. Some parents will always prefer it.

---

## Connectivity Strategy

### The Bandwidth Requirement

During a meet (~100K photos, 8-10 hours):

| Content | Size each | Upload rate needed |
|---------|-----------|-------------------|
| Thumbnails (320px) | ~50KB | ~500 MB/hr |
| Watermarked previews (1280px) | ~300KB | ~3 GB/hr |
| **Total for online browsing** | | **~3.5 GB/hr** |

Originals (5-8MB each, ~60GB/hr) are NOT uploaded during the event. They ship home on the NAS/DAS and upload overnight.

### Recommended: Starlink Portable

| Factor | Starlink (Roam/Portable) | 5G Hotspot |
|--------|--------------------------|------------|
| Upload speed | 10-25 Mbps typical | 10-50 Mbps, highly variable |
| Reliability | Consistent — dedicated satellite link | Degrades with crowd congestion |
| At a packed meet | Unaffected by 500 phones in the building | Local cell tower may be saturated |
| Setup | Needs clear sky view (set up outside) | Pocket-sized, works anywhere |
| Cost | ~$599 hardware + $165/mo | ~$50-80/mo |

**Recommendation:** Starlink as primary, 5G hotspot as backup.

At 10 Mbps upload (Starlink low end), you get ~4.5 GB/hr — enough to keep up with the ~3.5 GB/hr of thumbnails + previews. At 20+ Mbps (typical), you have comfortable margin.

The $165/mo cost is trivial relative to $350K/season revenue. Reliable internet at events also enables:
- Live photo browsing on parents' phones
- Stripe/Square Terminal API for automated payment processing
- Real-time order notifications
- Future: live event dashboard, photographer monitoring

### 5G as Backup

Carry a 5G hotspot for venues where Starlink isn't practical (no outdoor access, indoor-only venue). Accept that it's less reliable during peak meet times. The system should handle intermittent connectivity gracefully — queue uploads and resume when connection returns.

---

## Upload Architecture

### During the Event (Real-Time)

```
Photo processed (thumbnail + preview generated)
    ↓
Upload queue (background, non-blocking)
    ↓ Starlink / 5G
Cloud Hot Storage (Cloudflare R2)
    ↓
Parent browses on phone → watermarked previews from R2 CDN
```

- Uploads are queued and streamed as photos are processed
- If connectivity drops, queue buffers locally and resumes
- Upload priority: thumbnails first (fast, enables grid browsing), then previews
- Non-blocking — local kiosk workflow works regardless of upload status

### After the Event (Batch)

```
DAS/NAS arrives home
    ↓ rclone sync (home internet, overnight)
Cloud Cold Storage (Backblaze B2) ← originals
    ↓
Digital order fulfillment: generate download links
    ↓
Parents receive email with download access
```

- Originals upload overnight on home internet (no venue bandwidth needed)
- Digital delivery fulfilled within 24-48 hours of event
- USB orders are fulfilled on-site, unaffected by cloud sync timing

---

## Watermarking & Content Protection

### The Concern

Parents screenshot previews instead of buying, especially once photos are browsable online during the event.

### The Reality

- 320px thumbnails: useless for anything — no protection needed
- 1280px previews: okay for social media, terrible for prints
- Parents who screenshot watermarked previews were not going to buy
- Parents who want quality photos for prints, books, gifts will pay
- Every major sports photography company shows watermarked previews and does fine

### The Approach

- **Grid view (thumbnails):** No watermark. Too small to steal, and clean thumbnails look better for browsing.
- **Preview view (1280px):** Semi-transparent watermark. Company logo, diagonal across the image. Tasteful but impossible to crop out.
- **Purchased files:** Clean originals, no watermark. Full resolution.

Watermark generation can happen during the existing image processing pipeline — when the PhotoProcessor Oban worker generates previews, it applies the watermark to the cloud-destined copy. The local preview (for kiosk use) can remain unwatermarked since kiosk access is already controlled.

### Watermark Design Principles

- Semi-transparent (30-40% opacity) — visible but doesn't ruin the photo
- Diagonal across the center — can't be cropped out
- Company name/logo — doubles as branding
- Applied to the preview derivative, never to the original

---

## How This Connects to Existing Plans

| Doc | Relationship |
|-----|-------------|
| `docs/future/online-gallery.md` | Access codes from purchases gate the online gallery. Digital delivery generates an access code. |
| `docs/future/payment-providers.md` | Digital delivery needs online payment (card-not-present). Internet at venues enables Terminal API. |
| `docs/08-payments.md` | Current QR → POS app workflow still works for USB sales. Digital sales could use same flow or direct Terminal API. |

---

## What This Enables

1. **During the event:** Parents browse photos on their phones while at the meet, order digital or USB at the kiosk
2. **Same day:** USB buyers walk away with photos. Digital buyers get an access code immediately, download link within hours.
3. **Post-event:** All buyers have online access codes. Can browse, reorder, share with family.
4. **Season:** Access codes accumulate across events (see online-gallery.md).

The progression from "USB only at the kiosk" to "browse anywhere, buy anytime" happens incrementally — each piece (connectivity, cloud upload, watermarking, digital delivery) can be built and deployed independently.
