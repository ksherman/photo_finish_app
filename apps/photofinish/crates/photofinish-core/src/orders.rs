use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::id;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductRecord {
    pub id: String,
    pub product_type: String, // single_digital | full_digital | usb_addon | print | custom
    pub name: String,
    pub size: Option<String>,
    pub price_cents: i64,
    pub is_available: bool,
    pub display_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShippingAddress {
    pub street: String,
    pub street2: Option<String>,
    pub city: String,
    pub state: String,
    pub zip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderItemInput {
    pub product_id: String,
    pub subject_id: Option<String>,
    pub photo_id: Option<String>,
    #[serde(default = "default_qty")]
    pub quantity: i64,
}

fn default_qty() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderRequest {
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub shipping_address: Option<ShippingAddress>,
    pub notes: Option<String>,
    pub items: Vec<CreateOrderItemInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderItemRecord {
    pub id: String,
    pub order_id: String,
    pub product_id: Option<String>,
    pub subject_id: Option<String>,
    pub subject_display_name: Option<String>,
    pub photo_id: Option<String>,
    pub photo_filename: Option<String>,
    pub product_type: String,
    pub product_name: String,
    pub product_size: Option<String>,
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
    pub fulfillment_status: String,
    pub fulfilled_at: Option<String>,
}

/// Flat S&H fee for print orders whose largest print is up to 8×10 ($3.00, untaxed).
pub const SMALL_PRINT_SHIPPING_CENTS: i64 = 300;
/// Flat S&H fee for print orders containing any print 11×14 or larger ($10.00, untaxed).
pub const LARGE_PRINT_SHIPPING_CENTS: i64 = 1000;

/// Extracts the largest integer edge (in inches) from a print size or name string
/// such as `"5x7"`, `"8x10"`, `"11×14"`, or `"16x20 Poster Print"`.
pub fn parse_max_print_dimension_inches(text: &str) -> Option<u32> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut best: Option<u32> = None;

    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start1 = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let first: u32 = match chars[start1..i].iter().collect::<String>().parse() {
                Ok(n) => n,
                Err(_) => continue,
            };

            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < chars.len() && matches!(chars[j], 'x' | 'X' | '×') {
                j += 1;
                while j < chars.len() && chars[j].is_ascii_whitespace() {
                    j += 1;
                }
                let start2 = j;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j > start2 {
                    if let Ok(second) = chars[start2..j].iter().collect::<String>().parse::<u32>() {
                        let max_edge = first.max(second);
                        best = Some(best.map_or(max_edge, |prev| prev.max(max_edge)));
                        i = j;
                        continue;
                    }
                }
            }
        } else {
            i += 1;
        }
    }

    best
}

