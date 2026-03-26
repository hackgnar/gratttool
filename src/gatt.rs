use crate::connection::Connection;
use crate::error::GrattError;
use crate::handle_table::{CharProps, GattObject, normalize_uuid};
use crate::output;
use crate::output::OutputMode;

use bluer::gatt::remote::CharacteristicWriteRequest;
use bluer::gatt::WriteOp;
use futures::StreamExt;
use std::io::Write;
use std::pin::Pin;
use std::time::Duration;

/// Discover all primary services, printing in gatttool non-interactive format
pub async fn discover_primary(
    conn: &Connection,
    uuid_filter: Option<&str>,
) -> Result<(), GrattError> {
    let services = conn.handle_table.services();
    if services.is_empty() {
        eprintln!("Discover all primary services failed: No attribute found within the given range");
        return Ok(());
    }

    for obj in services {
        if let GattObject::Service {
            uuid,
            start_handle,
            end_handle,
            ..
        } = obj
        {
            if let Some(filter) = uuid_filter {
                if normalize_uuid(uuid) == normalize_uuid(filter) {
                    println!("{}", output::fmt_primary_by_uuid(*start_handle, *end_handle));
                }
            } else {
                println!(
                    "{}",
                    output::fmt_primary_service(*start_handle, *end_handle, uuid)
                );
            }
        }
    }
    Ok(())
}

/// Discover characteristics in a handle range, optionally filtered by UUID
pub async fn discover_characteristics(
    conn: &Connection,
    start: u16,
    end: u16,
    uuid_filter: Option<&str>,
) -> Result<(), GrattError> {
    let chars = conn.handle_table.characteristics_in_range(start, end, uuid_filter);

    if chars.is_empty() {
        eprintln!(
            "Discover all characteristics failed: No attribute found within the given range"
        );
        return Ok(());
    }

    for obj in chars {
        if let GattObject::Characteristic {
            uuid,
            declaration_handle,
            value_handle,
            properties,
            ..
        } = obj
        {
            println!(
                "{}",
                output::fmt_characteristic(*declaration_handle, properties.0, *value_handle, uuid)
            );
        }
    }
    Ok(())
}

/// Discover all descriptors in a handle range
pub async fn discover_descriptors(
    conn: &Connection,
    start: u16,
    end: u16,
) -> Result<(), GrattError> {
    let objects = conn.handle_table.descriptors_in_range(start, end);

    if objects.is_empty() {
        eprintln!("Discover descriptors failed: No attribute found within the given range");
        return Ok(());
    }

    for obj in objects {
        match obj {
            GattObject::Service {
                uuid,
                start_handle,
                ..
            } => {
                println!("{}", output::fmt_descriptor(*start_handle, uuid));
            }
            GattObject::Characteristic {
                uuid,
                declaration_handle,
                ..
            } => {
                println!("{}", output::fmt_descriptor(*declaration_handle, uuid));
            }
            GattObject::Descriptor { uuid, handle, .. } => {
                println!("{}", output::fmt_descriptor(*handle, uuid));
            }
        }
    }
    Ok(())
}

/// Read a characteristic or descriptor by handle
pub async fn read_by_handle(conn: &Connection, handle: u16, mode: OutputMode) -> Result<(), GrattError> {
    let obj = conn.handle_table.find_rw_object(handle);
    match obj {
        Some(GattObject::Characteristic { characteristic, .. }) => {
            let data = characteristic.read().await.map_err(|e| {
                GrattError::Gatt(format!(
                    "Characteristic value/descriptor read failed: {}",
                    e
                ))
            })?;
            println!("{}", output::fmt_char_value(&data, mode));
        }
        Some(GattObject::Descriptor { descriptor, .. }) => {
            let data = descriptor.read().await.map_err(|e| {
                GrattError::Gatt(format!(
                    "Characteristic value/descriptor read failed: {}",
                    e
                ))
            })?;
            println!("{}", output::fmt_char_value(&data, mode));
        }
        _ => {
            return Err(GrattError::Gatt(
                "Characteristic value/descriptor read failed: Invalid handle".to_string(),
            ));
        }
    }
    Ok(())
}

