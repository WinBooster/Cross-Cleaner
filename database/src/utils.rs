pub fn get_file_size_string(size: u64) -> String {
    if size == 0 {
        return String::from("0 B");
    }

    let units = ["B", "KB", "MB", "GB", "TB"];
    let digit_groups = ((size as f64).log(1024.0)).floor() as usize;

    let size_in_units = size as f64 / 1024_f64.powi(digit_groups as i32);
    format!("{:.1} {}", size_in_units, units[digit_groups])
}
