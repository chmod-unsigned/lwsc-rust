use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, EventMask, GrabMode, GrabStatus};
use x11rb::protocol::xtest::ConnectionExt as XTestExt;
use x11rb::rust_connection::RustConnection;
use std::thread;
use std::time::Duration;

fn main() {
    let (conn, screen_num) = RustConnection::connect(None).unwrap();
    let root = conn.setup().roots[screen_num].root;

    // Grab pointer
    let cookie = conn.grab_pointer(
        false,
        root,
        EventMask::BUTTON_PRESS | EventMask::POINTER_MOTION,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
        x11rb::NONE,
        x11rb::NONE,
        x11rb::CURRENT_TIME,
    ).unwrap();
    
    let reply = cookie.reply().unwrap();
    println!("Grab pointer status: {:?}", reply.status);

    // XTest move and click
    println!("Moving and clicking via XTest...");
    conn.xtest_fake_input(6, 0, 0, root, 100, 100, 0).unwrap();
    conn.xtest_fake_input(4, 1, 0, root, 100, 100, 0).unwrap();
    conn.xtest_fake_input(5, 1, 0, root, 100, 100, 0).unwrap();
    conn.flush().unwrap();

    thread::sleep(Duration::from_millis(1000));
    conn.ungrab_pointer(x11rb::CURRENT_TIME).unwrap();
    conn.flush().unwrap();
    println!("Done");
}