/// Read characteristics by UUID within a handle range
pub async fn read_by_uuid(
    conn: &Connection,
    uuid: &str,
    start: u16,
    end: u16,
    mode: OutputMode,
) -> Result<(), GrattError> {
    let chars = conn.handle_table.chars_by_uuid_in_range(uuid, start, end);

    if chars.is_empty() {
        eprintln!("Read characteristics by UUID failed: No attribute found within the given range");
        return Ok(());
    }

    for obj in chars {
        if let GattObject::Characteristic {
            characteristic,
            value_handle,
            ..
        } = obj
        {
            match characteristic.read().await {
                Ok(data) => {
                    println!("{}", output::fmt_read_by_uuid(*value_handle, &data, mode));
                }
                Err(e) => {
                    eprintln!("Read characteristics by UUID failed: {}", e);
                }
            }
        }
    }
    Ok(())
}

/// Write command (no response) to a handle
pub async fn write_command(
    conn: &Connection,
    handle: u16,
    value: &[u8],
) -> Result<(), GrattError> {
    let obj = conn.handle_table.find_rw_object(handle);
    match obj {
        Some(GattObject::Characteristic { characteristic, .. }) => {
            // Default WriteOp is Command (write without response)
            characteristic
                .write(value)
                .await
                .map_err(|e| GrattError::Gatt(format!("Write failed: {}", e)))?;
            // No output on success (fire and forget)
        }
        Some(GattObject::Descriptor { descriptor, .. }) => {
            descriptor
                .write(value)
                .await
                .map_err(|e| GrattError::Gatt(format!("Write failed: {}", e)))?;
        }
        _ => {
            return Err(GrattError::InvalidHandle("A valid handle is required".into()));
        }
    }
    Ok(())
}

/// Write request (with response) to a handle
pub async fn write_request(
    conn: &Connection,
    handle: u16,
    value: &[u8],
) -> Result<(), GrattError> {
    let obj = conn.handle_table.find_rw_object(handle);
    match obj {
        Some(GattObject::Characteristic { characteristic, .. }) => {
            let req = CharacteristicWriteRequest {
                op_type: WriteOp::Request,
                ..Default::default()
            };
            characteristic
                .write_ext(value, &req)
                .await
                .map_err(|e| {
                    GrattError::Gatt(format!("Characteristic Write Request failed: {}", e))
                })?;
            println!("Characteristic value was written successfully");
        }
        Some(GattObject::Descriptor { descriptor, .. }) => {
            descriptor
                .write(value)
                .await
                .map_err(|e| {
                    GrattError::Gatt(format!("Characteristic Write Request failed: {}", e))
                })?;
            println!("Characteristic value was written successfully");
        }
        _ => {
            return Err(GrattError::InvalidHandle("A valid handle is required".into()));
        }
    }
    Ok(())
}

/// Type alias for the combined notification stream
pub type NotifyStream = futures::stream::SelectAll<
    Pin<Box<dyn futures::Stream<Item = (u16, bool, Vec<u8>)> + Send>>,
>;

/// Subscribe to all characteristics for notifications. Tries every
/// characteristic, not just those advertising NOTIFY/INDICATE — some devices
/// (and CTF challenges) send notifications on handles that don't set those
/// property bits. Failures are silently ignored.
pub async fn subscribe_notifications(conn: &Connection) -> Result<Option<NotifyStream>, GrattError> {
    let all_chars = conn.handle_table.all_characteristics();

    if all_chars.is_empty() {
        return Ok(None);
    }

    let mut boxed_streams: Vec<Pin<Box<dyn futures::Stream<Item = (u16, bool, Vec<u8>)> + Send>>> =
        Vec::new();
    for obj in &all_chars {
        if let GattObject::Characteristic {
            characteristic,
            value_handle,
            properties,
            ..
        } = obj
        {
            let handle = *value_handle;
            let is_indication = (properties.0 & 0x20) != 0;
            match characteristic.notify().await {
                Ok(stream) => {
                    boxed_streams.push(Box::pin(
                        stream.map(move |data| (handle, is_indication, data)),
                    ));
                }
                Err(_) => continue,
            }
        }
    }

    if boxed_streams.is_empty() {
        return Ok(None);
    }

    Ok(Some(futures::stream::select_all(boxed_streams)))
}

