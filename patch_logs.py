import sys

with open("src/ui/config_window.rs", "r") as f:
    content = f.read()

content = content.replace("IS_WINDOW_OPEN.store(false, Ordering::SeqCst);", "println!(\"[ConfigWindow] Drop guard triggered. Setting IS_WINDOW_OPEN to false\");\n                        IS_WINDOW_OPEN.store(false, Ordering::SeqCst);")

content = content.replace("CLOSE_REQUESTED.store(true, Ordering::SeqCst);", "println!(\"[ConfigWindow] Toggle requested. Setting CLOSE_REQUESTED to true\");\n            CLOSE_REQUESTED.store(true, Ordering::SeqCst);")

content = content.replace("ctx.send_viewport_cmd(egui::ViewportCommand::Close);", "println!(\"[ConfigWindow] Sending ViewportCommand::Close\");\n            ctx.send_viewport_cmd(egui::ViewportCommand::Close);")

with open("src/ui/config_window.rs", "w") as f:
    f.write(content)
