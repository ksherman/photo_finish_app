# PhotoFinish - API Reference

**Version:** 1.0  
**Date:** November 29, 2025

---

## Overview

HTTP API for Tauri app ↔ Phoenix communication. All endpoints under `/api/`.

---

## Authentication

None for MVP (trusted local network). Future: API key header.

---

## Endpoints

### Events

#### GET /api/events

List all events.

**Response:**
```json
{
  "data": [
    {
      "id": "uuid",
      "name": "St. Valentines Meet",
      "slug": "st-valentines-meet",
      "event_date": "2025-02-14",
      "status": "active"
    }
  ]
}
```

#### GET /api/events/:id

Get event details.

**Response:**
```json
{
  "data": {
    "id": "uuid",
    "name": "St. Valentines Meet",
    "slug": "st-valentines-meet",
    "event_date": "2025-02-14",
    "status": "active",
    "order_code": "STV",
    "tax_rate": 8.5,
    "num_gyms": 2,
    "sessions_per_gym": 4
  }
}
```

---

### Roster

#### GET /api/events/:event_id/roster

Get competitor roster for Tauri app folder renaming.

**Query params:**
- `search` — Filter by name/number (optional)

**Response:**
```json
{
  "data": [
    {
      "id": "uuid",
      "competitor_number": "1022",
      "display_name": "1022 Kevin S",
      "first_name": "Kevin",
      "last_name": "Smith",
      "team_name": "Bay Area Gymnastics"
    }
  ]
}
```

---

### Ingestion

#### POST /api/ingestion/notify

Notify Phoenix that files have been copied to NAS.

**Request:**
```json
{
  "event_id": "uuid",
  "photographer": "KDS",
  "envelope_code": "Group 1A/Beam",
  "order_number": "0001",
  "destination_path": "/originals/st-valentines-meet/kds/gym-a/session-3/0001",
  "file_count": 45
}
```

**Response:**
```json
{
  "status": "ok",
  "message": "Ingestion queued",
  "job_id": "uuid"
}
```

#### POST /api/ingestion/rename

Report folder renames from Tauri app.

**Request:**
```json
{
  "event_id": "uuid",
  "base_path": "/originals/st-valentines-meet/kds/gym-a/session-3/0001",
  "renames": [
    {"original": "EOS100", "new": "1022 Kevin S", "photo_count": 24},
    {"original": "EOS101", "new": "1023 Sarah J", "photo_count": 31}
  ]
}
```

**Response:**
```json
{
  "status": "ok",
  "competitors_linked": 2
}
```

#### GET /api/ingestion/status

Get current ingestion pipeline status.

**Response:**
```json
{
  "data": {
    "queue_depth": 12,
    "processing": 4,
    "completed_today": 1456,
    "errors_today": 3,
    "last_activity": "2025-02-14T12:34:56Z",
    "watcher_active": true
  }
}
```

---

### Photos

#### GET /api/events/:event_id/photos

List photos with filters.

**Query params:**
- `gym` — Filter by gym
- `session` — Filter by session
- `apparatus` — Filter by apparatus
- `event_competitor_id` — Filter by competitor
- `status` — Filter by status (ready, processing, error)
- `page`, `per_page` — Pagination

**Response:**
```json
{
  "data": [
    {
      "id": "uuid",
      "filename": "IMG_8234.jpg",
      "thumbnail_url": "/thumbnails/st-valentines-meet/uuid.jpg",
      "preview_url": "/previews/st-valentines-meet/uuid.jpg",
      "status": "ready",
      "captured_at": "2025-02-14T10:23:45Z"
    }
  ],
  "meta": {
    "total": 1456,
    "page": 1,
    "per_page": 50
  }
}
```

#### GET /api/photos/:id

Get photo details.

**Response:**
```json
{
  "data": {
    "id": "uuid",
    "filename": "IMG_8234.jpg",
    "original_filename": "IMG_8234.jpg",
    "ingestion_path": "/originals/...",
    "current_path": "/originals/...",
    "thumbnail_url": "...",
    "preview_url": "...",
    "width": 6000,
    "height": 4000,
    "file_size_bytes": 3145728,
    "status": "ready",
    "photographer": "KDS",
    "captured_at": "2025-02-14T10:23:45Z",
    "exif_data": {...}
  }
}
```

