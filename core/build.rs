use std::process::Command;
use std::path::Path;

fn main() {
    // Tell Cargo to re-run if dashboard source changes
    println!("cargo:rerun-if-changed=dashboard/src");
    println!("cargo:rerun-if-changed=dashboard/package.json");

    let dashboard_dir = Path::new("../dashboard");

    // Only build if dashboard source exists
    if !dashboard_dir.join("package.json").exists() {
        return;
    }

    // Check if Node is available
    let node_check = Command::new("node").arg("--version").output();
    if node_check.is_err() {
        println!("cargo:warning=Node.js not found — dashboard will not be built.");
        println!("cargo:warning=Install Node.js and run: cd dashboard && npm install && npm run build");
        return;
    }

    // npm install if node_modules missing
    if !dashboard_dir.join("node_modules").exists() {
        println!("cargo:warning=Installing dashboard dependencies...");
        let status = Command::new("npm")
            .arg("install")
            .current_dir(dashboard_dir)
            .status();

        if status.map(|s| !s.success()).unwrap_or(true) {
            println!("cargo:warning=npm install failed — dashboard will not be built.");
            return;
        }
    }

    // npm run build
    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(dashboard_dir)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:warning=Dashboard built successfully.");
        }
        _ => {
            println!("cargo:warning=Dashboard build failed. Run manually: cd dashboard && npm run build");
        }
    }
}
