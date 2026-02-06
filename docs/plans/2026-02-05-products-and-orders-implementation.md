# Products & Orders Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement USB-only ordering at kiosks — product catalog with per-event pricing, customer order capture, and staff admin management.

**Architecture:** New `PhotoFinish.Orders` Ash domain with four resources (ProductTemplate, EventProduct, Order, OrderItem). Kiosk flow added to the existing viewer. Admin order management added to the existing admin scope.

**Tech Stack:** Ash Framework resources, AshPostgres, AshPhoenix.Form, Phoenix LiveView, Tailwind CSS

---

### Task 1: Add ID generators for new entities

**Context:** `PhotoFinish.Id` already has `order_id/0`, `order_item_id/0`, and `product_id/0` generators. We need two more: `product_template_id` and `event_product_id`.

**Files:**
- Modify: `server/lib/photo_finish/id.ex`
- Modify: `server/test/photo_finish/id_test.exs` (if exists, otherwise create)

**Steps:**
1. Add `product_template_id/0` (prefix: `"ptm_"`) and `event_product_id/0` (prefix: `"evp_"`) functions
2. Test that they generate IDs with the correct prefix format
3. Commit

---

### Task 2: Create ProductTemplate Ash resource

**Context:** System-level product catalog. Not event-scoped. Holds default pricing. Follow the pattern in `events/event.ex` for resource structure.

**Files:**
- Create: `server/lib/photo_finish/orders/product_template.ex`
- Create: `server/test/photo_finish/orders/product_template_test.exs`

**Attributes:**
- `id` — string PK using `product_template_id/0`
- `product_type` — atom enum (`:usb`, `:print`, `:collage`, `:custom_photo`, `:accessory`)
- `product_name` — string, required
- `product_size` — string, nullable (null for USB/accessories)
- `default_price_cents` — integer, required
- `is_active` — boolean, default true
- `display_order` — integer, default 0
- timestamps

**Actions:** Default CRUD (create accepting all fields, update accepting all fields, read, destroy)

**Steps:**
1. Write tests for creating a product template with valid attrs, and for required field validation
2. Create the Ash resource following the Event resource pattern
3. Run tests, verify pass
4. Commit

---

### Task 3: Create EventProduct Ash resource

**Context:** Per-event price overrides. Belongs to an event and a product template. When an event is set up, active templates get copied here with the option to override pricing.

**Files:**
- Create: `server/lib/photo_finish/orders/event_product.ex`
- Create: `server/test/photo_finish/orders/event_product_test.exs`

**Attributes:**
- `id` — string PK using `event_product_id/0`
- `price_cents` — integer, required
- `is_available` — boolean, default true
- timestamps

**Relationships:**
- `belongs_to :event` (PhotoFinish.Events.Event)
- `belongs_to :product_template` (PhotoFinish.Orders.ProductTemplate)

**Identities:**
- Unique on `[:event_id, :product_template_id]`

**Actions:** Default CRUD

**Steps:**
1. Write tests for creating an event product linked to an event and template, and for the uniqueness constraint
2. Create the Ash resource
3. Run tests, verify pass
4. Commit

---

### Task 4: Create Order Ash resource

**Context:** One per customer transaction. Contains customer info, financial totals, payment tracking.

**Files:**
- Create: `server/lib/photo_finish/orders/order.ex`
- Create: `server/test/photo_finish/orders/order_test.exs`

**Attributes:**
- `id` — string PK using `order_id/0`
- `order_number` — string, required (generated at creation, unique)
- `customer_name` — string, required
- `customer_email` — string, nullable
- `customer_phone` — string, nullable
- `subtotal_cents` — integer, required
- `tax_rate_basis_points` — integer, required
- `tax_cents` — integer, required
- `total_cents` — integer, required
- `payment_status` — atom enum (`:pending`, `:paid`, `:refunded`), default `:pending`
- `payment_reference` — string, nullable
- `notes` — string, nullable
- timestamps

**Relationships:**
- `belongs_to :event` (PhotoFinish.Events.Event)
- `has_many :order_items` (PhotoFinish.Orders.OrderItem)

