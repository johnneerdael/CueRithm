fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&cuerithm_autotag::settings_catalog()).unwrap()
    );
}