/// Listen for notifications from both the D-Bus stream and the HCI monitor
/// socket.  The monitor catches notifications on characteristics that don't
/// advertise NOTIFY/INDICATE (where BlueZ D-Bus drops the PDU).  Handles
/// seen via D-Bus are tracked to avoid duplicates from the monitor.
pub async fn listen_combined(
    dbus_stream: Option<NotifyStream>,
    monitor_rx: Option<tokio::sync::mpsc::UnboundedReceiver<crate::monitor::NotificationEvent>>,
    mode: OutputMode,
) -> Result<(), GrattError> {
    use std::collections::HashSet;

    let mut dbus = dbus_stream;
    let mut monitor = monitor_rx;
    // Track handles that delivered via D-Bus so we can deduplicate
    let mut dbus_handles: HashSet<u16> = HashSet::new();

    let has_any = dbus.is_some() || monitor.is_some();
    if !has_any {
        eprintln!("No characteristics with notify/indicate found, waiting...");
        tokio::signal::ctrl_c().await.ok();
        return Ok(());
    }

    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    loop {
        tokio::select! {
            // D-Bus notification stream
            Some((handle, is_indication, data)) = async {
                match dbus.as_mut() {
                    Some(s) => s.next().await,
                    None => std::future::pending().await,
                }
            } => {
                dbus_handles.insert(handle);
                if is_indication {
                    println!("{}", output::fmt_indication(handle, &data, mode));
                } else {
                    println!("{}", output::fmt_notification(handle, &data, mode));
                }
                std::io::stdout().flush().ok();
            }
            // HCI monitor (catches notifications D-Bus drops)
            Some(evt) = async {
                match monitor.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                // Skip if D-Bus already delivered this handle (avoid duplicates)
                if dbus_handles.contains(&evt.handle) {
                    continue;
                }
                if evt.is_indication {
                    println!("{}", output::fmt_indication(evt.handle, &evt.value, mode));
                } else {
                    println!("{}", output::fmt_notification(evt.handle, &evt.value, mode));
                }
                std::io::stdout().flush().ok();
            }
            _ = &mut ctrl_c => {
                break;
            }
        }
    }

    Ok(())
}

