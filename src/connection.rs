use bluer::{Adapter, Address, AddressType, Device, Session};

use crate::error::GrattError;
use crate::handle_table::HandleTable;

/// Active BLE connection state
#[allow(dead_code)]
pub struct Connection {
    pub session: Session,
    pub adapter: Adapter,
    pub device: Device,
    pub handle_table: HandleTable,
    pub address: Address,
    pub addr_type: AddressType,
    pub sec_level: String,
    pub psm: u16,
    pub mtu_exchanged: bool,
}

/// Parse adapter name like "hci0" to extract the index, or treat as-is for bluer
#[allow(dead_code)]
pub fn parse_adapter_name(name: &str) -> String {
    // bluer uses adapter names like "hci0" directly
    name.to_string()
}

/// Parse a MAC address string like "AA:BB:CC:DD:EE:FF"
pub fn parse_address(addr: &str) -> Result<Address, GrattError> {
    addr.parse::<Address>()
        .map_err(|e| GrattError::Connection(format!("Invalid address '{}': {}", addr, e)))
}

/// Map string address type to bluer AddressType
pub fn parse_addr_type(addr_type: &str) -> AddressType {
    if addr_type.eq_ignore_ascii_case("random") {
        AddressType::LeRandom
    } else {
        AddressType::LePublic
    }
}

/// Connect to a BLE device and build the handle table
pub async fn connect(
    adapter_name: &str,
    address: Address,
    addr_type: AddressType,
    sec_level: &str,
    psm: u16,
) -> Result<Connection, GrattError> {
    let session = Session::new().await?;
    let adapter = session.adapter(adapter_name)?;

    // Ensure adapter is powered on
    if !adapter.is_powered().await? {
        adapter.set_powered(true).await?;
    }

    // Start discovery to find the device
    let device = find_or_create_device(&adapter, address, addr_type).await?;

    // Connect
    if !device.is_connected().await? {
        device.connect().await.map_err(|e| {
            GrattError::Connection(format!("connect error: {}", e))
        })?;
    }

    // Wait for services to be resolved
    wait_for_services(&device).await?;

    // Build handle table
    let handle_table = HandleTable::build(&device).await?;

    Ok(Connection {
        session,
        adapter,
        device,
        handle_table,
        address,
        addr_type,
        sec_level: sec_level.to_string(),
        psm,
        mtu_exchanged: false,
    })
}

/// Find a device by address or create a known-device entry
async fn find_or_create_device(
    adapter: &Adapter,
    address: Address,
    addr_type: AddressType,
) -> Result<Device, GrattError> {
    // First check if we already know about this device
    for addr in adapter.device_addresses().await? {
        if addr == address {
            let dev = adapter.device(address)?;
            if dev.address_type().await.ok() == Some(addr_type) || addr_type == AddressType::LePublic {
                return Ok(dev);
            }
        }
    }

    // Start a short discovery to find it
    let discover = adapter.discover_devices().await?;
    let mut stream = std::pin::pin!(discover);

    use futures::StreamExt;
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(10));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(event) = stream.next() => {
                if let bluer::AdapterEvent::DeviceAdded(addr) = event {
                    if addr == address {
                        drop(stream);
                        return Ok(adapter.device(address)?);
                    }
                }
            }
            _ = &mut timeout => {
                return Err(GrattError::Connection(format!(
                    "Device {} not found during discovery", address
                )));
            }
        }
    }
}

/// Wait for GATT services to be resolved after connection
async fn wait_for_services(device: &Device) -> Result<(), GrattError> {
    // Poll for services resolved, with timeout
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if device.is_services_resolved().await? {
            return Ok(());
        }
        if tokio::time::Instant::now() > deadline {
            return Err(GrattError::Connection(
                "Timed out waiting for services to resolve".into(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Disconnect from a device
pub async fn disconnect(device: &Device) -> Result<(), GrattError> {
    if device.is_connected().await.unwrap_or(false) {
        device.disconnect().await?;
    }
    Ok(())
}
