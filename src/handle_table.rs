use bluer::gatt::remote::{Characteristic, Descriptor, Service};
use std::collections::BTreeMap;

/// GATT characteristic property flags (matches BlueZ ATT property bits)
#[derive(Debug, Clone, Copy)]
pub struct CharProps(pub u8);

impl CharProps {
    pub const BROADCAST: u8 = 0x01;
    pub const READ: u8 = 0x02;
    pub const WRITE_WITHOUT_RESP: u8 = 0x04;
    pub const WRITE: u8 = 0x08;
    pub const NOTIFY: u8 = 0x10;
    pub const INDICATE: u8 = 0x20;
    pub const AUTH_SIGNED_WRITE: u8 = 0x40;
    pub const EXTENDED: u8 = 0x80;

    pub fn from_bluer_flags(flags: &bluer::gatt::CharacteristicFlags) -> Self {
        let mut bits: u8 = 0;
        if flags.broadcast { bits |= Self::BROADCAST; }
        if flags.read { bits |= Self::READ; }
        if flags.write_without_response { bits |= Self::WRITE_WITHOUT_RESP; }
        if flags.write { bits |= Self::WRITE; }
        if flags.notify { bits |= Self::NOTIFY; }
        if flags.indicate { bits |= Self::INDICATE; }
        if flags.authenticated_signed_writes { bits |= Self::AUTH_SIGNED_WRITE; }
        if flags.extended_properties { bits |= Self::EXTENDED; }
        CharProps(bits)
    }

    /// Convert property bits to a human-readable string like "READ WRITE NOTIFY"
    pub fn to_string(&self) -> String {
        let mut parts = Vec::new();
        if self.0 & Self::BROADCAST != 0 { parts.push("BROADCAST"); }
        if self.0 & Self::READ != 0 { parts.push("READ"); }
        if self.0 & Self::WRITE_WITHOUT_RESP != 0 { parts.push("WRITE NO RESP"); }
        if self.0 & Self::WRITE != 0 { parts.push("WRITE"); }
        if self.0 & Self::NOTIFY != 0 { parts.push("NOTIFY"); }
        if self.0 & Self::INDICATE != 0 { parts.push("INDICATE"); }
        if self.0 & Self::AUTH_SIGNED_WRITE != 0 { parts.push("AUTH SIGNED WRITE"); }
        if self.0 & Self::EXTENDED != 0 { parts.push("EXTENDED"); }
        parts.join(" ")
    }
}

/// Represents a GATT object mapped by its ATT handle
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GattObject {
    Service {
        service: Service,
        uuid: String,
        start_handle: u16,
        end_handle: u16,
    },
    Characteristic {
        characteristic: Characteristic,
        uuid: String,
        declaration_handle: u16,
        value_handle: u16,
        properties: CharProps,
    },
    Descriptor {
        descriptor: Descriptor,
        uuid: String,
        handle: u16,
    },
}

/// Handle table mapping ATT handles to bluer GATT objects
#[derive(Debug)]
pub struct HandleTable {
    pub entries: BTreeMap<u16, GattObject>,
}

impl HandleTable {
    pub fn new() -> Self {
        HandleTable {
            entries: BTreeMap::new(),
        }
    }

    /// Build the handle table by enumerating all services, characteristics, and descriptors.
    /// Uses D-Bus properties to get the actual ATT handles from BlueZ.
    ///
    /// End group handles are computed as the highest attribute handle belonging to
    /// each service, with the last service ending at 0xffff (matching the ATT Read
    /// By Group Type Response that the original gatttool uses).
    pub async fn build(device: &bluer::Device) -> Result<Self, bluer::Error> {
        let mut table = HandleTable::new();

        struct ServiceInfo {
            service: Service,
            uuid: String,
            start_handle: u16,
            max_child_handle: u16,
        }

        let mut service_infos: Vec<ServiceInfo> = Vec::new();

        for service in device.services().await? {
            let uuid = format_uuid128(&service.uuid().await?);
            let svc_handle = service.id();
            let mut max_handle = svc_handle;

            for char in service.characteristics().await? {
                let char_uuid = format_uuid128(&char.uuid().await?);
                let flags = char.flags().await?;
                let props = CharProps::from_bluer_flags(&flags);

                let decl_handle = char.id();
                let value_handle = decl_handle + 1;

                if value_handle > max_handle {
                    max_handle = value_handle;
                }

                table.entries.insert(
                    decl_handle,
                    GattObject::Characteristic {
                        characteristic: char.clone(),
                        uuid: char_uuid,
                        declaration_handle: decl_handle,
                        value_handle,
                        properties: props,
                    },
                );

                for desc in char.descriptors().await? {
                    let desc_uuid = format_uuid128(&desc.uuid().await?);
                    let desc_handle = desc.id();

                    if desc_handle > max_handle {
                        max_handle = desc_handle;
                    }

                    table.entries.insert(
                        desc_handle,
                        GattObject::Descriptor {
                            descriptor: desc,
                            uuid: desc_uuid,
                            handle: desc_handle,
                        },
                    );
                }
            }

            service_infos.push(ServiceInfo {
                service,
                uuid,
                start_handle: svc_handle,
                max_child_handle: max_handle,
            });
        }

        // Sort services by start handle so we can identify the last one
        service_infos.sort_by_key(|s| s.start_handle);

        // Insert services with computed end handles.
        // Last service gets end_handle = 0xffff (ATT protocol convention).
        // Other services use their highest child handle as the end handle.
        let last_idx = service_infos.len().saturating_sub(1);
        for (i, info) in service_infos.iter().enumerate() {
            let end_handle = if i == last_idx {
                0xffff
            } else {
                info.max_child_handle
            };

            table.entries.insert(
                info.start_handle,
                GattObject::Service {
                    service: info.service.clone(),
                    uuid: info.uuid.clone(),
                    start_handle: info.start_handle,
                    end_handle,
                },
            );
        }

        Ok(table)
    }

