use std::path::Path;
use x11rb::connection::Connection;
fn main() {
    match lwsc2::core::state::load_state_definitions("config/states.yaml") {
        Ok(_) => println!("YAML OK"),
        Err(e) => println!("YAML ERR: {}", e),
    }

    let (conn, screen_num) = x11rb::rust_connection::RustConnection::connect(None).unwrap();
    let root = conn.setup().roots[screen_num].root;

    println!("Grabbing pointer...");
    use x11rb::protocol::xproto::{EventMask, GrabMode, ConnectionExt};
    let cookie = conn.grab_pointer(
        false, root,
        EventMask::BUTTON_PRESS | EventMask::POINTER_MOTION,
        GrabMode::ASYNC, GrabMode::ASYNC,
        x11rb::NONE, x11rb::NONE, x11rb::CURRENT_TIME,
    ).unwrap();
    let reply = cookie.reply().unwrap();
    println!("Grab pointer status: {:?}", reply.status);

    println!("Moving and clicking via XTest...");
    use x11rb::protocol::xtest::ConnectionExt as XTestExt;
    conn.xtest_fake_input(6, 0, 0, root, 100, 100, 0).unwrap();
    conn.xtest_fake_input(4, 1, 0, root, 100, 100, 0).unwrap();
    conn.xtest_fake_input(5, 1, 0, root, 100, 100, 0).unwrap();
    conn.flush().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(1000));
    conn.ungrab_pointer(x11rb::CURRENT_TIME).unwrap();
    conn.flush().unwrap();
    println!("Done");
}
