// PERF: Fully optimized
pub fn get_file_size_string(size: u64) -> String {
    if size == 0 {
        return String::from("0 B");
    }

    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    const SCALE: u64 = 1024;

    let mut size_u = size;
    let mut group = 0;

    while size_u >= SCALE && group < UNITS.len() - 1 {
        size_u /= SCALE;
        group += 1;
    }

    if group == 0 {
        format!("{} {}", size_u, UNITS[group])
    } else {
        let size_f = size as f64 / (SCALE as f64).powi(group as i32);
        format!("{:.1} {}", size_f, UNITS[group])
    }
}