**Steps:**
1. Write tests for order creation with valid attrs and required field validation
2. Create the Ash resource
3. Run tests, verify pass
4. Commit

---

### Task 5: Create OrderItem Ash resource

**Context:** Line items on an order. For USB vertical slice, each item represents one USB for one competitor.

**Files:**
- Create: `server/lib/photo_finish/orders/order_item.ex`
- Create: `server/test/photo_finish/orders/order_item_test.exs`

**Attributes:**
- `id` — string PK using `order_item_id/0`
- `quantity` — integer, default 1
- `unit_price_cents` — integer, required
- `line_total_cents` — integer, required
- `fulfillment_status` — atom enum (`:pending`, `:fulfilled`), default `:pending`
- timestamps

**Relationships:**
- `belongs_to :order` (PhotoFinish.Orders.Order)
- `belongs_to :event_product` (PhotoFinish.Orders.EventProduct)
- `belongs_to :event_competitor` (PhotoFinish.Events.EventCompetitor)

**Steps:**
1. Write tests for order item creation with relationships
2. Create the Ash resource
3. Run tests, verify pass
4. Commit

---

### Task 6: Create Orders domain and generate migration

**Context:** Wire up all four resources under a `PhotoFinish.Orders` Ash domain. Add the domain to the application config. Generate and run the Ash migration.

**Files:**
- Create: `server/lib/photo_finish/orders.ex` (domain module)
- Modify: `server/config/config.exs` (add domain to ash config)
- Generated: migration file via `mix ash_postgres.generate_migrations`

**Steps:**
1. Create the Orders domain module listing all four resources
2. Register the domain in the Ash config (follow pattern of existing domains)
3. Run `mix ash_postgres.generate_migrations` to generate the migration
4. Run `mix ecto.migrate`
5. Run full test suite to verify nothing is broken
6. Commit

---

### Task 7: Add `next_order_number` to Event resource

**Context:** Events need a counter for generating sequential order numbers. Add the field to the existing Event Ash resource.

**Files:**
- Modify: `server/lib/photo_finish/events/event.ex`
- Modify or create test for the new field

**Steps:**
1. Add `next_order_number` integer attribute with default 0 to the Event resource
2. Generate migration for the schema change
3. Run migration
4. Test that new events get a default `next_order_number` of 0
5. Commit

---

### Task 8: Implement order number generation

**Context:** When an order is created, generate the order number by atomically incrementing the event's `next_order_number` and formatting as `"{order_code}-{NNNN}"`. This should be a helper module or a custom Ash change.

**Files:**
- Create: `server/lib/photo_finish/orders/order_number.ex` (or similar)
- Create: test file for order number logic
- Modify: `server/lib/photo_finish/orders/order.ex` (wire in the generation on create)

**Steps:**
1. Write tests for order number formatting (padding, prefix)
2. Implement the generation logic — atomic increment of `events.next_order_number` within the order creation
3. Test that creating an order produces a correctly formatted order number
4. Test that sequential orders get incrementing numbers
5. Commit

---

### Task 9: Implement order creation logic

**Context:** A custom action on Order (or a module in the Orders domain) that handles the full order creation flow: calculates totals from line items, snapshots the tax rate from the event, generates the order number, and creates order items in one transaction.

**Files:**
- Modify: `server/lib/photo_finish/orders/order.ex` (custom create action)
- Modify: `server/lib/photo_finish/orders.ex` (code interface)
- Create: integration test for order placement

**Steps:**
1. Write an integration test: given an event with products and a competitor, place an order and verify totals, order number, and items
2. Implement the custom create action or domain function
3. Run tests, verify pass
4. Commit

---

### Task 10: Seed default product templates

**Context:** Create a seeds file or migration that inserts the default product catalog (USB at $100, prints at standard prices). This gives admins a starting point.

**Files:**
- Modify: `server/priv/repo/seeds.exs` (or create a separate seeds file)

**Steps:**
1. Add seed data for the default product templates (USB, 4 print sizes)
2. Test that seeds run without error
3. Commit

---

### Task 11: Admin product management LiveView

**Context:** Admin UI to manage the default product catalog (CRUD for ProductTemplate). Follow existing admin LiveView patterns (Index + Form). Scoped under `/admin/products`.

