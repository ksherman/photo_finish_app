use rand::RngExt;

const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const DIGITS: &[u8] = b"0123456789";

/// Generates a prefixed, human-scannable unique ID (e.g. `pho_kxz4821a9`).
/// Uses 4 letters + 5 digits + 2 letters (~1.7 trillion combinations per prefix)
/// so merging event bundles from multiple servers has negligible collision risk.
pub fn generate(prefix: &str) -> String {
    let mut rng = rand::rng();
    let mut out = String::with_capacity(prefix.len() + 11);
    out.push_str(prefix);

    for _ in 0..4 {
        let idx = rng.random_range(0..LETTERS.len());
        out.push(LETTERS[idx] as char);
    }
    for _ in 0..5 {
        let idx = rng.random_range(0..DIGITS.len());
        out.push(DIGITS[idx] as char);
    }
    for _ in 0..2 {
        let idx = rng.random_range(0..LETTERS.len());
        out.push(LETTERS[idx] as char);
    }

    out
}

pub fn event_id() -> String {
    generate("evt_")
}

pub fn subject_id() -> String {
    generate("sub_")
}

pub fn photo_id() -> String {
    generate("pho_")
}

pub fn ingest_batch_id() -> String {
    generate("bat_")
}

pub fn product_id() -> String {
    generate("prd_")
}

pub fn order_id() -> String {
    generate("ord_")
}

pub fn order_item_id() -> String {
    generate("itm_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generates_prefixed_unique_ids() {
        let mut seen = HashSet::new();
        for _ in 0..200 {
            let id = photo_id();
            assert!(id.starts_with("pho_"));
            assert_eq!(id.len(), 15);
            assert!(seen.insert(id));
        }
    }
}
