# ShootProof Studio API Integration — Online Storefront & Lab Drop-Ship

**Status:** Future / Target v3 Direction (supersedes custom R2/B2 storefront in `online-gallery.md` & `on-site-digital-delivery.md`)
**Date:** 2026-10-20 (captured in repo 2026-11-22)

---

## Why ShootProof Instead of Building a Custom Web Store

Building and maintaining a custom cloud photo storefront (checkout, multi-state sales tax, print-lab fulfillment, digital download hosting, marketing emails, customer support) is non-core work for PhotoFinish.

- **ShootProof Unlimited:** **$600/year flat, 0% commission**, with a full Studio REST OpenAPI specification.
- **Replaces:** PhotoReflect (~15%–18% commission + double-shipping prints) and avoids high-commission volume SaaS platforms like PhotoDay/GotPhoto (7%–10% commission).
- **Architectural consequence:** PhotoFinish stays focused on what happens **at the venue** (SD ingest, local LAN viewing kiosks, roster matching, on-site Square checkout, fast USB-C/A fulfillment) plus an **automated ShootProof API uploader & order-pusher**.

---

## End-to-End Hybrid Workflow

```text
ON-SITE (Local Network — Zero Internet Required)
  SD Cards → Ingest App (Gym → Session → Rotation/Group → Apparatus)
           → Local Server & Roster Matcher
           → Local Viewing Kiosks (Search by Athlete / Bib #)
           → Digital Checkout:
               • Single Digital ($22–$25; 3+ triggers full-bundle upsell)
               • Full Athlete Digital ($65–$95, Monday email link + online access)
               • Prints (5x7, 8x10, 11x14, 16x20 — direct lab drop-ship)
               • Instant USB-C/A Add-On (+$10 upsell)
           → Square Payment:
               • Offline: QR code (square-commerce-v1://) → Staff cellular iPhone Tap-to-Pay
               • Online (Hotspot/Starlink): Square Terminal API (POST /v2/terminals/checkouts)

FULFILLMENT & TWO-SPEED SYNC
  • Instant USB buyers: Local copy to Dual USB-C/A 3.0 drive (~10s, ≥25–35 MB/s sustained write)
  • Priority Paid Digitals (optional): Trickle-upload paid athlete's ~100 photos (~500 MB) over 5G/Starlink
  • Full Meet Sync (Sunday night on Home Fiber):
      Automated folder pivot (Gym → Session → Group → Apparatus → Athlete
                              ⟹ Event → Gym → Athlete)
      + Parallel S3 upload (20–50 concurrent workers) via ShootProof API
```

**Operational SLA:** Promising digital delivery links on **Monday morning** guarantees concrete-walled school gyms with dead cell signal never break the customer promise, while +$10 instant USB-C/A drives serve parents who want files in hand immediately.

---

## Five ShootProof Studio API Integration Points

*(Endpoint structure validated against ShootProof's Studio OpenAPI spec.)*

| Step | ShootProof Studio API Endpoint | Role in PhotoFinish |
| --- | --- | --- |
| **1. Create Event & Nested Albums** | `POST /brand/{brandId}/event`<br>`POST /brand/{brandId}/event/{eventId}/album` | Creates the meet event and nested `Gym → Athlete` sub-albums (plus optional per-album PINs via `/album/passwords`). |
| **2. High-Speed S3 Photo Upload** | `POST /brand/{brandId}/event/{eventId}/photo/upload-policy`<br>`PATCH .../album/{eventAlbumId}/photo` | Pivots on-site apparatus folders into athlete-sorted albums and streams JPEGs directly to ShootProof S3 using 20–50 async workers. |
| **3. Automated Digital Zip Delivery** | `POST /brand/{brandId}/event/{eventId}/album/{eventAlbumId}/zip-bundle` | Generates and emails full-res digital download links for paid athlete packages (eliminates manual zipping). |
| **4. On-Site Print → Lab Drop-Ship** | `POST /brand/{brandId}/order`<br>`POST /brand/{brandId}/order/{orderId}/lab-shipment` | Pushes on-site kiosk print orders into ShootProof's WHCC / Bay Photo fulfillment queue so prints drop-ship directly to the family. |
| **5. Per-Athlete QR / Sorting** | `POST /brand/{brandId}/event/{eventId}/qr-code`<br>`GET /brand/{brandId}/event/{eventId}/volume-sort` | Generates native per-athlete QR codes / access links for kiosk receipts and post-meet marketing. |

---

## On-Site Pricing & Product Model Implications

PhotoFinish's kiosk cart and pricing model should support:

1. **Single Digital Photo ($22–$25):** Per-photo selection in the kiosk viewer, 100% automated email fulfillment via ShootProof.
2. **3-Photo Tipping-Point Upsell:** When a family adds 3 single digital photos for an athlete, automatically prompt to upgrade to the **Full Athlete Digital Package** for roughly the same price.
3. **Full Athlete Digital Package ($65–$95, meet-dependent):** All photos of that athlete across all apparatuses (Monday link + online gallery access).
4. **Instant Dual USB-C/A Add-On (+$10):** Paid add-on rather than free on every order; uses verified high-speed dual USB-C/USB-A 3.0 drives (e.g., 16 GB ≥25–35 MB/s sustained write or SanDisk Ultra Dual Drive Go) so copying ~500 MB takes ~6–15 seconds.
5. **Prints (5x7, 8x10, 11x14, 16x20):** Collect shipping address at kiosk checkout so orders route straight to WHCC / Bay Photo via ShootProof lab drop-ship (no double-shipping back to home office).

---

## Multi-Venue Season Scale to Design For

Based on the 2026 season audit (~850,000 uploaded photos across ~12 competition weekends / 17 galleries):

- **Peak single meet:** ~165,000 photos across multi-gym venues (6–9 photographers).
- **Peak concurrent weekend:** **4 separate meets running simultaneously across multiple states** (8–12 photographers, 4 independent local server boxes).
- **Key requirement:** Each meet box must run completely offline/standalone during the weekend and produce a **self-contained, portable event package** that can be gathered on Sunday night onto a home workstation/NAS for batch ShootProof upload, print lab submission, and consolidated revenue/rebate reporting.
