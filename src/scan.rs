use std::collections::BTreeMap;

use bluer::{Adapter, AdapterEvent, Address, AddressType, Session};
use futures::StreamExt;

use crate::error::GrattError;
use crate::output;

/// Information about a discovered BLE device
#[derive(Debug, Clone)]
pub struct ScannedDevice {
    pub address: Address,
    pub addr_type: Option<AddressType>,
    pub name: Option<String>,
    pub rssi: Option<i16>,
}

/// Perform a BLE device scan for the given duration (seconds).
/// Prints devices as they are found, then prints a summary table.
pub async fn scan(adapter_name: &str, duration_secs: u64) -> Result<Vec<ScannedDevice>, GrattError> {
    let session = Session::new().await?;
    let adapter = session.adapter(adapter_name)?;

    // Ensure adapter is powered on
    if !adapter.is_powered().await? {
        adapter.set_powered(true).await?;
    }

    eprintln!(
        "LE Scan on {} [{}] for {}s ...",
        adapter_name,
        adapter.address().await?,
        duration_secs,
    );

    let mut devices: BTreeMap<Address, ScannedDevice> = BTreeMap::new();

    // Check devices already known to the adapter
    populate_known_devices(&adapter, &mut devices).await;

    // Start discovery
    let discover = adapter.discover_devices().await?;
    let mut stream = std::pin::pin!(discover);

    let timeout = tokio::time::sleep(std::time::Duration::from_secs(duration_secs));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(event) = stream.next() => {
                match event {
                    AdapterEvent::DeviceAdded(addr) => {
                        if let Ok(dev) = adapter.device(addr) {
                            let name = dev.name().await.ok().flatten();
                            let rssi = dev.rssi().await.ok().flatten();
                            let addr_type = dev.address_type().await.ok();

                            let display_name = name.as_deref().unwrap_or("(unknown)");
                            eprintln!("{} {} RSSI: {}",
                                addr,
                                display_name,
                                rssi.map(|r| format!("{}", r)).unwrap_or_else(|| "n/a".into()),
                            );

                            devices.insert(addr, ScannedDevice {
                                address: addr,
                                addr_type,
                                name,
                                rssi,
                            });
                        }
                    }
                    AdapterEvent::DeviceRemoved(_) => {}
                    _ => {}
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    let result: Vec<ScannedDevice> = devices.into_values().collect();

    // Print summary table
    if result.is_empty() {
        println!("No devices found.");
    } else {
        print!("{}", output::render_scan_table(&result));
    }

    Ok(result)
}

/// Populate the device map with devices already known to the adapter.
async fn populate_known_devices(
    adapter: &Adapter,
    devices: &mut BTreeMap<Address, ScannedDevice>,
) {
    if let Ok(addrs) = adapter.device_addresses().await {
        for addr in addrs {
            if let Ok(dev) = adapter.device(addr) {
                let name = dev.name().await.ok().flatten();
                let rssi = dev.rssi().await.ok().flatten();
                let addr_type = dev.address_type().await.ok();

                devices.insert(addr, ScannedDevice {
                    address: addr,
                    addr_type,
                    name,
                    rssi,
                });
            }
        }
    }
}

/// Scan for interactive mode — returns lines to display.
pub async fn scan_interactive(adapter_name: &str, duration_secs: u64) -> Result<String, GrattError> {
    let session = Session::new().await?;
    let adapter = session.adapter(adapter_name)?;

    if !adapter.is_powered().await? {
        adapter.set_powered(true).await?;
    }

    let mut devices: BTreeMap<Address, ScannedDevice> = BTreeMap::new();

    populate_known_devices(&adapter, &mut devices).await;

    let discover = adapter.discover_devices().await?;
    let mut stream = std::pin::pin!(discover);

    let timeout = tokio::time::sleep(std::time::Duration::from_secs(duration_secs));
    tokio::pin!(timeout);

    let mut live_output = String::new();
    live_output.push_str(&format!(
        "LE Scan on {} [{}] for {}s ...\n",
        adapter_name,
        adapter.address().await?,
        duration_secs,
    ));

    loop {
        tokio::select! {
            Some(event) = stream.next() => {
                if let AdapterEvent::DeviceAdded(addr) = event {
                    if let Ok(dev) = adapter.device(addr) {
                        let name = dev.name().await.ok().flatten();
                        let rssi = dev.rssi().await.ok().flatten();
                        let addr_type = dev.address_type().await.ok();

                        let display_name = name.as_deref().unwrap_or("(unknown)");
                        live_output.push_str(&format!(
                            "{} {} RSSI: {}\n",
                            addr,
                            display_name,
                            rssi.map(|r| format!("{}", r)).unwrap_or_else(|| "n/a".into()),
                        ));

                        devices.insert(addr, ScannedDevice {
                            address: addr,
                            addr_type,
                            name,
                            rssi,
                        });
                    }
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    let result: Vec<ScannedDevice> = devices.into_values().collect();

    if result.is_empty() {
        live_output.push_str("No devices found.");
    } else {
        live_output.push_str(&output::render_scan_table(&result));
    }

    Ok(live_output)
}
