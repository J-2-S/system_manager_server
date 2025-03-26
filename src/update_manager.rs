use std::process::{Command, Stdio};
// ONLY TEST ON DEBIAN WE NEED TO TEST OTHER DISTROS
// Detects the package manager
pub fn detect_package_manager() -> Option<&'static str> {
    let managers = [
        ("/usr/bin/apt", "apt"),
        ("/usr/bin/dnf", "dnf"),
        ("/usr/bin/yum", "yum"),
        ("/usr/bin/zypper", "zypper"),
        ("/usr/bin/pacman", "pacman"),
    ];

    for (path, name) in managers.iter() {
        if std::path::Path::new(path).exists() {
            return Some(name);
        }
    }

    None
}

// Runs a shell command and returns output as a String
pub fn run_command(command: &str) -> Option<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .output()
        .ok()?;

    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

// Parses installed packages into a Vec<String>
pub fn list_installed_packages() -> Vec<String> {
    let command = match detect_package_manager() {
        Some("apt") => "dpkg --get-selections | awk '{print $1}'",
        Some("dnf") => "dnf list installed | awk '{print $1}' | tail -n +2",
        Some("yum") => "yum list installed | awk '{print $1}' | tail -n +2",
        Some("zypper") => "zypper se --installed-only | awk '{print $2}'",
        Some("pacman") => "pacman -Q | awk '{print $1}'",
        _ => return vec![],
    };

    run_command(command)
        .map(|output| output.lines().map(|s| s.to_string()).collect())
        .unwrap_or_else(Vec::new)
}

// Parses available updates into a Vec<(String, String)> (package name, new version)
pub fn check_updates() -> Vec<(String, String)> {
    let command = match detect_package_manager() {
        Some("apt") => "apt list --upgradable | awk -F'/' '{print $1, $2}' | tail -n +2",
        Some("dnf") => "dnf check-update | awk '{print $1, $2}' | tail -n +2",
        Some("yum") => "yum check-update | awk '{print $1, $2}' | tail -n +2",
        Some("zypper") => "zypper list-updates | awk '{print $2, $3}'",
        Some("pacman") => "pacman -Qu | awk '{print $1, $2}'",
        _ => return vec![],
    };

    run_command(command)
        .map(|output| {
            output
                .lines()
                .filter_map(|line| {
                    let mut parts = line.split_whitespace();
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_else(Vec::new)
}


// Updates packages
pub fn update_packages() {
    let updates = check_updates();
    if updates.is_empty() {
        println!("No updates available.");
        return;
    }

    println!("Available updates:");
    for (pkg, version) in &updates {
        println!("  {} -> {}", pkg, version);
    }


        let update_command = match detect_package_manager() {
            Some("apt") => "apt update && apt upgrade -y",
            Some("dnf") => "dnf upgrade -y",
            Some("yum") => "yum update -y",
            Some("zypper") => "zypper refresh && zypper update -y",
            Some("pacman") => "pacman -Syu --noconfirm",
            _ => return,
        };
        println!("Updating packages...");
        let _ = Command::new("sh").arg("-c").arg(update_command).status();
        println!("Update complete.");
}


