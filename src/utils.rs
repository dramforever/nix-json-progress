pub fn store_path_base(path: &str) -> String {
    path.split('/')
        .last()
        .unwrap_or("")
        .split_once('-')
        .map_or("", |x| x.1)
        .trim_end_matches(".drv")
        .to_string()
}
