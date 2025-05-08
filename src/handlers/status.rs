use crate::{settings::Settings, update_manager::check_updates};
use crate::settings::{get_or_create_settings, load_settings};
use sysinfo::Disks;
use battery::Manager;
use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize)]
struct ServerStatus {
    online: bool,
    up_to_date: bool,
    low_storage: bool,
    low_power: bool,
}

/**
 * returns the amount of free space in percent
 */
pub fn check_storage()->u8{
    let disks = Disks::new_with_refreshed_list();
    let available_space:u64 =  disks.iter().map(|disk| disk.available_space()).sum();
    let total_space:u64 = disks.iter().map(|disk| disk.total_space()).sum();
    100 - (total_space / available_space) as u8 // Should never give a value larger then 255 100% max
}

/** 
 * Returns the current battery percentage as a u8
 * If no battery is found, it returns 100 ( assuming not low power to prevent warnings ).
 */
pub fn check_power() -> Result<u8, String> {
    let manager = Manager::new().map_err(|e| format!("{}",e))?;
    // Use the first available battery (if any); if there is none, assume not low power.
    if let Some(battery_result) = manager.batteries().map_err(|e| format!("{}",e))?.next() {
        let battery = battery_result.map_err(|e|format!("{}",e))?;
        Ok((battery.state_of_charge().value * 100.0).round() as u8)
    } else {
        // No battery found, assume not applicable.
        Ok(100)
    }
}

/**
 * Returns the current server status as a JSON string based on the `ServerStatus` struct.
 */
pub fn get_status() -> Result<String,String> {
    let settings = get_or_create_settings().map_err(|error|error.to_string())?;
    let storage_threshold = settings.thresholds.low_storage;
    let power_threshold = settings.thresholds.low_power;

    // Check for low storage
    let free_storage = check_storage() < storage_threshold;

    // Check for low power
    let power_low = check_power().map_err(|e|e.to_string())? < power_threshold;
    // This assumes that the check_updates function returns a vector and a tuple of empty strings to indicate no updates.
    let mut updates = true;
    if !settings.ignore_update{
        updates = check_updates() == vec![( "".to_string(), "".to_string() )];
    }
    
    let current_status = ServerStatus {
        online: true,
        up_to_date: updates,
        low_storage: free_storage,
        low_power: power_low,
    };

    // Convert the struct to JSON string
    match serde_json::to_string(&current_status) {
        Ok(json_string) => Ok(json_string),
        Err(e) => {
            eprintln!("Error serializing server status: {}", e);
            Err(format!("{}",e))
        }
    }
}
