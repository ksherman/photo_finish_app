# PhotoFinish - Ordering System

**Version:** 1.0  
**Date:** November 29, 2025

---

## Overview

On-site photo sales integrated into the viewer interface. Primary revenue driver (~70% USB drives, ~30% prints).

---

## MVP Scope

**Included:**
- Shopping cart in viewer
- USB drive orders (all photos for selected competitors)
- Individual print orders (photo + size)
- Digital checkout form
- Order number generation
- Admin order dashboard
- Payment recording (manual)
- USB fulfillment (file list)
- Print fulfillment (batch download)

**Deferred:**
- Collage builder
- Custom text/borders
- Square POS integration
- Email receipts
- Print lab API
- Inventory tracking
- Online portal

---

## User Flow

### Customer (Viewing Station)

```
1. Browse photos for competitor
2. Add photos to cart (floating badge shows count)
3. Open cart → select products:
   - USB Drive: includes all photos for competitor(s)
   - Prints: select size per photo
4. Checkout form:
   - Customer name (required)
   - Phone, email (optional)
   - Athlete name/number (pre-filled)
5. Submit → receive order number (e.g., "STV-0793")
6. Take order number to payment desk
7. Pay via Square/cash/check
```

### Staff (Order Processing)

```
1. Customer pays at desk
2. Staff finds order by number
3. Click "Mark as Paid" → select payment method
4. Fulfill items:
   - USB: view file list, copy to drive, mark complete
   - Prints: batch download, send to lab, mark sent
5. Hand USB to customer or notify when prints ready
```

---

## Order States

### Payment Status (order-level)

| Status | Description |
|--------|-------------|
| `pending` | Awaiting payment |
| `paid` | Payment received |
| `refunded` | Payment returned |

### Fulfillment Status (item-level)

| Status | Description |
|--------|-------------|
| `pending` | Not started |
| `preparing` | In progress (copying/downloading) |
| `ready` | Ready for pickup/ship |
| `delivered` | Handed to customer or shipped |

### Accounting Flag

`needs_accounting_review: boolean` — set when order requires review (refund, discount, etc.)

---

## Order Number Format

```
{EVENT_CODE}-{SEQUENCE}

EVENT_CODE = First 3 letters of event name (uppercase)
SEQUENCE = 4-digit zero-padded auto-increment per event

Example: "STV-0793" (St. Valentines Meet, order #793)
```

```elixir
def generate_order_number(event) do
  next_seq = get_next_sequence(event.id)
  "#{event.order_code}-#{String.pad_leading(to_string(next_seq), 4, "0")}"
end
```

---

## Product Catalog

### Default Products

| Type | Size | Price |
|------|------|-------|
| USB Drive | - | $100.00 |
| Print | 5x7 | $18.00 |
| Print | 8x10 | $30.00 |
| Print | 11x14 | $40.00 |
| Print | 16x20 | $65.00 |

### Future Products (Deferred)

| Type | Size | Price |
|------|------|-------|
| 3 Photo Collage | 8x10 | $60.00 |
| 3 Photo Collage | 11x14 | $70.00 |
| Custom Photo | 8x10 | $55.00 |

---

## Tax Calculation

```elixir
def calculate_totals(line_items, tax_rate \\ 8.5) do
  subtotal = Enum.sum(Enum.map(line_items, & &1.line_total_cents))
  tax = round(subtotal * (tax_rate / 100))
  
  %{
    subtotal_cents: subtotal,
    tax_rate: tax_rate,
    tax_cents: tax,
    total_cents: subtotal + tax
  }
end
```

---

## USB Drive Orders

### What's Included

All photos for selected competitor(s):
- If order has competitor IDs [A, B], USB contains all photos for both
- Organized by competitor, then apparatus

### USB File Structure

```
USB_DRIVE/
├── 1022 Kevin S/
│   ├── Floor/
│   │   ├── IMG_8234.jpg
│   │   └── IMG_8235.jpg
│   ├── Beam/
│   │   └── IMG_8301.jpg
│   └── Bars/
│       └── IMG_8402.jpg
└── 2224 Sarah J/
    ├── Floor/
    │   └── IMG_8240.jpg
    └── Vault/
        └── IMG_8510.jpg
```

### Fulfillment Query

```elixir
def get_usb_photos(competitor_ids) do
  from(p in Photo,
    join: c in Competitor, on: p.competitor_id == c.id,
    join: n in HierarchyNode, on: p.node_id == n.id,
    where: c.id in ^competitor_ids,
    where: p.status in ["ready", "finalized"],
    select: %{
      competitor_name: c.display_name,
      competitor_number: c.competitor_number,
      apparatus: n.name,
      file_path: coalesce(p.current_path, p.ingestion_path),
      filename: p.filename
    },
    order_by: [c.competitor_number, n.display_order, p.filename]
  )
  |> Repo.all()
end
```

### Admin UI for USB Fulfillment

