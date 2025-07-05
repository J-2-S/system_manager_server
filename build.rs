use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    // Path to the input Tailwind CSS file
    let input_css = Path::new("global.css");

    // Output path in static/
    let output_css = Path::new("static/style.css");

    // Watch global CSS and Tailwind config
    println!("cargo:rerun-if-changed=global.css");
    println!("cargo:rerun-if-changed=tailwind.config.js");

    // Watch all files in templates/ recursively
    watch_dir_recursive(Path::new("templates"));

    // Step 1: Build Tailwind CSS using the CLI
    let status = Command::new("npx")
        .args([
            "@tailwindcss/cli",
            "-i",
            input_css.to_str().unwrap(),
            "-o",
            output_css.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run Tailwind CSS build");

    if !status.success() {
        panic!("Tailwind build failed");
    }

    // Step 2: Copy everything from static/ to build/
    let out_dir = PathBuf::from("build");
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir).unwrap();
    }
    fs::create_dir_all(&out_dir).unwrap();

    for entry in fs::read_dir("static").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let file_name = path.file_name().unwrap();

        if path.is_file() {
            let dest = out_dir.join(file_name);
            fs::copy(&path, dest).unwrap();
        }
    }

    println!("cargo:rerun-if-changed=static");
}

fn watch_dir_recursive(path: &Path) {
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                watch_dir_recursive(&path);
            } else {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
