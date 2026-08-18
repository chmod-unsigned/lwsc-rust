fn main() {
    match lwsc2::core::state::load_state_definitions("config/states.yaml") {
        Ok(_) => println!("YAML OK"),
        Err(e) => println!("YAML ERR: {}", e),
    }
}
