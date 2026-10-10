/// sysinfo reports 100% per logical CPU; Core reports a share of the whole host.
pub(crate) fn host_cpu_percent(process_cpu: f32, cpu_count: f32) -> f32 {
    (process_cpu / cpu_count.max(1.0)).clamp(0.0, 100.0)
}

pub(crate) fn host_memory_percent(
    process_bytes: u64,
    host_bytes: u64,
) -> Option<f64> {
    if host_bytes == 0 {
        return None;
    }
    Some((process_bytes as f64 / host_bytes as f64 * 100.0).clamp(0.0, 100.0))
}

#[cfg(test)]
mod tests {
    use super::{host_cpu_percent, host_memory_percent};

    #[test]
    fn resource_percentages_use_total_host_capacity() {
        assert_eq!(host_cpu_percent(200.0, 8.0), 25.0);
        assert_eq!(host_memory_percent(512, 2048), Some(25.0));
    }

    #[test]
    fn resource_percentages_handle_limits_and_missing_capacity() {
        assert_eq!(host_cpu_percent(900.0, 8.0), 100.0);
        assert_eq!(host_cpu_percent(-1.0, 8.0), 0.0);
        assert_eq!(host_memory_percent(4096, 2048), Some(100.0));
        assert_eq!(host_memory_percent(512, 0), None);
    }
}
