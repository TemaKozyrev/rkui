use std::time::Duration;

use rdkafka::consumer::Consumer;
use rdkafka::topic_partition_list::TopicPartitionList;
use rdkafka::Offset;

use super::service::Kafka;

impl Kafka {
    /// Apply partition/offset filters and reset internal reading state.
    pub fn apply_filters_mut(
        &mut self,
        partition: Option<String>,
        start_offset: Option<i64>,
        start_from: Option<String>,
    ) -> anyhow::Result<()> {
        self.config.partition = partition;
        self.config.start_offset = start_offset;
        self.config.start_from = start_from.or_else(|| self.config.start_from.clone());
        // Reset assignment state so next consume will reassign
        self.assigned.store(true, std::sync::atomic::Ordering::SeqCst);
        self.end_offsets
            .lock()
            .map_err(|e| anyhow::anyhow!("State lock poisoned (end_offsets): {e}"))?
            .clear();
        self.partitions
            .lock()
            .map_err(|e| anyhow::anyhow!("State lock poisoned (partitions): {e}"))?
            .clear();
        self.done_partitions
            .lock()
            .map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?
            .clear();
        self.buffers
            .lock()
            .map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?
            .clear();
        let empty = TopicPartitionList::new();
        self.consumer.assign(&empty)?;
        // Mark as not assigned so next consume will ensure assignment
        self.assigned.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Ensure we are assigned to the desired partitions with proper starting offsets.
    pub(crate) fn ensure_assigned(&self) -> anyhow::Result<()> {
        use std::sync::atomic::Ordering;
        if self.assigned.swap(true, Ordering::SeqCst) {
            return Ok(()); // already assigned
        }
        let topic = &self.config.topic;
        // Determine partitions to consume
        let partitions: Vec<i32> = if let Some(part_str) = &self.config.partition {
            if part_str != "all" && !part_str.is_empty() {
                let p: i32 = part_str
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Invalid partition id '{}': {}", part_str, e))?;
                vec![p]
            } else {
                // enumerate all partitions for topic
                let md = self
                    .consumer
                    .client()
                    .fetch_metadata(Some(topic), Duration::from_secs(5))?;
                let t = md
                    .topics()
                    .iter()
                    .find(|t| t.name() == topic)
                    .ok_or_else(|| anyhow::anyhow!("Topic not found in metadata"))?;
                t.partitions().iter().map(|p| p.id()).collect()
            }
        } else {
            // enumerate all partitions for topic
            let md = self
                .consumer
                .client()
                .fetch_metadata(Some(topic), Duration::from_secs(5))?;
            let t = md
                .topics()
                .iter()
                .find(|t| t.name() == topic)
                .ok_or_else(|| anyhow::anyhow!("Topic not found in metadata"))?;
            t.partitions().iter().map(|p| p.id()).collect()
        };

        // Snapshot end offsets (high watermarks) before we start reading
        {
            let mut ends = self
                .end_offsets
                .lock()
                .map_err(|e| anyhow::anyhow!("State lock poisoned (end_offsets): {e}"))?;
            ends.clear();
            for p in &partitions {
                let (_low, high) = self
                    .consumer
                    .fetch_watermarks(topic, *p, Duration::from_secs(5))?;
                ends.insert(*p, high);
            }
        }
        // store partitions
        {
            let mut parts = self
                .partitions
                .lock()
                .map_err(|e| anyhow::anyhow!("State lock poisoned (partitions): {e}"))?;
            *parts = partitions.clone();
        }
        // init buffers for partitions
        self.buffers
            .lock()
            .map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?
            .clear();

        // Assign explicit starting offsets based on selected partition and requested start_offset
        let mut tpl = TopicPartitionList::new();
        let is_all = self
            .config
            .partition
            .as_deref()
            .map(|s| s == "all")
            .unwrap_or(true);
        let newest = self
            .config
            .start_from
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("newest"))
            .unwrap_or(false);
        const BACK_WINDOW: i64 = 2000; // how many latest offsets to read back from end when starting from newest
        for p in partitions {
            let off = if newest {
                let (low, high) =
                    self.consumer.fetch_watermarks(topic, p, Duration::from_secs(5))?;
                let start = if high > BACK_WINDOW { high - BACK_WINDOW } else { low };
                Offset::Offset(start)
            } else if is_all {
                // When reading all partitions, ignore start_offset and begin from earliest for each
                Offset::Beginning
            } else if let Some(req) = self.config.start_offset {
                // Clamp to earliest available if requested offset is older than retention (deleted)
                let (low, _high) = self
                    .consumer
                    .fetch_watermarks(topic, p, Duration::from_secs(5))?;
                let effective = if req < low { low } else { req };
                Offset::Offset(effective)
            } else {
                Offset::Beginning
            };
            tpl.add_partition_offset(topic, p, off)?;
        }
        self.consumer.assign(&tpl)?;
        Ok(())
    }
}