```
┌─────────────────────────────────────────────────────┐
│ Order STV-0793 - USB Drive                         │
├─────────────────────────────────────────────────────┤
│ Competitors: 1022 Kevin S, 2224 Sarah J            │
│ Total photos: 87                                    │
├─────────────────────────────────────────────────────┤
│ Files to copy:                                      │
│                                                     │
│ 1022 Kevin S/                                       │
│ ├── Floor/ (24 files)                              │
│ ├── Beam/ (31 files)                               │
│ └── Bars/ (12 files)                               │
│                                                     │
│ 2224 Sarah J/                                       │
│ ├── Floor/ (8 files)                               │
│ └── Vault/ (12 files)                              │
├─────────────────────────────────────────────────────┤
│ [Copy Script] [Download Manifest]                  │
│                                                     │
│ [Mark as Complete]                                  │
└─────────────────────────────────────────────────────┘
```

---

## Print Orders

### Fulfillment Workflow

1. Select print orders from dashboard (batch)
2. Download high-res originals
3. Generate CSV manifest
4. Upload to print lab website manually
5. Mark as "sent to lab"
6. When received, mark as "ready"

### Manifest CSV

```csv
order_number,customer_name,filename,size,quantity
STV-0793,Jane Doe,IMG_8234.jpg,8x10,1
STV-0793,Jane Doe,IMG_8301.jpg,5x7,2
STV-0794,Mike Smith,IMG_8402.jpg,11x14,1
```

### Batch Download

```elixir
def prepare_print_batch(order_item_ids) do
  items = from(oi in OrderItem,
    join: p in Photo, on: oi.photo_id == p.id,
    join: o in Order, on: oi.order_id == o.id,
    where: oi.id in ^order_item_ids,
    select: %{
      order_number: o.order_number,
      customer_name: o.customer_name,
      photo_path: coalesce(p.current_path, p.ingestion_path),
      filename: p.filename,
      size: oi.product_size,
      quantity: oi.quantity
    }
  )
  |> Repo.all()
  
  # Copy files to temp dir with order-prefixed names
  # Generate manifest CSV
  # Create zip archive
  # Return download path
end
```

---

## Shopping Cart (LiveView)

### State

```elixir
defmodule PhotoFinishWeb.CartLive do
  def mount(_params, _session, socket) do
    {:ok, assign(socket,
      cart_items: [],  # [{photo_id, product_type, size}]
      competitor_context: nil  # Current competitor being browsed
    )}
  end
end
```

### Add to Cart

```elixir
def handle_event("add_to_cart", %{"photo_id" => photo_id}, socket) do
  # Default to print, let user select size later
  item = %{photo_id: photo_id, product_type: "print", size: nil}
  {:noreply, update(socket, :cart_items, &[item | &1])}
end

def handle_event("add_usb", %{"competitor_id" => competitor_id}, socket) do
  item = %{competitor_id: competitor_id, product_type: "usb_drive"}
  {:noreply, update(socket, :cart_items, &[item | &1])}
end
```

### Cart Modal

```
┌─────────────────────────────────────────────────────┐
│ Shopping Cart (5 items)                    [×]     │
├─────────────────────────────────────────────────────┤
│                                                     │
│ USB Drive - 1022 Kevin S (all photos)   $100.00   │
│                                          [Remove]  │
│                                                     │
│ Print - IMG_8234.jpg                               │
│ Size: [5x7 ▼] [8x10] [11x14] [16x20]    $18.00   │
│                                          [Remove]  │
│                                                     │
│ Print - IMG_8301.jpg                               │
│ Size: [5x7] [8x10 ▼] [11x14] [16x20]    $30.00   │
│                                          [Remove]  │
│                                                     │
├─────────────────────────────────────────────────────┤
│ Subtotal:                               $148.00    │
│ Tax (8.5%):                              $12.58    │
│ Total:                                  $160.58    │
├─────────────────────────────────────────────────────┤
│ [Continue Shopping]              [Proceed to Checkout]│
└─────────────────────────────────────────────────────┘
```

---

## Checkout Form

```
┌─────────────────────────────────────────────────────┐
│ Checkout                                            │
├─────────────────────────────────────────────────────┤
│                                                     │
│ Your Name *                                         │
│ [_________________________________]                 │
│                                                     │
│ Phone                                               │
│ [_________________________________]                 │
│                                                     │
│ Email                                               │
│ [_________________________________]                 │
│                                                     │
│ Athlete Name                                        │
│ [Kevin Smith_______________________] (pre-filled)  │
│                                                     │
│ Athlete Number                                      │
│ [1022_____________________________] (pre-filled)  │
│                                                     │
│ Notes                                               │
│ [_________________________________]                 │
│                                                     │
├─────────────────────────────────────────────────────┤
│ Total: $160.58                                      │
│                                                     │
│ [← Back to Cart]              [Place Order]        │
└─────────────────────────────────────────────────────┘
```

---

## Order Confirmation

```
┌─────────────────────────────────────────────────────┐
│                    ✓ Order Placed!                  │
├─────────────────────────────────────────────────────┤
│                                                     │
│            Your Order Number:                       │
│                                                     │
│               STV-0793                              │
│                                                     │
│ Please take this number to the payment desk         │
│ to complete your order.                             │
│                                                     │
│ Total Due: $160.58                                  │
│                                                     │
├─────────────────────────────────────────────────────┤
│ [Print Receipt]            [Start New Order]       │
└─────────────────────────────────────────────────────┘
```

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Order entry time | 2 hours → 15 min (80% reduction) |
| Transcription errors | 0 (100% accuracy) |
| USB fulfillment | < 10 min per order |
| Customer checkout | < 3 min |
