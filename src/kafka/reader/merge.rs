use std::collections::HashMap;

use crate::kafka::{Kafka, UiMessage};

/// Dispatch merge strategy by start_from option: oldest vs newest.
pub fn consume_merge(
    kafka: &Kafka,
    ends: &HashMap<i32, i64>,
    parts: &Vec<i32>,
    limit: usize,
) -> anyhow::Result<Vec<UiMessage>> {
    let newest = kafka
        .config
        .start_from
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("newest"))
        .unwrap_or(false);
    if newest {
        super::merge_newest::consume_merge_newest(kafka, ends, parts, limit)
    } else {
        super::merge_oldest::consume_merge_oldest(kafka, ends, parts, limit)
    }
}
