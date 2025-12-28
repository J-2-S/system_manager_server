use crate::update_manager::check_updates;
use battery::Manager;
use sysinfo::Disks;

/// Checks the storage status of the system
/// returns the amount of free space in percent as a [u8]
pub fn check_storage() -> u8 {
    let disks = Disks::new_with_refreshed_list();
    let available_space: u64 = disks.iter().map(|disk| disk.available_space()).sum();
    let total_space: u64 = disks.iter().map(|disk| disk.total_space()).sum();
    100 - (total_space / available_space) as u8 // Should never give a value larger then 255 100% max
}

/// Checks the power status of the system
/// if the system runs on batteries, it will return the percentage of the battery
/// if the system does not run on batteries, it will return 100 as a [u8]
pub fn check_power() -> Result<u8, Box<dyn std::error::Error + Send + Sync>> {
    let manager = Manager::new()?;
    // Use the first available battery (if any); if there is none, assume not low power.
    if let Some(battery_result) = manager.batteries()?.next() {
        let battery = battery_result?;
        Ok((battery.state_of_charge().value * 100.0).round() as u8)
    } else {
        // No battery found, assume not applicable.
        Ok(100)
    }
}