    /// Look up a GATT object by handle
    #[allow(dead_code)]
    pub fn by_handle(&self, handle: u16) -> Option<&GattObject> {
        self.entries.get(&handle)
    }

    /// Find the characteristic whose value_handle matches
    pub fn char_by_value_handle(&self, handle: u16) -> Option<&GattObject> {
        self.entries.values().find(|obj| {
            matches!(obj, GattObject::Characteristic { value_handle, .. } if *value_handle == handle)
        })
    }

    /// Find a readable/writable object by handle. For a value_handle, returns the characteristic.
    /// For a descriptor handle, returns the descriptor. Also checks direct handle match.
    pub fn find_rw_object(&self, handle: u16) -> Option<&GattObject> {
        // First check if this is a value handle of a characteristic
        if let Some(obj) = self.char_by_value_handle(handle) {
            return Some(obj);
        }
        // Then check direct handle match (descriptor, or characteristic declaration)
        self.entries.get(&handle)
    }

    /// Get all services
    pub fn services(&self) -> Vec<&GattObject> {
        self.entries
            .values()
            .filter(|obj| matches!(obj, GattObject::Service { .. }))
            .collect()
    }

    /// Get characteristics within a handle range, optionally filtered by UUID
    pub fn characteristics_in_range(
        &self,
        start: u16,
        end: u16,
        uuid_filter: Option<&str>,
    ) -> Vec<&GattObject> {
        self.entries
            .range(start..=end)
            .map(|(_, obj)| obj)
            .filter(|obj| {
                if let GattObject::Characteristic { uuid, .. } = obj {
                    if let Some(filter) = uuid_filter {
                        uuid_matches(uuid, filter)
                    } else {
                        true
                    }
                } else {
                    false
                }
            })
            .collect()
    }

    /// Get all entries (descriptors + characteristics + services) in a handle range
    pub fn descriptors_in_range(&self, start: u16, end: u16) -> Vec<&GattObject> {
        self.entries
            .range(start..=end)
            .map(|(_, obj)| obj)
            .collect()
    }

    /// Find characteristics matching a UUID for read-by-uuid
    pub fn chars_by_uuid_in_range(
        &self,
        uuid_filter: &str,
        start: u16,
        end: u16,
    ) -> Vec<&GattObject> {
        self.entries
            .range(start..=end)
            .map(|(_, obj)| obj)
            .filter(|obj| {
                if let GattObject::Characteristic { uuid, .. } = obj {
                    uuid_matches(uuid, uuid_filter)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Find all characteristics that support notification or indication
    pub fn notifiable_characteristics(&self) -> Vec<&GattObject> {
        self.entries
            .values()
            .filter(|obj| {
                if let GattObject::Characteristic { properties, .. } = obj {
                    (properties.0 & CharProps::NOTIFY) != 0
                        || (properties.0 & CharProps::INDICATE) != 0
                } else {
                    false
                }
            })
            .collect()
    }
}

/// Format a bluer UUID to match gatttool's output format.
/// Short UUIDs (16-bit standard) are formatted as full 128-bit with the standard base.
fn format_uuid128(uuid: &uuid::Uuid) -> String {
    // BlueZ outputs all UUIDs as their string form
    // For standard 16-bit UUIDs, they appear as 00001800-0000-1000-8000-00805f9b34fb
    uuid.to_string().to_lowercase()
}

/// Normalize a UUID string for comparison.
/// Handles short UUID forms like "1800", "0x1800", or full 128-bit.
pub fn normalize_uuid(input: &str) -> String {
    let input = input.trim().to_lowercase();
    let input = input
        .strip_prefix("0x")
        .unwrap_or(&input)
        .to_string();

    // If it looks like a short (16-bit) UUID, expand to 128-bit base
    if input.len() <= 4 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        let short = u16::from_str_radix(&input, 16).unwrap_or(0);
        format!(
            "{:08x}-0000-1000-8000-00805f9b34fb",
            short as u32
        )
    } else if input.len() <= 8 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        // 32-bit UUID
        let int_val = u32::from_str_radix(&input, 16).unwrap_or(0);
        format!(
            "{:08x}-0000-1000-8000-00805f9b34fb",
            int_val
        )
    } else {
        input
    }
}

/// Check if a stored UUID matches a filter UUID (handling short/long form)
fn uuid_matches(stored: &str, filter: &str) -> bool {
    let norm_stored = normalize_uuid(stored);
    let norm_filter = normalize_uuid(filter);
    norm_stored == norm_filter
}