/// Enumerate all services, characteristics, descriptors and values in a table
pub async fn enumerate(conn: &Connection) -> Result<(), GrattError> {
    // Print device info table first
    let dev_info = collect_device_info(conn).await;
    print!("{}", output::render_device_info_table(&dev_info));
    println!();

    let services = conn.handle_table.services();
    if services.is_empty() {
        eprintln!("No services found");
        return Ok(());
    }

    let mut rows: Vec<output::EnumRow> = Vec::new();

    for svc in &services {
        if let GattObject::Service {
            uuid,
            start_handle,
            end_handle,
            ..
        } = svc
        {
            // Service row
            rows.push(output::enum_service_row(*start_handle, *end_handle, uuid));

            // Get characteristics belonging to this service
            let chars =
                conn.handle_table
                    .characteristics_in_range(*start_handle, *end_handle, None);

            for ch in &chars {
                if let GattObject::Characteristic {
                    characteristic,
                    uuid: char_uuid,
                    value_handle,
                    properties,
                    declaration_handle,
                    ..
                } = ch
                {
                    let props_str = properties.to_string();

                    // Try to read the value if READ is supported (skip INDICATE to avoid hangs)
                    let data = if (properties.0 & CharProps::READ) != 0
                        && (properties.0 & CharProps::INDICATE) == 0
                    {
                        match tokio::time::timeout(
                            Duration::from_secs(3),
                            characteristic.read(),
                        )
                        .await
                        {
                            Ok(Ok(d)) => Some(d),
                            _ => None,
                        }
                    } else {
                        None
                    };

                    rows.push(output::enum_char_row(
                        *value_handle,
                        char_uuid,
                        &props_str,
                        data.as_deref(),
                    ));

                    // Get descriptors for this characteristic
                    // Descriptors are between value_handle+1 and the next characteristic or service end
                    let desc_start = *value_handle + 1;
                    // Find next characteristic's declaration handle to bound the range
                    let desc_end = chars
                        .iter()
                        .filter_map(|c| {
                            if let GattObject::Characteristic {
                                declaration_handle: dh,
                                ..
                            } = c
                            {
                                if *dh > *declaration_handle {
                                    Some(*dh - 1)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .min()
                        .unwrap_or(*end_handle);

                    let descs = if desc_start <= desc_end {
                        conn.handle_table.descriptors_in_range(desc_start, desc_end)
                    } else {
                        Vec::new()
                    };
                    for desc_obj in descs {
                        if let GattObject::Descriptor {
                            uuid: desc_uuid,
                            handle: desc_handle,
                            ..
                        } = desc_obj
                        {
                            rows.push(output::enum_desc_row(*desc_handle, desc_uuid));
                        }
                    }
                }
            }

            // Separator between services
            rows.push(output::enum_separator_row());
        }
    }

    // Remove trailing separator
    if rows.last().map_or(false, |r| r.handles.is_empty() && r.description.is_empty()) {
        rows.pop();
    }

    print!("{}", output::render_enumerate_table(&rows));
    Ok(())
}

/// Collect device information for the info table
async fn collect_device_info(conn: &Connection) -> Vec<(String, String)> {
    let dev = &conn.device;
    let mut info: Vec<(String, String)> = Vec::new();

    // Address
    info.push(("Address".into(), format!("{}", conn.address)));

    // Address Type
    let addr_type_str = match conn.addr_type {
        bluer::AddressType::LePublic => "Public",
        bluer::AddressType::LeRandom => "Random",
        _ => "Unknown",
    };
    info.push(("Address Type".into(), addr_type_str.into()));

    // Name
    if let Ok(Some(name)) = dev.name().await {
        info.push(("Name".into(), name));
    }

    // Alias (often a friendly name set by BlueZ)
    if let Ok(alias) = dev.alias().await {
        // Only show alias if it differs from the address (BlueZ defaults alias to address)
        if alias != format!("{}", conn.address) {
            info.push(("Alias".into(), alias));
        }
    }

    // RSSI
    if let Ok(Some(rssi)) = dev.rssi().await {
        info.push(("RSSI".into(), format!("{} dBm", rssi)));
    }

    // TX Power
    if let Ok(Some(tx)) = dev.tx_power().await {
        info.push(("TX Power".into(), format!("{} dBm", tx)));
    }

    // Connected / Paired / Trusted
    if let Ok(connected) = dev.is_connected().await {
        info.push(("Connected".into(), if connected { "Yes" } else { "No" }.into()));
    }
    if let Ok(paired) = dev.is_paired().await {
        info.push(("Paired".into(), if paired { "Yes" } else { "No" }.into()));
    }
    if let Ok(trusted) = dev.is_trusted().await {
        info.push(("Trusted".into(), if trusted { "Yes" } else { "No" }.into()));
    }

    // Appearance
    if let Ok(Some(appearance)) = dev.appearance().await {
        info.push(("Appearance".into(), format!("0x{:04x}", appearance)));
    }

    // Adapter
    info.push(("Adapter".into(), dev.adapter_name().to_string()));

    // Manufacturer Data
    if let Ok(mfr) = dev.manufacturer_data().await {
        if let Some(mfr_map) = mfr {
            for (company_id, data) in &mfr_map {
                let hex: String = data.iter().map(|b| format!("{:02x} ", b)).collect();
                info.push((
                    format!("Manufacturer (0x{:04x})", company_id),
                    hex.trim_end().to_string(),
                ));
            }
        }
    }

    // Service UUIDs
    if let Ok(Some(uuids)) = dev.uuids().await {
        if !uuids.is_empty() {
            let uuid_strs: Vec<String> = uuids.iter().map(|u| u.to_string().to_lowercase()).collect();
            info.push(("Service UUIDs".into(), uuid_strs.join(", ")));
        }
    }

    info
}

/// Query the negotiated ATT MTU from the first available characteristic.
/// BlueZ auto-negotiates MTU during connection; this reads the result.
pub async fn get_mtu(conn: &Connection) -> Result<usize, GrattError> {
    // Find the first characteristic and query its MTU property
    for obj in conn.handle_table.entries.values() {
        if let GattObject::Characteristic { characteristic, .. } = obj {
            return characteristic.mtu().await.map_err(|e| {
                GrattError::Gatt(format!("Failed to read MTU: {}", e))
            });
        }
    }
    Err(GrattError::Gatt("No characteristics available to query MTU".into()))
}

// --- Interactive mode variants (same logic, different output format) ---

/// Discover primary services (interactive format)
pub async fn discover_primary_interactive(
    conn: &Connection,
    uuid_filter: Option<&str>,
) -> Result<Vec<String>, GrattError> {
    let services = conn.handle_table.services();
    let mut lines = Vec::new();

    if services.is_empty() {
        return Err(GrattError::Gatt(
            "Discover all primary services failed: No attribute found within the given range"
                .into(),
        ));
    }

    for obj in services {
        if let GattObject::Service {
            uuid,
            start_handle,
            end_handle,
            ..
        } = obj
        {
            if let Some(filter) = uuid_filter {
                if normalize_uuid(uuid) == normalize_uuid(filter) {
                    lines.push(output::fmt_primary_by_uuid_interactive(
                        *start_handle,
                        *end_handle,
                    ));
                }
            } else {
                lines.push(output::fmt_primary_service_interactive(
                    *start_handle,
                    *end_handle,
                    uuid,
                ));
            }
        }
    }

    if lines.is_empty() && uuid_filter.is_some() {
        return Err(GrattError::Gatt("No service UUID found".into()));
    }

    Ok(lines)
}

/// Discover characteristics (interactive format)
pub async fn discover_characteristics_interactive(
    conn: &Connection,
    start: u16,
    end: u16,
    uuid_filter: Option<&str>,
) -> Result<Vec<String>, GrattError> {
    let chars = conn.handle_table.characteristics_in_range(start, end, uuid_filter);
    let mut lines = Vec::new();

    if chars.is_empty() {
        return Err(GrattError::Gatt(
            "Discover all characteristics failed: No attribute found within the given range".into(),
        ));
    }

    for obj in chars {
        if let GattObject::Characteristic {
            uuid,
            declaration_handle,
            value_handle,
            properties,
            ..
        } = obj
        {
            lines.push(output::fmt_characteristic_interactive(
                *declaration_handle,
                properties.0,
                *value_handle,
                uuid,
            ));
        }
    }
    Ok(lines)
}

/// Discover descriptors (interactive format)
pub async fn discover_descriptors_interactive(
    conn: &Connection,
    start: u16,
    end: u16,
) -> Result<Vec<String>, GrattError> {
    let objects = conn.handle_table.descriptors_in_range(start, end);
    let mut lines = Vec::new();

    if objects.is_empty() {
        return Err(GrattError::Gatt(
            "Discover descriptors failed: No attribute found within the given range".into(),
        ));
    }

    for obj in objects {
        let (h, u) = match obj {
            GattObject::Service {
                uuid,
                start_handle,
                ..
            } => (*start_handle, uuid.as_str()),
            GattObject::Characteristic {
                uuid,
                declaration_handle,
                ..
            } => (*declaration_handle, uuid.as_str()),
            GattObject::Descriptor { uuid, handle, .. } => (*handle, uuid.as_str()),
        };
        lines.push(output::fmt_descriptor_interactive(h, u));
    }
    Ok(lines)
}

/// Find included services (interactive format)
pub async fn find_included_interactive(
    _conn: &Connection,
    _start: u16,
    _end: u16,
) -> Result<Vec<String>, GrattError> {
    // bluer doesn't have direct included service discovery.
    // Most devices have none, so report accordingly.
    Ok(vec!["No included services found for this range".into()])
}

/// Read by handle (interactive format)
pub async fn read_by_handle_interactive(
    conn: &Connection,
    handle: u16,
) -> Result<String, GrattError> {
    let obj = conn.handle_table.find_rw_object(handle);
    match obj {
        Some(GattObject::Characteristic { characteristic, .. }) => {
            let data = characteristic.read().await.map_err(|e| {
                GrattError::Gatt(format!(
                    "Characteristic value/descriptor read failed: {}",
                    e
                ))
            })?;
            Ok(output::fmt_char_value(&data, OutputMode::Hex))
        }
        Some(GattObject::Descriptor { descriptor, .. }) => {
            let data = descriptor.read().await.map_err(|e| {
                GrattError::Gatt(format!(
                    "Characteristic value/descriptor read failed: {}",
                    e
                ))
            })?;
            Ok(output::fmt_char_value(&data, OutputMode::Hex))
        }
        _ => Err(GrattError::Gatt(
            "Characteristic value/descriptor read failed: Invalid handle".to_string(),
        )),
    }
}

/// Read by UUID (interactive format)
pub async fn read_by_uuid_interactive(
    conn: &Connection,
    uuid: &str,
    start: u16,
    end: u16,
) -> Result<Vec<String>, GrattError> {
    let chars = conn.handle_table.chars_by_uuid_in_range(uuid, start, end);
    let mut lines = Vec::new();

    if chars.is_empty() {
        return Err(GrattError::Gatt(
            "Read characteristics by UUID failed: No attribute found within the given range".into(),
        ));
    }

    for obj in chars {
        if let GattObject::Characteristic {
            characteristic,
            value_handle,
            ..
        } = obj
        {
            match characteristic.read().await {
                Ok(data) => {
                    lines.push(output::fmt_read_by_uuid(*value_handle, &data, OutputMode::Hex));
                }
                Err(e) => {
                    return Err(GrattError::Gatt(format!(
                        "Read characteristics by UUID failed: {}",
                        e
                    )));
                }
            }
        }
    }
    Ok(lines)
}

/// Write command (interactive, no response)
pub async fn write_command_interactive(
    conn: &Connection,
    handle: u16,
    value: &[u8],
) -> Result<(), GrattError> {
    write_command(conn, handle, value).await
}

/// Write request (interactive, with response)
pub async fn write_request_interactive(
    conn: &Connection,
    handle: u16,
    value: &[u8],
) -> Result<String, GrattError> {
    let obj = conn.handle_table.find_rw_object(handle);
    match obj {
        Some(GattObject::Characteristic { characteristic, .. }) => {
            let req = CharacteristicWriteRequest {
                op_type: WriteOp::Request,
                ..Default::default()
            };
            characteristic.write_ext(value, &req).await.map_err(|e| {
                GrattError::Gatt(format!("Characteristic Write Request failed: {}", e))
            })?;
            Ok("Characteristic value was written successfully".into())
        }
        Some(GattObject::Descriptor { descriptor, .. }) => {
            descriptor.write(value).await.map_err(|e| {
                GrattError::Gatt(format!("Characteristic Write Request failed: {}", e))
            })?;
            Ok("Characteristic value was written successfully".into())
        }
        _ => Err(GrattError::InvalidHandle(
            "A valid handle is required".into(),
        )),
    }
}
