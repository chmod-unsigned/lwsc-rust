fn main() {
    let content = std::fs::read_to_string("config/states.yaml").unwrap();
    match serde_yaml::from_str::<serde_yaml::Value>(&content) {
        Ok(_) => println!("Parsed as generic YAML Ok"),
        Err(e) => println!("Generic parse error: {}", e),
    }
}