**Files:**
- Create: `server/lib/photo_finish_web/live/admin/product_live/index.ex`
- Create: `server/lib/photo_finish_web/live/admin/product_live/form.ex`
- Modify: `server/lib/photo_finish_web/router.ex` (add routes)

**Steps:**
1. Add routes for product management under the admin scope
2. Build the index LiveView — list all product templates with name, type, size, default price, active status
3. Build the form LiveView — create/edit a product template using AshPhoenix.Form
4. Manual test in browser
5. Commit

---

### Task 12: Event product setup on event show page

**Context:** On the admin event show/edit page, add a section for managing that event's products. When an event doesn't have event_products yet, provide a "Initialize Products" action that copies active templates. Then allow per-event price overrides.

**Files:**
- Modify: `server/lib/photo_finish_web/live/admin/event_live/show.ex`
- Create helper or component for the products section
- Create: test for the initialization logic

**Steps:**
1. Add a domain function to initialize event products from active templates (copy default prices)
2. Test: initializing products for an event creates EventProduct rows with default prices
3. Add a products section to the event show page — shows event products with editable prices
4. Add "Initialize Products" button that only appears when the event has no products yet
5. Manual test in browser
6. Commit

---

### Task 13: Viewer order flow — "Order Photos" entry point

**Context:** Add an "Order Photos" button to the competitor viewer page (`ViewerLive.Competitor`). This navigates to a new order flow. For the vertical slice, this goes directly to a simple USB order page.

**Files:**
- Modify: `server/lib/photo_finish_web/live/viewer_live/competitor.ex` (add button)
- Create: `server/lib/photo_finish_web/live/viewer_live/order.ex` (order flow LiveView)
- Modify: `server/lib/photo_finish_web/router.ex` (add viewer order route)

**Steps:**
1. Add route: `live "/competitor/:id/order", ViewerLive.Order, :new` under the viewer scope
2. Add "Order Photos" button to the competitor page header
3. Create the order LiveView — mounts with the event_competitor, loads the USB event_product and its price, shows a simple order screen
4. Manual test in browser
5. Commit

---

### Task 14: Viewer order flow — checkout and confirmation

**Context:** The order LiveView from Task 13 needs a checkout form (customer name, optional email/phone) and a confirmation screen showing the order number.

**Files:**
- Modify: `server/lib/photo_finish_web/live/viewer_live/order.ex`

**Steps:**
1. Add the checkout form section — customer name (required), email, phone fields
2. On submit: call the order creation logic from Task 9, display the order number on a confirmation screen
3. Add a "Start New Order" button on confirmation that navigates back to the viewer home
4. Test the full flow end-to-end manually
5. Commit

---

### Task 15: Admin order management LiveView

**Context:** Admin view to list, search, and manage orders for an event. Staff uses this to find orders by number, mark as paid, add payment reference, and mark items as fulfilled.

**Files:**
- Create: `server/lib/photo_finish_web/live/admin/order_live/index.ex`
- Create: `server/lib/photo_finish_web/live/admin/order_live/show.ex`
- Modify: `server/lib/photo_finish_web/router.ex` (add routes)

**Steps:**
1. Add routes: order index and show under admin scope, scoped to an event (`/admin/events/:event_id/orders`)
2. Build index LiveView — list orders for the event with order number, customer, total, payment status, fulfillment status. Add search/filter by order number.
3. Build show LiveView — order details with actions: mark paid (with payment reference input), add notes, mark individual items as fulfilled
4. Add link to orders from the event show page
5. Manual test in browser
6. Commit

---

### Task 16: Update documentation

**Context:** The existing `docs/02-data-model.md` has a single `products` table. Our design uses `ProductTemplate` + `EventProduct`. Update the docs to reflect the actual implementation.

**Files:**
- Modify: `docs/02-data-model.md`
- Modify: `docs/06-ordering.md` (if needed to reflect USB-only vertical slice scope)

**Steps:**
1. Update the data model doc to reflect ProductTemplate + EventProduct split
2. Update ordering doc if any flow details changed during implementation
3. Commit