/// Calculates flat untaxed print shipping & handling for a list of products in an order:
/// - No print items → `$0.00` (`0` cents)
/// - Any print `11×14` or larger (max edge ≥ 11") → `$10.00` (`1000` cents)
/// - Print orders with all prints ≤ `8×10` → `$3.00` (`300` cents)
pub fn calculate_print_shipping_cents<'a, I>(products: I) -> i64
where
    I: IntoIterator<Item = &'a ProductRecord>,
{
    let mut has_print = false;
    let mut has_large_print = false;

    for prod in products {
        if prod.product_type != "print" {
            continue;
        }
        has_print = true;
        let max_dim = prod
            .size
            .as_deref()
            .and_then(parse_max_print_dimension_inches)
            .or_else(|| parse_max_print_dimension_inches(&prod.name));
        if max_dim.is_some_and(|d| d >= 11) {
            has_large_print = true;
        }
    }

    if !has_print {
        0
    } else if has_large_print {
        LARGE_PRINT_SHIPPING_CENTS
    } else {
        SMALL_PRINT_SHIPPING_CENTS
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderStats {
    pub total_orders: i64,
    pub pending_payment_count: i64,
    pub paid_count: i64,
    pub refunded_count: i64,
    pub pending_usb_count: i64,
    pub pending_fulfillment_count: i64,
    pub gross_paid_cents: i64,
    pub subtotal_paid_cents: i64,
    pub shipping_paid_cents: i64,
    pub tax_paid_cents: i64,
    pub pending_revenue_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderListResponse {
    pub stats: OrderStats,
    pub orders: Vec<OrderRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePaymentRequest {
    pub order_id: String,
    pub payment_status: String, // pending | paid | refunded
    pub payment_method: Option<String>, // square_qr | square_terminal | cash | check | other
    pub payment_reference: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductUpsertInput {
    pub id: Option<String>,
    pub product_type: String,
    pub name: String,
    pub size: Option<String>,
    pub price_cents: i64,
    pub is_available: bool,
    pub display_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventOrderSettingsInput {
    pub order_code: String,
    pub station_code: String,
    pub tax_rate_basis_points: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbSubjectBreakdown {
    pub subject_label: String,
    pub photo_count: usize,
    pub total_bytes: u64,
    pub sections: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbOrderPlan {
    pub order_id: String,
    pub order_number: String,
    pub total_photos: usize,
    pub total_bytes: u64,
    pub subjects: Vec<UsbSubjectBreakdown>,
    pub missing_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRecord {
    pub id: String,
    pub order_number: String,
    pub station_code: String,
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub shipping_address: Option<ShippingAddress>,
    pub subtotal_cents: i64,
    pub shipping_cents: i64,
    pub tax_rate_basis_points: i64,
    pub tax_cents: i64,
    pub total_cents: i64,
    pub payment_status: String,
    pub payment_method: Option<String>,
    pub payment_reference: Option<String>,
    pub paid_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub items: Vec<OrderItemRecord>,
    /// Offline-safe Square POS URL (`square-commerce-v1://payment/create?data=...`)
    /// encoded into a QR code on the kiosk confirmation screen.
    pub square_pos_url: String,
}

pub fn list_products(conn: &Connection, include_unavailable: bool) -> Result<Vec<ProductRecord>> {
    let sql = if include_unavailable {
        "SELECT id, product_type, name, size, price_cents, is_available, display_order
         FROM products ORDER BY display_order ASC, name ASC"
    } else {
        "SELECT id, product_type, name, size, price_cents, is_available, display_order
         FROM products WHERE is_available = 1 ORDER BY display_order ASC, name ASC"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok(ProductRecord {
            id: r.get(0)?,
            product_type: r.get(1)?,
            name: r.get(2)?,
            size: r.get(3)?,
            price_cents: r.get(4)?,
            is_available: r.get::<_, i64>(5)? != 0,
            display_order: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn place_order(conn: &mut Connection, req: CreateOrderRequest) -> Result<OrderRecord> {
    let customer_name = req.customer_name.trim().to_string();
    if customer_name.is_empty() {
        return Err(Error::InvalidInput("Customer name is required".into()));
    }
    if req.items.is_empty() {
        return Err(Error::InvalidInput("Order must contain at least one item".into()));
    }

    let customer_email = req
        .customer_email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let customer_phone = req
        .customer_phone
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let notes = req
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    // Load authoritative product catalog
    let catalog: HashMap<String, ProductRecord> = list_products(&tx, false)?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();

    let mut has_print = false;
    let mut resolved_items = Vec::with_capacity(req.items.len());
    let mut subtotal_cents: i64 = 0;

    for item in &req.items {
        let qty = item.quantity.max(1);
        let prod = catalog.get(&item.product_id).ok_or_else(|| {
            Error::InvalidInput(format!("Product '{}' is unavailable or invalid", item.product_id))
        })?;
        if prod.product_type == "print" {
            has_print = true;
        }
        let line_total = prod.price_cents * qty;
        subtotal_cents += line_total;
        resolved_items.push((prod.clone(), item.clone(), qty, line_total));
    }

    if has_print {
        let valid_addr = req.shipping_address.as_ref().map(|a| {
            !a.street.trim().is_empty()
                && !a.city.trim().is_empty()
                && !a.state.trim().is_empty()
                && !a.zip.trim().is_empty()
        });
        if valid_addr != Some(true) {
            return Err(Error::InvalidInput(
                "Shipping address (street, city, state, zip) is required for print orders".into(),
            ));
        }
    }

    // Atomically increment order number and read event order settings
    tx.execute(
        "UPDATE event_config SET next_order_number = next_order_number + 1",
        [],
    )?;
    let (order_code, station_code, next_num, tax_rate_bps): (String, String, i64, i64) = tx
        .query_row(
            "SELECT order_code, station_code, next_order_number, tax_rate_basis_points
             FROM event_config LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;

    let order_number = format!("{}-{}{:04}", order_code, station_code, next_num);
    let shipping_cents =
        calculate_print_shipping_cents(resolved_items.iter().map(|(prod, _, _, _)| prod));
    // Sales tax applies strictly to product subtotal — shipping & handling is NOT taxed.
    let tax_cents = ((subtotal_cents as f64) * (tax_rate_bps as f64) / 10_000.0).round() as i64;
    let total_cents = subtotal_cents + shipping_cents + tax_cents;

    let order_id = id::order_id();
    let now = Utc::now().to_rfc3339();
    let shipping_json = match &req.shipping_address {
        Some(addr) => Some(serde_json::to_string(addr)?),
        None => None,
    };

    tx.execute(
        "INSERT INTO orders (
            id, order_number, station_code, customer_name, customer_email, customer_phone,
            shipping_address_json, subtotal_cents, shipping_cents, tax_rate_basis_points, tax_cents,
            total_cents, payment_status, notes, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'pending', ?13, ?14, ?15)",
        rusqlite::params![
            order_id,
            order_number,
            station_code,
            customer_name,
            customer_email,
            customer_phone,
            shipping_json,
            subtotal_cents,
            shipping_cents,
            tax_rate_bps,
            tax_cents,
            total_cents,
            notes,
            now,
            now,
        ],
    )?;

    for (prod, input, qty, line_total) in resolved_items {
        let item_id = id::order_item_id();
        tx.execute(
            "INSERT INTO order_items (
                id, order_id, product_id, subject_id, photo_id,
                product_type, product_name, product_size,
                quantity, unit_price_cents, line_total_cents,
                fulfillment_status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'pending', ?12, ?13)",
            rusqlite::params![
                item_id,
                order_id,
                prod.id,
                input.subject_id,
                input.photo_id,
                prod.product_type,
                prod.name,
                prod.size,
                qty,
                prod.price_cents,
                line_total,
                now,
                now,
            ],
        )?;
    }

    tx.commit()?;

    get_order_by_number(conn, &order_number)?
        .ok_or_else(|| Error::NotFound(format!("Order {} not found after creation", order_number)))
}

pub fn get_order_by_number(conn: &Connection, order_number: &str) -> Result<Option<OrderRecord>> {
    let row = conn
        .query_row(
            "SELECT id, order_number, station_code, customer_name, customer_email, customer_phone,
                    shipping_address_json, subtotal_cents, shipping_cents, tax_rate_basis_points,
                    tax_cents, total_cents, payment_status, payment_method, payment_reference,
                    paid_at, notes, created_at
             FROM orders WHERE order_number = ?1",
            [order_number],
            extract_order_row,
        )
        .optional()?;

    match row {
        Some(raw) => Ok(Some(hydrate_order(conn, raw)?)),
        None => Ok(None),
    }
}

pub fn get_order_by_id(conn: &Connection, order_id: &str) -> Result<Option<OrderRecord>> {
    let row = conn
        .query_row(
            "SELECT id, order_number, station_code, customer_name, customer_email, customer_phone,
                    shipping_address_json, subtotal_cents, shipping_cents, tax_rate_basis_points,
                    tax_cents, total_cents, payment_status, payment_method, payment_reference,
                    paid_at, notes, created_at
             FROM orders WHERE id = ?1",
            [order_id],
            extract_order_row,
        )
        .optional()?;

    match row {
        Some(raw) => Ok(Some(hydrate_order(conn, raw)?)),
        None => Ok(None),
    }
}

type RawOrderTuple = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
    i64,
    i64,
    i64,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

fn extract_order_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawOrderTuple> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
        r.get(12)?,
        r.get(13)?,
        r.get(14)?,
        r.get(15)?,
        r.get(16)?,
        r.get(17)?,
    ))
}

fn load_items_for_order(conn: &Connection, order_id: &str) -> Result<Vec<OrderItemRecord>> {
    let mut stmt = conn.prepare(
        "SELECT oi.id, oi.order_id, oi.product_id, oi.subject_id, s.display_name,
                oi.photo_id, p.filename, oi.product_type, oi.product_name, oi.product_size,
                oi.quantity, oi.unit_price_cents, oi.line_total_cents, oi.fulfillment_status,
                oi.fulfilled_at
         FROM order_items oi
         LEFT JOIN subjects s ON s.id = oi.subject_id
         LEFT JOIN photos p ON p.id = oi.photo_id
         WHERE oi.order_id = ?1
         ORDER BY oi.created_at ASC",
    )?;
    let items: Vec<OrderItemRecord> = stmt
        .query_map([order_id], |r| {
            Ok(OrderItemRecord {
                id: r.get(0)?,
                order_id: r.get(1)?,
                product_id: r.get(2)?,
                subject_id: r.get(3)?,
                subject_display_name: r.get(4)?,
                photo_id: r.get(5)?,
                photo_filename: r.get(6)?,
                product_type: r.get(7)?,
                product_name: r.get(8)?,
                product_size: r.get(9)?,
                quantity: r.get(10)?,
                unit_price_cents: r.get(11)?,
                line_total_cents: r.get(12)?,
                fulfillment_status: r.get(13)?,
                fulfilled_at: r.get(14)?,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(items)
}

fn hydrate_order(conn: &Connection, raw: RawOrderTuple) -> Result<OrderRecord> {
    let (
        id,
        order_number,
        station_code,
        customer_name,
        customer_email,
        customer_phone,
        shipping_json,
        subtotal_cents,
        shipping_cents,
        tax_rate_basis_points,
        tax_cents,
        total_cents,
        payment_status,
        payment_method,
        payment_reference,
        paid_at,
        notes,
        created_at,
    ) = raw;

    let items = load_items_for_order(conn, &id)?;
    let shipping_address: Option<ShippingAddress> = match shipping_json {
        Some(j) => serde_json::from_str(&j).ok(),
        None => None,
    };
    let square_pos_url = build_square_pos_url(&order_number, &customer_name, total_cents, &items);

    Ok(OrderRecord {
        id,
        order_number,
        station_code,
        customer_name,
        customer_email,
        customer_phone,
        shipping_address,
        subtotal_cents,
        shipping_cents,
        tax_rate_basis_points,
        tax_cents,
        total_cents,
        payment_status,
        payment_method,
        payment_reference,
        paid_at,
        notes,
        created_at,
        items,
        square_pos_url,
    })
}

pub fn list_orders(conn: &Connection) -> Result<OrderListResponse> {
    let mut stmt = conn.prepare(
        "SELECT id, order_number, station_code, customer_name, customer_email, customer_phone,
                shipping_address_json, subtotal_cents, shipping_cents, tax_rate_basis_points,
                tax_cents, total_cents, payment_status, payment_method, payment_reference,
                paid_at, notes, created_at
         FROM orders
         ORDER BY created_at DESC",
    )?;
    let raw_orders: Vec<RawOrderTuple> = stmt
        .query_map([], extract_order_row)?
        .collect::<std::result::Result<_, _>>()?;

    let mut stats = OrderStats::default();
    let mut orders = Vec::with_capacity(raw_orders.len());

    for raw in raw_orders {
        let ord = hydrate_order(conn, raw)?;
        stats.total_orders += 1;
        match ord.payment_status.as_str() {
            "paid" => {
                stats.paid_count += 1;
                stats.gross_paid_cents += ord.total_cents;
                stats.subtotal_paid_cents += ord.subtotal_cents;
                stats.shipping_paid_cents += ord.shipping_cents;
                stats.tax_paid_cents += ord.tax_cents;
            }
            "refunded" => {
                stats.refunded_count += 1;
            }
            _ => {
                stats.pending_payment_count += 1;
                stats.pending_revenue_cents += ord.total_cents;
            }
        }
        if ord.payment_status != "refunded" {
            let has_pending_usb = ord
                .items
                .iter()
                .any(|i| i.product_type == "usb_addon" && i.fulfillment_status != "fulfilled");
            if has_pending_usb {
                stats.pending_usb_count += 1;
            }
            let has_pending_item = ord
                .items
                .iter()
                .any(|i| i.fulfillment_status != "fulfilled");
            if has_pending_item {
                stats.pending_fulfillment_count += 1;
            }
        }
        orders.push(ord);
    }

    Ok(OrderListResponse { stats, orders })
}

pub fn update_order_payment(
    conn: &mut Connection,
    req: &UpdatePaymentRequest,
) -> Result<OrderRecord> {
    let status = req.payment_status.trim();
    if !matches!(status, "pending" | "paid" | "refunded") {
        return Err(Error::InvalidInput(format!(
            "Invalid payment status '{}'",
            status
        )));
    }
    let method = req
        .payment_method
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let reference = req
        .payment_reference
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let now = Utc::now().to_rfc3339();

    let existing_paid_at: Option<Option<String>> = conn
        .query_row(
            "SELECT paid_at FROM orders WHERE id = ?1",
            [&req.order_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(prev_paid_at) = existing_paid_at else {
        return Err(Error::NotFound(format!("Order '{}' not found", req.order_id)));
    };

    let paid_at: Option<String> = match status {
        "paid" => Some(prev_paid_at.unwrap_or_else(|| now.clone())),
        "refunded" => prev_paid_at,
        _ => None,
    };

    if let Some(ref notes) = req.notes {
        let clean_notes = notes.trim();
        let notes_opt = if clean_notes.is_empty() {
            None
        } else {
            Some(clean_notes)
        };
        conn.execute(
            "UPDATE orders
             SET payment_status = ?2, payment_method = ?3, payment_reference = ?4,
                 paid_at = ?5, notes = ?6, updated_at = ?7
             WHERE id = ?1",
            rusqlite::params![
                req.order_id,
                status,
                method,
                reference,
                paid_at,
                notes_opt,
                now
            ],
        )?;
    } else {
        conn.execute(
            "UPDATE orders
             SET payment_status = ?2, payment_method = ?3, payment_reference = ?4,
                 paid_at = ?5, updated_at = ?6
             WHERE id = ?1",
            rusqlite::params![req.order_id, status, method, reference, paid_at, now],
        )?;
    }

    get_order_by_id(conn, &req.order_id)?
        .ok_or_else(|| Error::NotFound(format!("Order '{}' not found", req.order_id)))
}

pub fn update_order_notes(
    conn: &mut Connection,
    order_id: &str,
    notes: Option<&str>,
) -> Result<OrderRecord> {
    let clean = notes.map(str::trim).filter(|s| !s.is_empty());
    let now = Utc::now().to_rfc3339();
    let changed = conn.execute(
        "UPDATE orders SET notes = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![order_id, clean, now],
    )?;
    if changed == 0 {
        return Err(Error::NotFound(format!("Order '{}' not found", order_id)));
    }
    get_order_by_id(conn, order_id)?
        .ok_or_else(|| Error::NotFound(format!("Order '{}' not found", order_id)))
}

pub fn update_order_item_fulfillment(
    conn: &mut Connection,
    order_id: &str,
    item_id: &str,
    fulfillment_status: &str,
) -> Result<OrderRecord> {
    if !matches!(fulfillment_status, "pending" | "fulfilled") {
        return Err(Error::InvalidInput(format!(
            "Invalid fulfillment status '{}'",
            fulfillment_status
        )));
    }
    let now = Utc::now().to_rfc3339();
    let fulfilled_at: Option<&str> = if fulfillment_status == "fulfilled" {
        Some(now.as_str())
    } else {
        None
    };
    let changed = conn.execute(
        "UPDATE order_items
         SET fulfillment_status = ?3, fulfilled_at = ?4, updated_at = ?5
         WHERE id = ?1 AND order_id = ?2",
        rusqlite::params![item_id, order_id, fulfillment_status, fulfilled_at, now],
    )?;
    if changed == 0 {
        return Err(Error::NotFound(format!("Order item '{}' not found", item_id)));
    }
    get_order_by_id(conn, order_id)?
        .ok_or_else(|| Error::NotFound(format!("Order '{}' not found", order_id)))
}

pub fn mark_usb_items_fulfilled(conn: &mut Connection, order_id: &str) -> Result<usize> {
    let now = Utc::now().to_rfc3339();
    let changed = conn.execute(
        "UPDATE order_items
         SET fulfillment_status = 'fulfilled', fulfilled_at = ?2, updated_at = ?2
         WHERE order_id = ?1 AND product_type = 'usb_addon' AND fulfillment_status != 'fulfilled'",
        rusqlite::params![order_id, now],
    )?;
    Ok(changed)
}

pub fn upsert_product(conn: &mut Connection, input: &ProductUpsertInput) -> Result<ProductRecord> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(Error::InvalidInput("Product name is required".into()));
    }
    if input.price_cents < 0 {
        return Err(Error::InvalidInput("Price cannot be negative".into()));
    }
    let ptype = input.product_type.trim();
    if !matches!(
        ptype,
        "single_digital" | "full_digital" | "usb_addon" | "print" | "custom"
    ) {
        return Err(Error::InvalidInput(format!(
            "Invalid product type '{}'",
            ptype
        )));
    }
    let size = input
        .size
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let avail: i64 = if input.is_available { 1 } else { 0 };
    let now = Utc::now().to_rfc3339();

    let prod_id = if let Some(ref existing_id) = input.id {
        let changed = conn.execute(
            "UPDATE products
             SET product_type = ?2, name = ?3, size = ?4, price_cents = ?5,
                 is_available = ?6, display_order = ?7, updated_at = ?8
             WHERE id = ?1",
            rusqlite::params![
                existing_id,
                ptype,
                name,
                size,
                input.price_cents,
                avail,
                input.display_order,
                now
            ],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(format!("Product '{}' not found", existing_id)));
        }
        existing_id.clone()
    } else {
        let new_id = id::product_id();
        conn.execute(
            "INSERT INTO products (
                id, product_type, name, size, price_cents, is_available,
                display_order, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                new_id,
                ptype,
                name,
                size,
                input.price_cents,
                avail,
                input.display_order,
                now,
                now
            ],
        )?;
        new_id
    };

    Ok(ProductRecord {
        id: prod_id,
        product_type: ptype.to_string(),
        name: name.to_string(),
        size: size.map(str::to_string),
        price_cents: input.price_cents,
        is_available: input.is_available,
        display_order: input.display_order,
    })
}

pub fn update_event_order_settings(
    conn: &mut Connection,
    input: &EventOrderSettingsInput,
) -> Result<()> {
    let order_code = input.order_code.trim().to_uppercase();
    let station_code = input.station_code.trim().to_uppercase();
    if order_code.is_empty() || order_code.len() > 8 {
        return Err(Error::InvalidInput(
            "Order code must be 1–8 characters".into(),
        ));
    }
    if station_code.is_empty() || station_code.len() > 4 {
        return Err(Error::InvalidInput(
            "Station code must be 1–4 characters".into(),
        ));
    }
    if !(0..=2500).contains(&input.tax_rate_basis_points) {
        return Err(Error::InvalidInput(
            "Tax rate must be between 0% and 25%".into(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE event_config
         SET order_code = ?1, station_code = ?2, tax_rate_basis_points = ?3, updated_at = ?4",
        rusqlite::params![order_code, station_code, input.tax_rate_basis_points, now],
    )?;
    Ok(())
}

/// Formats one or more `(event_name, orders)` groups into a clean RFC-4180 CSV string
/// suitable for bookkeeping, lab fulfillment, and multi-venue season consolidation.
pub fn format_orders_csv(event_orders: &[(String, Vec<OrderRecord>)]) -> String {
    let mut out = String::from(
        "event_name,order_number,station_code,created_at,payment_status,payment_method,payment_reference,paid_at,customer_name,customer_email,customer_phone,shipping_address,item_type,item_name,item_size,subject,photo_filename,quantity,unit_price,line_total,fulfillment_status,order_subtotal,order_shipping,order_tax,order_total,notes\n",
    );

    for (event_name, orders) in event_orders {
        for ord in orders {
            let addr = ord
                .shipping_address
                .as_ref()
                .map(|a| {
                    let s2 = a
                        .street2
                        .as_deref()
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| format!(" {}", s.trim()))
                        .unwrap_or_default();
                    format!(
                        "{}{}, {}, {} {}",
                        a.street.trim(),
                        s2,
                        a.city.trim(),
                        a.state.trim(),
                        a.zip.trim()
                    )
                })
                .unwrap_or_default();

            for item in &ord.items {
                let cols: Vec<String> = vec![
                    event_name.clone(),
                    ord.order_number.clone(),
                    ord.station_code.clone(),
                    ord.created_at.clone(),
                    ord.payment_status.clone(),
                    ord.payment_method.clone().unwrap_or_default(),
                    ord.payment_reference.clone().unwrap_or_default(),
                    ord.paid_at.clone().unwrap_or_default(),
                    ord.customer_name.clone(),
                    ord.customer_email.clone().unwrap_or_default(),
                    ord.customer_phone.clone().unwrap_or_default(),
                    addr.clone(),
                    item.product_type.clone(),
                    item.product_name.clone(),
                    item.product_size.clone().unwrap_or_default(),
                    item.subject_display_name.clone().unwrap_or_default(),
                    item.photo_filename.clone().unwrap_or_default(),
                    item.quantity.to_string(),
                    format_dollars(item.unit_price_cents),
                    format_dollars(item.line_total_cents),
                    item.fulfillment_status.clone(),
                    format_dollars(ord.subtotal_cents),
                    format_dollars(ord.shipping_cents),
                    format_dollars(ord.tax_cents),
                    format_dollars(ord.total_cents),
                    ord.notes.clone().unwrap_or_default(),
                ];
                let line: Vec<String> = cols.into_iter().map(csv_escape).collect();
                out.push_str(&line.join(","));
                out.push('\n');
            }
        }
    }

    out
}

fn format_dollars(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{}{}.{:02}", sign, abs / 100, abs % 100)
}

fn csv_escape(s: String) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

/// Builds an offline-safe Square Point of Sale API deep-link URL (`square-commerce-v1://`)
/// so a staff iPhone scanning the kiosk QR code opens Square POS pre-filled with the exact
/// total and reconciliation note.
pub fn build_square_pos_url(
    order_number: &str,
    customer_name: &str,
    total_cents: i64,
    items: &[OrderItemRecord],
) -> String {
    let mut subjects: Vec<&str> = items
        .iter()
        .filter_map(|i| i.subject_display_name.as_deref())
        .collect();
    subjects.sort_unstable();
    subjects.dedup();

    let note = if subjects.is_empty() {
        format!("{} | {}", order_number, customer_name)
    } else {
        format!("{} | {} | {}", order_number, customer_name, subjects.join(", "))
    };

    let payload = serde_json::json!({
        "amount_money": {
            "amount": total_cents,
            "currency_code": "USD"
        },
        "version": "1.3",
        "notes": note,
        "options": {
            "supported_tender_types": ["CREDIT_CARD", "CASH"]
        }
    });

    let payload_str = payload.to_string();
    let encoded = urlencoding::encode(&payload_str);
    format!("square-commerce-v1://payment/create?data={}", encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{CreateEventParams, EventBundle};
    use tempfile::tempdir;

    fn sample_product(product_type: &str, name: &str, size: Option<&str>) -> ProductRecord {
        ProductRecord {
            id: "prd_test".into(),
            product_type: product_type.into(),
            name: name.into(),
            size: size.map(str::to_string),
            price_cents: 1000,
            is_available: true,
            display_order: 0,
        }
    }

    #[test]
    fn print_shipping_tiers_match_order_form_rules() {
        assert_eq!(parse_max_print_dimension_inches("5x7"), Some(7));
        assert_eq!(parse_max_print_dimension_inches("8x10"), Some(10));
        assert_eq!(parse_max_print_dimension_inches("11×14"), Some(14));
        assert_eq!(parse_max_print_dimension_inches("16x20 Poster Print"), Some(20));
        assert_eq!(parse_max_print_dimension_inches("24 x 30"), Some(30));
        assert_eq!(parse_max_print_dimension_inches("Digital Package"), None);

        // Digital / USB only -> $0
        let digital = sample_product("full_digital", "All Photos Digital", None);
        let usb = sample_product("usb_addon", "Instant Dual USB", None);
        assert_eq!(calculate_print_shipping_cents([&digital, &usb]), 0);

        // Prints up to 8x10 -> $3.00 flat
        let p57 = sample_product("print", "5x7 Lustre Print", Some("5x7"));
        let p810 = sample_product("print", "8x10 Lustre Print", Some("8x10"));
        assert_eq!(calculate_print_shipping_cents([&p57]), 300);
        assert_eq!(calculate_print_shipping_cents([&p57, &p810, &digital]), 300);

        // Any print 11x14 or larger -> $10.00 flat
        let p1114 = sample_product("print", "11x14 Print", Some("11x14"));
        let p1620 = sample_product("print", "16x20 Poster Print", Some("16x20"));
        let p2430 = sample_product("print", "24×30 Collage", None);
        assert_eq!(calculate_print_shipping_cents([&p1114]), 1000);
        assert_eq!(calculate_print_shipping_cents([&p57, &p1620]), 1000);
        assert_eq!(calculate_print_shipping_cents([&p2430]), 1000);

        // Unsized custom print defaults to small tier ($3.00)
        let custom_print = sample_product("print", "Custom Wallet Set", None);
        assert_eq!(calculate_print_shipping_cents([&custom_print]), 300);
    }

    #[test]
    fn print_orders_require_mailing_address_and_keep_shipping_untaxed() {
        let dir = tempdir().unwrap();
        let bundle = EventBundle::create(CreateEventParams {
            root_path: dir.path().join("shipping-meet"),
            name: "Shipping Test Meet".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("SHP".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: Some(1000), // 10% makes tax math crystal clear
            scaffold: None,
        })
        .unwrap();

        let products = bundle.list_products(false).unwrap();
        let p57 = products
            .iter()
            .find(|p| p.product_type == "print" && p.size.as_deref() == Some("5x7"))
            .unwrap();
        let p1114 = products
            .iter()
            .find(|p| p.product_type == "print" && p.size.as_deref() == Some("11x14"))
            .unwrap();

        // Missing shipping address must fail for print orders
        let missing_addr_err = bundle
            .place_order(CreateOrderRequest {
                customer_name: "Parent No Address".into(),
                customer_email: None,
                customer_phone: None,
                shipping_address: None,
                notes: None,
                items: vec![CreateOrderItemInput {
                    product_id: p57.id.clone(),
                    subject_id: None,
                    photo_id: None,
                    quantity: 1,
                }],
            })
            .unwrap_err();
        assert!(missing_addr_err.to_string().contains("Shipping address"));

        let addr = ShippingAddress {
            street: "965 W Highland Ave".into(),
            street2: None,
            city: "Elgin".into(),
            state: "IL".into(),
            zip: "60123".into(),
        };

        // Small print order: 2x 5x7 ($36.00) + $3.00 untaxed S&H + $3.60 tax (10% of $36 only) = $42.60
        let small_order = bundle
            .place_order(CreateOrderRequest {
                customer_name: "Small Print Parent".into(),
                customer_email: Some("small@example.com".into()),
                customer_phone: None,
                shipping_address: Some(addr.clone()),
                notes: None,
                items: vec![CreateOrderItemInput {
                    product_id: p57.id.clone(),
                    subject_id: None,
                    photo_id: None,
                    quantity: 2,
                }],
            })
            .unwrap();
        assert_eq!(small_order.subtotal_cents, 3600);
        assert_eq!(small_order.shipping_cents, 300);
        assert_eq!(small_order.tax_cents, 360); // strictly 10% of 3600, NOT 3900
        assert_eq!(small_order.total_cents, 4260);

        // Large print order: 5x7 ($18) + 11x14 ($40) = $58.00 + $10.00 untaxed S&H + $5.80 tax = $73.80
        let large_order = bundle
            .place_order(CreateOrderRequest {
                customer_name: "Large Print Parent".into(),
                customer_email: Some("large@example.com".into()),
                customer_phone: None,
                shipping_address: Some(addr),
                notes: None,
                items: vec![
                    CreateOrderItemInput {
                        product_id: p57.id.clone(),
                        subject_id: None,
                        photo_id: None,
                        quantity: 1,
                    },
                    CreateOrderItemInput {
                        product_id: p1114.id.clone(),
                        subject_id: None,
                        photo_id: None,
                        quantity: 1,
                    },
                ],
            })
            .unwrap();
        assert_eq!(large_order.subtotal_cents, 5800);
        assert_eq!(large_order.shipping_cents, 1000);
        assert_eq!(large_order.tax_cents, 580); // strictly 10% of 5800, NOT 6800
        assert_eq!(large_order.total_cents, 7380);
        // Square POS URL must encode the exact total including untaxed shipping
        assert!(large_order.square_pos_url.contains("7380"));

        // Mark both paid and verify OrderStats reconciles subtotal + shipping + tax == gross
        bundle
            .update_order_payment(&UpdatePaymentRequest {
                order_id: small_order.id.clone(),
                payment_status: "paid".into(),
                payment_method: Some("square_qr".into()),
                payment_reference: None,
                notes: None,
            })
            .unwrap();
        bundle
            .update_order_payment(&UpdatePaymentRequest {
                order_id: large_order.id.clone(),
                payment_status: "paid".into(),
                payment_method: Some("square_qr".into()),
                payment_reference: None,
                notes: None,
            })
            .unwrap();

        let listed = bundle.list_orders().unwrap();
        assert_eq!(listed.stats.subtotal_paid_cents, 3600 + 5800);
        assert_eq!(listed.stats.shipping_paid_cents, 300 + 1000);
        assert_eq!(listed.stats.tax_paid_cents, 360 + 580);
        assert_eq!(
            listed.stats.gross_paid_cents,
            listed.stats.subtotal_paid_cents
                + listed.stats.shipping_paid_cents
                + listed.stats.tax_paid_cents
        );

        let csv = format_orders_csv(&[("Shipping Test Meet".into(), listed.orders)]);
        assert!(csv.contains("order_subtotal,order_shipping,order_tax,order_total"));
        assert!(csv.contains(",36.00,3.00,3.60,42.60,"));
        assert!(csv.contains(",58.00,10.00,5.80,73.80,"));
    }
}
