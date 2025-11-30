# Ordering System - Quick Reference

## Overview
Digital ordering system integrated into viewer interface. Replaces manual paper order forms. Primary revenue driver for the business.

## Key Stats
- **70%** of orders: USB drives ($100 - all photos)
- **30%** of orders: Prints and specialty products
- **Most orders:** Placed during event at viewing stations
- **Tax rate:** 8.5% (fixed)

## Current Pain Points (To Solve)
- 2+ hours of manual data entry per event
- Handwriting interpretation errors
- Difficult order tracking
- Hard to find photos from written notes
- No real-time revenue visibility

## MVP Scope (Phase 1)

### Included
✅ Shopping cart in viewer  
✅ USB drive orders  
✅ Individual print orders (photo + size)  
✅ Digital checkout form  
✅ Order number generation  
✅ Admin order dashboard  
✅ Payment recording (manual)  
✅ USB fulfillment (file list)  
✅ Print fulfillment (download photos)  
✅ Tax calculation (8.5%)  

### Explicitly Deferred
❌ Collage builder (manual creation)  
❌ Custom photo text/borders (manual)  
❌ Square POS integration  
❌ Email receipts (manual)  
❌ Print lab API (manual send)  
❌ Inventory tracking  
❌ Post-event online portal  

## Database Tables

### orders
- Order number (e.g., "STV-0793")
- Customer info (name, phone, email)
- Athlete info (name, number)
- Financial (subtotal, tax, total)
- Status (pending_payment, paid, fulfilling, etc.)
- Payment details (method, reference, paid_at)

### order_items
- Product type (usb_drive, print, etc.)
- Product size (NULL for USB, or 5x7, 8x10, etc.)
- Photo reference (NULL for USB, UUID for prints)
- Pricing (unit price, quantity, line total)
- Fulfillment status per item

### products
- Product catalog with configurable pricing
- Can be event-specific or global defaults
- Includes USB drives, prints, collages, custom

## User Flow

### Customer (At Viewing Station)

```
1. Browse photos
   └→ Click on competitor/athlete

2. Add photos to cart
   └→ Click "Add to Cart" on each photo

3. View cart
   └→ Floating cart badge shows count
   └→ Click to open cart modal

4. Select products
   └→ USB Drive (all photos)
   └→ OR individual prints (choose size)

5. Checkout
   └→ Enter customer info
   └→ Enter athlete info (pre-filled)
   └→ Review total with tax

6. Submit order
   └→ Receive order number (e.g., "STV-0793")

7. Pay at desk
   └→ Take order number to payment desk
   └→ Pay via Square/cash/check
```

### Staff (Order Processing)

```
1. View orders dashboard
   └→ See all orders
   └→ Filter by status

2. Customer pays at desk
   └→ Staff clicks order
   └→ "Mark as Paid" → record payment method

3. Fulfill USB orders
   └→ View list of all photo files
   └→ Manually copy to USB drive
   └→ Mark as complete

4. Fulfill print orders
   └→ Batch download high-res photos
   └→ Download CSV manifest
   └→ Send to print lab
   └→ Mark as sent
```

## Order Number Format

```
Event Code + Sequential Number
Example: "STV-0793"

Event Code = First 3 letters of event name (uppercase)
Number = Auto-increment per event (4 digits, zero-padded)
```

## Product Catalog (Default Pricing)

| Product | Size | Price |
|---------|------|-------|
| USB Drive - All Photos | - | $100.00 |
| Print | 5x7 | $18.00 |
| Print | 8x10 | $30.00 |
| Print | 11x14 | $40.00 |
| Print | 16x20 | $65.00 |
| 3 Photo Collage | 8x10 | $60.00 |
| 3 Photo Collage | 11x14 | $70.00 |
| 3 Photo Collage | 16x20 | $100.00 |
| 3 Photo Collage | 24x30 | $140.00 |
| Custom Photo | 8x10 | $55.00 |
| Custom Photo | 11x14 | $62.00 |
| Custom Photo | 16x20 | $92.00 |
| Custom Photo | Poster 24x30 | $125.00 |

## Fulfillment Workflows

### USB Drive (MVP - Manual)
1. Admin views USB order item
2. System shows list of all photo file paths for athlete
3. Staff manually copies files to USB drive
4. Label USB: "Athlete Name #Number - Order#"
5. Mark as complete

**Future:** Auto-generate zip or copy script

### Prints (MVP - Manual)
1. Admin selects multiple print orders
2. System downloads high-res photos in batch
3. System generates CSV manifest:
   - Order number
   - Customer name
   - Photo filename
   - Print size
   - Quantity
4. Staff uploads photos + manifest to print lab website
5. Mark as sent to lab
6. When received, mark as ready to ship

**Future:** Print lab API integration (drop-ship)

## Success Metrics

### MVP Goals
- **80% reduction** in order entry time (2 hours → 15 min)
- **100% accuracy** (zero transcription errors)
- **<10 minutes** USB fulfillment per order
- **<3 minutes** customer checkout time

### Business Value
- Real-time revenue tracking
- Product sales analytics
- Customer email list capture
- Reduced labor costs

## Technical Implementation

### Tax Calculation
```
Subtotal = Sum of all line items
Tax = Subtotal × 0.085 (8.5%)
Total = Subtotal + Tax
```

### File Collection (USB Orders)
```
Query: Get all photos for competitor_id
Group by event (Floor, Beam, Bars, etc.)
Return file paths with metadata
```

### Photo Download (Print Orders)
```
For each order_item:
  - Get photo.file_path (original high-res)
  - Copy to temp directory
  - Rename: {order_number}_{filename}_{size}.jpg
Generate manifest CSV
Create zip archive
Provide download link
```

## Phase 2 Features (Future)

- Collage builder (in-app photo layout)
- Custom photo editor (text, borders)
- Square POS integration (auto-mark paid)
- Email receipts (auto-send on order)
- Print lab API (drop-ship directly)
- Inventory tracking (USB stock)
- Shipping address capture
- Discount codes
- Analytics dashboard

## Open Questions

1. Order number format confirmed?
   - Event code (3 letters) + number works?

2. Email receipts in MVP?
   - Or manual only?

3. USB variations?
   - Just one product (all photos)?
   - Or event-specific USBs (e.g., "USB - Floor Only $40")?

4. Collage workflow?
   - MVP: Select 3 photos, manual Photoshop creation?
   - Phase 2: In-app layout tool?