---

### Orders

#### POST /api/events/:event_id/orders

Create new order (from viewer checkout).

**Request:**
```json
{
  "customer_name": "Jane Doe",
  "customer_email": "jane@example.com",
  "customer_phone": "555-1234",
  "items": [
    {
      "product_type": "usb_drive",
      "competitor_ids": ["uuid1", "uuid2"]
    },
    {
      "product_type": "print",
      "photo_id": "uuid",
      "size": "8x10",
      "quantity": 1
    }
  ]
}
```

**Response:**
```json
{
  "data": {
    "id": "uuid",
    "order_number": "STV-0793",
    "subtotal_cents": 14800,
    "tax_cents": 1258,
    "total_cents": 16058,
    "payment_status": "pending"
  }
}
```

#### GET /api/events/:event_id/orders

List orders.

**Query params:**
- `payment_status` — pending, paid, refunded
- `search` — Order number or customer name
- `page`, `per_page`

**Response:**
```json
{
  "data": [
    {
      "id": "uuid",
      "order_number": "STV-0793",
      "customer_name": "Jane Doe",
      "total_cents": 16058,
      "payment_status": "pending",
      "created_at": "2025-02-14T12:34:56Z"
    }
  ]
}
```

#### GET /api/orders/:id

Get order details.

**Response:**
```json
{
  "data": {
    "id": "uuid",
    "order_number": "STV-0793",
    "customer_name": "Jane Doe",
    "customer_email": "jane@example.com",
    "subtotal_cents": 14800,
    "tax_rate": 8.5,
    "tax_cents": 1258,
    "total_cents": 16058,
    "payment_status": "pending",
    "items": [
      {
        "id": "uuid",
        "product_type": "usb_drive",
        "product_name": "USB Drive - All Photos",
        "competitor_ids": ["uuid1", "uuid2"],
        "unit_price_cents": 10000,
        "quantity": 1,
        "line_total_cents": 10000,
        "fulfillment_status": "pending"
      },
      {
        "id": "uuid",
        "product_type": "print",
        "product_name": "8x10 Print",
        "photo_id": "uuid",
        "size": "8x10",
        "unit_price_cents": 3000,
        "quantity": 1,
        "line_total_cents": 3000,
        "fulfillment_status": "pending"
      }
    ]
  }
}
```

#### PATCH /api/orders/:id/payment

Record payment.

**Request:**
```json
{
  "payment_method": "cash",
  "payment_reference": null
}
```

**Response:**
```json
{
  "data": {
    "id": "uuid",
    "payment_status": "paid",
    "paid_at": "2025-02-14T12:45:00Z"
  }
}
```

#### PATCH /api/order_items/:id/fulfillment

Update item fulfillment status.

**Request:**
```json
{
  "fulfillment_status": "ready"
}
```

---

### Products

#### GET /api/events/:event_id/products

Get product catalog for event.

**Response:**
```json
{
  "data": [
    {
      "id": "uuid",
      "product_type": "usb_drive",
      "product_name": "USB Drive - All Photos",
      "product_size": null,
      "price_cents": 10000,
      "is_active": true
    },
    {
      "id": "uuid",
      "product_type": "print",
      "product_name": "8x10 Print",
      "product_size": "8x10",
      "price_cents": 3000,
      "is_active": true
    }
  ]
}
```

---

## WebSocket (Phoenix Channels)

### photos:competitor:{competitor_id}

Subscribe to photo updates for a competitor.

### admin:updates

Admin-only channel for system updates.

**Events:**
- `ingestion_progress` — Processing queue status
- `new_order` — Order placed
- `order_updated` — Payment/fulfillment change

---

## Error Responses

```json
{
  "error": {
    "code": "not_found",
    "message": "Order not found"
  }
}
```

**Codes:**
- `not_found` — 404
- `validation_error` — 422
- `server_error` — 500
