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
    let tax_cents = ((subtotal_cents as f64) * (tax_rate_bps as f64) / 10_000.0).round() as i64;
    let total_cents = subtotal_cents + tax_cents;

    let order_id = id::order_id();
    let now = Utc::now().to_rfc3339();
    let shipping_json = match &req.shipping_address {
        Some(addr) => Some(serde_json::to_string(addr)?),
        None => None,
    };

    tx.execute(
        "INSERT INTO orders (
            id, order_number, station_code, customer_name, customer_email, customer_phone,
            shipping_address_json, subtotal_cents, tax_rate_basis_points, tax_cents,
            total_cents, payment_status, notes, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'pending', ?12, ?13, ?14)",
        rusqlite::params![
            order_id,
            order_number,
            station_code,
            customer_name,
            customer_email,
            customer_phone,
            shipping_json,
            subtotal_cents,
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
                    shipping_address_json, subtotal_cents, tax_rate_basis_points, tax_cents,
                    total_cents, payment_status, payment_method, payment_reference, paid_at,
                    notes, created_at
             FROM orders WHERE order_number = ?1",
            [order_number],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, i64>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, Option<String>>(12)?,
                    r.get::<_, Option<String>>(13)?,
                    r.get::<_, Option<String>>(14)?,
                    r.get::<_, Option<String>>(15)?,
                    r.get::<_, String>(16)?,
                ))
            },
        )
        .optional()?;

    let Some((
        id,
        order_number,
        station_code,
        customer_name,
        customer_email,
        customer_phone,
        shipping_json,
        subtotal_cents,
        tax_rate_basis_points,
        tax_cents,
        total_cents,
        payment_status,
        payment_method,
        payment_reference,
        paid_at,
        notes,
        created_at,
    )) = row
    else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT oi.id, oi.order_id, oi.product_id, oi.subject_id, s.display_name,
                oi.photo_id, p.filename, oi.product_type, oi.product_name, oi.product_size,
                oi.quantity, oi.unit_price_cents, oi.line_total_cents, oi.fulfillment_status
         FROM order_items oi
         LEFT JOIN subjects s ON s.id = oi.subject_id
         LEFT JOIN photos p ON p.id = oi.photo_id
         WHERE oi.order_id = ?1
         ORDER BY oi.created_at ASC",
    )?;
    let items: Vec<OrderItemRecord> = stmt
        .query_map([&id], |r| {
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
            })
        })?
        .collect::<std::result::Result<_, _>>()?;

    let shipping_address: Option<ShippingAddress> = match shipping_json {
        Some(j) => serde_json::from_str(&j).ok(),
        None => None,
    };

    let square_pos_url = build_square_pos_url(&order_number, &customer_name, total_cents, &items);

    Ok(Some(OrderRecord {
        id,
        order_number,
        station_code,
        customer_name,
        customer_email,
        customer_phone,
        shipping_address,
        subtotal_cents,
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
    }))
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
