# PhotoFinish - Payment Processing

**Version:** 1.0
**Date:** February 5, 2026

---

## Overview

On-site payment collection at viewing kiosks using Square. Designed for local-network environments where the server and kiosks may not have internet access.

---

## Architecture Decision: QR Code + Square POS App

### Constraints

- Server and kiosks run on a **local network with no guaranteed internet**
- Up to 15 viewing stations at a single event
- Must support card-present transactions for lower processing fees
- Minimal hardware cost per station

### Chosen Approach

Kiosks generate a QR code at checkout. Staff scans it with an iPhone running the Square POS app, which has cellular internet. Payment is processed on the phone using Tap to Pay on iPhone (NFC — no additional hardware needed) or an attached Square Reader.

```
[iPad Kiosk - local network]          [Staff iPhone - cellular]
        |                                       |
  Family browses photos                         |
  Adds to cart, checks out                      |
  Screen shows QR code ---------> Staff scans QR code
    (amount + order ref)          Square POS opens pre-filled
        |                         Family taps card on iPhone (Tap to Pay)
        |                         Payment completes
  Staff marks order paid <------- Staff confirms on kiosk
```

### Why Not Other Approaches?

| Option | Rejected Because |
|--------|-----------------|
| Square Terminal + Terminal API | Requires internet on server; $299/terminal |
| Square Reader + native app on each kiosk | Requires building/deploying a native iOS app; Reader needs Bluetooth pairing per device |
| Web Payments SDK (card-not-present) | Higher fees (2.9% + 30c vs 2.6% + 15c); requires internet on kiosks |
| Manual entry at payment desk | Current approach; slow, error-prone at scale |

---

## Square POS API Integration

### How It Works

Square's [Point of Sale API](https://developer.squareup.com/docs/pos-api/how-it-works) allows a URL with the `square-commerce-v1://` scheme to open the Square POS app pre-populated with transaction details.

The kiosk encodes checkout information into a QR code. When scanned by an iPhone, it opens the Square POS app with:

- **Amount** (total including tax)
- **Note** (order number for reconciliation)
- **Supported tender types** (card, cash)

### QR Code Payload

The QR code contains a URL that opens Square POS:

```
square-commerce-v1://payment/create?data={percent-encoded JSON}
```

The JSON payload:

```json
{
  "amount_money": {
    "amount": 16058,
    "currency_code": "USD"
  },
  "callback_url": "photfinish://payment-complete",
  "client_id": "YOUR_SQUARE_APP_ID",
  "version": "1.3",
  "notes": "STV-0793 | Kevin Smith | 1022",
  "options": {
    "supported_tender_types": ["CREDIT_CARD", "CASH"]
  }
}
```

### Key Fields

| Field | Purpose |
|-------|---------|
| `amount_money.amount` | Total in cents (including tax) |
| `notes` | Order number + customer info for reconciliation |
| `supported_tender_types` | Card and/or cash |
| `callback_url` | Optional; not used if kiosk has no native app |

---

## Processing Rates

### Card-Present vs Card-Not-Present

| | Card-Present (in-person) | Card-Not-Present (online) |
|---|---|---|
| Interchange (network level) | 1.70% - 2.05% | 2.25% - 2.65% |
| Square rate (free plan) | 2.6% + 15c | 2.9% + 30c |
| Fraud rate | ~0.06% | ~0.93% |

On $50,000 in event sales, card-present saves roughly **$250-$500** in fees.

Tap to Pay on iPhone uses NFC and qualifies as **card-present** — same rate as a physical reader.

### Square Plan Pricing

| Plan | Monthly Fee | In-Person Rate | Online Rate |
|------|-------------|----------------|-------------|
| Free | $0 | 2.6% + 15c | 3.3% + 30c |
| Plus | $49/mo | 2.5% + 15c | 2.9% + 30c |
| Premium | $149/mo | 2.4% + 15c | 2.9% + 30c |

### Custom / Negotiated Rates

At **$300K+/year** in processing volume (our expected range), Square offers custom pricing. Contact their sales team to negotiate rates based on:

- Annual processing volume
- Average ticket size
- Business history

This could bring in-person rates below the standard tiers. Worth pursuing before the first event season on the new system.

---

## Hardware

| Item | Cost | Quantity | Notes |
|------|------|----------|-------|
| iPhone with Square POS app | (existing) | 1-2 | Staff phone with cellular data; Tap to Pay built in (no reader needed) |
| Square Reader (Bluetooth) | $49 | 0-1 | Optional backup; needed only if Tap to Pay is unreliable |
| iPad (viewing station) | ~$150 refurb | 15 | Runs Phoenix LiveView in Safari kiosk mode |

Total payment hardware cost: **$0-$49** using Tap to Pay on iPhone (vs $4,485+ for 15 Square Terminals).

---

## Order Reconciliation

Since the kiosk and Square operate independently (no real-time server-to-Square connection), reconciliation uses the order number stored in the Square transaction note.

### During the Event

1. Staff scans QR, processes payment on iPhone
2. Staff taps "Mark as Paid" on the kiosk (manual confirmation)
3. Order status updates to `paid` in the local database

### Post-Event

When internet is available, sync with Square's API to verify:

```elixir
# Fetch transactions from Square and match by note field
def reconcile_orders(event) do
  square_payments = SquareClient.list_payments(
    begin_time: event.start_date,
    end_time: event.end_date
  )

  for payment <- square_payments do
    order_number = parse_order_number(payment.note)
    # Match to local orders, flag discrepancies
  end
end
```

---

## Checkout UX Flow

### 1. Order Confirmation Screen (Kiosk)

After the customer submits their order, the kiosk displays:

```
+-----------------------------------------------------+
|                                                       |
|            Order STV-0793                             |
|                                                       |
|   Items:                                              |
|     USB Drive - 1022 Kevin S          $100.00         |
|     Print 8x10 - IMG_8234.jpg         $30.00         |
|                                                       |
|   Subtotal:                           $130.00         |
|   Tax (8.5%):                          $11.05         |
|   Total:                              $141.05         |
|                                                       |
|          [QR CODE]                                    |
|                                                       |
|   Show this QR code to staff to pay                   |
|                                                       |
|   [Cancel Order]            [I've Paid - Done]        |
+-----------------------------------------------------+
```

### 2. Staff Scans QR Code (iPhone)

- iPhone camera reads QR code
- Square POS app opens with amount ($141.05) and note (STV-0793)
- Customer taps card on iPhone (Tap to Pay) or Square Reader
- Payment completes

### 3. Staff Confirms on Kiosk

- Staff taps "Mark as Paid" or customer taps "I've Paid - Done"
- Kiosk returns to browse mode

---

## Future Enhancements

- **Terminal API integration**: If internet becomes reliable at venues, use Square Terminal API for automated payment-to-order linking
- **Payment link fallback**: Generate Square Payment Links for customers who want to pay on their own phone
- **Receipt delivery**: Email/SMS receipts using Square's built-in receipt system
- **Multi-reader support**: Multiple staff iPhones processing payments in parallel during peak times
