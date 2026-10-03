use sha2::{Digest, Sha256};
use std::process::Command;

/// Gathers hardware identifiers from CPU, Motherboard, and Disk,
/// and produces a deterministic SHA-256 string for the device.
pub fn get_hardware_id() -> String {
    let mut hasher = Sha256::new();

    let cpu_info = get_cpu_id();
    let board_info = get_motherboard_serial();
    let disk_info = get_disk_uuid();

    hasher.update(cpu_info.as_bytes());
    hasher.update(b"|");
    hasher.update(board_info.as_bytes());
    hasher.update(b"|");
    hasher.update(disk_info.as_bytes());

    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(target_os = "macos")]
fn get_cpu_id() -> String {
    let output = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "Apple_Silicon_Generic".to_string(),
    }
}

#[cfg(target_os = "macos")]
fn get_motherboard_serial() -> String {
    let output = Command::new("ioreg")
        .args(["-l"])
        .output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("\"IOPlatformSerialNumber\"") {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() > 1 {
                    return parts[1].replace('\"', "").trim().to_string();
                }
            }
        }
    }
    "MAC_IO_PLATFORM_FALLBACK".to_string()
}

#[cfg(target_os = "macos")]
fn get_disk_uuid() -> String {
    let output = Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("\"IOPlatformUUID\"") {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() > 1 {
                    return parts[1].replace('\"', "").trim().to_string();
                }
            }
        }
    }
    "MAC_DISK_UUID_FALLBACK".to_string()
}

#[cfg(target_os = "windows")]
fn get_cpu_id() -> String {
    let output = Command::new("wmic")
        .args(["cpu", "get", "ProcessorId"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.lines().nth(1).unwrap_or("WIN_CPU_DEFAULT").trim().to_string()
        }
        _ => "WIN_CPU_FALLBACK".to_string(),
    }
}

#[cfg(target_os = "windows")]
fn get_motherboard_serial() -> String {
    let output = Command::new("wmic")
        .args(["baseboard", "get", "SerialNumber"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.lines().nth(1).unwrap_or("WIN_MB_DEFAULT").trim().to_string()
        }
        _ => "WIN_MB_FALLBACK".to_string(),
    }
}

#[cfg(target_os = "windows")]
fn get_disk_uuid() -> String {
    let output = Command::new("wmic")
        .args(["diskdrive", "get", "SerialNumber"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.lines().nth(1).unwrap_or("WIN_DISK_DEFAULT").trim().to_string()
        }
        _ => "WIN_DISK_FALLBACK".to_string(),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn get_cpu_id() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_else(|_| "LINUX_CPU_DEFAULT".to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn get_motherboard_serial() -> String {
    std::fs::read_to_string("/sys/class/dmi/id/product_serial")
        .unwrap_or_else(|_| "LINUX_MB_DEFAULT".to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn get_disk_uuid() -> String {
    std::fs::read_to_string("/etc/machine-id")
        .unwrap_or_else(|_| "LINUX_DISK_DEFAULT".to_string())
}

pub fn get_device_name() -> String {
    #[cfg(target_os = "macos")]
    {
        "macOS Desktop".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "Windows Desktop".to_string()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "Linux Desktop".to_string()
    }
}
