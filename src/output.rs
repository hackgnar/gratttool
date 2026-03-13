use colored::{ColoredString, Colorize, CustomColor};

// --- Catppuccin Mocha palette ---
const ROSEWATER: CustomColor = CustomColor { r: 245, g: 224, b: 220 };
const FLAMINGO: CustomColor  = CustomColor { r: 242, g: 205, b: 205 };
const PINK: CustomColor      = CustomColor { r: 245, g: 194, b: 231 };
const MAUVE: CustomColor     = CustomColor { r: 203, g: 166, b: 247 };
const RED: CustomColor       = CustomColor { r: 243, g: 139, b: 168 };
const PEACH: CustomColor     = CustomColor { r: 250, g: 179, b: 135 };
const YELLOW: CustomColor    = CustomColor { r: 249, g: 226, b: 175 };
const GREEN: CustomColor     = CustomColor { r: 166, g: 227, b: 161 };
const TEAL: CustomColor      = CustomColor { r: 148, g: 226, b: 213 };
const SKY: CustomColor       = CustomColor { r: 137, g: 220, b: 235 };
const SAPPHIRE: CustomColor  = CustomColor { r: 116, g: 199, b: 236 };
const BLUE: CustomColor      = CustomColor { r: 137, g: 180, b: 250 };
const LAVENDER: CustomColor  = CustomColor { r: 180, g: 190, b: 254 };
const TEXT: CustomColor       = CustomColor { r: 205, g: 214, b: 244 };
const SUBTEXT0: CustomColor  = CustomColor { r: 166, g: 173, b: 200 };
const OVERLAY0: CustomColor  = CustomColor { r: 108, g: 112, b: 134 };
const SURFACE1: CustomColor  = CustomColor { r: 69, g: 71, b: 90 };

fn ctp(s: &str, c: CustomColor) -> ColoredString { s.custom_color(c) }
fn ctp_bold(s: &str, c: CustomColor) -> ColoredString { s.custom_color(c).bold() }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OutputMode {
    #[default]
    Hex,       // "66 6c 61 67 ..." (default, backward compatible)
    Ascii,     // "flag_value" (dot for non-printable)
    HexAscii,  // "66 6c 61 67 ...  flag_value"
}

/// Format a byte slice according to the selected output mode
pub fn format_value(data: &[u8], mode: OutputMode) -> String {
    match mode {
        OutputMode::Hex => format_hex_bytes(data),
        OutputMode::Ascii => format_data_ascii(data),
        OutputMode::HexAscii => {
            if data.is_empty() {
                return String::new();
            }
            let hex = format_hex_bytes(data);
            let ascii = format_data_ascii(data);
            format!("{}  {}", hex.trim_end(), ascii)
        }
    }
}

/// Get terminal width, defaulting to 120
fn term_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(120)
}

/// Format a byte slice as space-separated hex pairs: "0a 1b 2c "
pub fn format_hex_bytes(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 3);
    for b in data {
        s.push_str(&format!("{:02x} ", b));
    }
    s
}

/// Format a UUID string, converting 128-bit UUIDs to lowercase
#[allow(dead_code)]
pub fn format_uuid(uuid_str: &str) -> String {
    uuid_str.to_lowercase()
}

// --- Non-interactive output (uses '=') ---

pub fn fmt_primary_service(start: u16, end: u16, uuid: &str) -> String {
    format!(
        "attr handle = 0x{:04x}, end grp handle = 0x{:04x} uuid: {}",
        start, end, uuid
    )
}

pub fn fmt_primary_by_uuid(start: u16, end: u16) -> String {
    format!("Starting handle: {:04x} Ending handle: {:04x}", start, end)
}

pub fn fmt_characteristic(handle: u16, properties: u8, value_handle: u16, uuid: &str) -> String {
    format!(
        "handle = 0x{:04x}, char properties = 0x{:02x}, char value handle = 0x{:04x}, uuid = {}",
        handle, properties, value_handle, uuid
    )
}

pub fn fmt_descriptor(handle: u16, uuid: &str) -> String {
    format!("handle = 0x{:04x}, uuid = {}", handle, uuid)
}

pub fn fmt_char_value(data: &[u8], mode: OutputMode) -> String {
    format!("Characteristic value/descriptor: {}", format_value(data, mode))
}

pub fn fmt_read_by_uuid(handle: u16, data: &[u8], mode: OutputMode) -> String {
    format!(
        "handle: 0x{:04x} \t value: {}",
        handle,
        format_value(data, mode)
    )
}

pub fn fmt_notification(handle: u16, data: &[u8], mode: OutputMode) -> String {
    format!(
        "Notification handle = 0x{:04x} value: {}",
        handle,
        format_value(data, mode)
    )
}

pub fn fmt_indication(handle: u16, data: &[u8], mode: OutputMode) -> String {
    format!(
        "Indication   handle = 0x{:04x} value: {}",
        handle,
        format_value(data, mode)
    )
}

// --- Interactive output (uses ':') ---

pub fn fmt_primary_service_interactive(start: u16, end: u16, uuid: &str) -> String {
    format!(
        "attr handle: 0x{:04x}, end grp handle: 0x{:04x} uuid: {}",
        start, end, uuid
    )
}

pub fn fmt_primary_by_uuid_interactive(start: u16, end: u16) -> String {
    format!(
        "Starting handle: 0x{:04x} Ending handle: 0x{:04x}",
        start, end
    )
}

pub fn fmt_characteristic_interactive(
    handle: u16,
    properties: u8,
    value_handle: u16,
    uuid: &str,
) -> String {
    format!(
        "handle: 0x{:04x}, char properties: 0x{:02x}, char value handle: 0x{:04x}, uuid: {}",
        handle, properties, value_handle, uuid
    )
}

pub fn fmt_descriptor_interactive(handle: u16, uuid: &str) -> String {
    format!("handle: 0x{:04x}, uuid: {}", handle, uuid)
}

#[allow(dead_code)]
pub fn fmt_included_interactive(
    handle: u16,
    start_handle: u16,
    end_handle: u16,
    uuid: &str,
) -> String {
    format!(
        "handle: 0x{:04x}, start handle: 0x{:04x}, end handle: 0x{:04x} uuid: {}",
        handle, start_handle, end_handle, uuid
    )
}

// --- Device info table output ---

/// Render a device info table (key-value pairs) with Catppuccin-styled box-drawing
pub fn render_device_info_table(info: &[(String, String)]) -> String {
    if info.is_empty() {
        return String::new();
    }

    let tw = term_width();

    // Find the address for the title
    let title_text = info
        .iter()
        .find(|(k, _)| k == "Address")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let title = format!(" {} ", ctp_bold(&title_text, PEACH));
    let title_vis_len = visible_len(&title);

    // Compute column widths
    let mut key_width = 0usize;
    let mut val_width = 0usize;
    for (k, v) in info {
        key_width = key_width.max(k.len());
        val_width = val_width.max(visible_len(v));
    }

    // inner = " key  | val "  =>  1 + key + 1 + 3 + 1 + val + 1 = key + val + 7
    // but we only pad key/val, borders are separate
    // layout: "│ <key padded> │ <val padded> │" inner_content = key_w + 3 + val_w
    let inner_content = key_width + 3 + val_width;
    let total_inner = inner_content.max(title_vis_len).min(tw.saturating_sub(2));

    // Recalculate val_width to fill available space
    let val_width = total_inner.saturating_sub(key_width + 3);

    let mut out = String::new();
    let border = ctp("─", SURFACE1);
    let vert = ctp("│", SURFACE1);

    // Top border with title
    out.push_str(&ctp("┌", SURFACE1).to_string());
    if !title_text.is_empty() {
        out.push_str(&title);
        let remaining = total_inner + 2 - title_vis_len;
        for _ in 0..remaining { out.push_str(&border.to_string()); }
    } else {
        for _ in 0..total_inner + 2 { out.push_str(&border.to_string()); }
    }
    out.push_str(&ctp("┐", SURFACE1).to_string());
    out.push('\n');

    // Rows
    for (k, v) in info {
        out.push_str(&vert.to_string());
        out.push(' ');
        out.push_str(&ctp_bold(k, MAUVE).to_string());
        out.push_str(&" ".repeat(key_width - k.len()));
        out.push(' ');
        out.push_str(&vert.to_string());
        out.push(' ');

        // Color specific values
        let colored_val = colorize_info_value(k, v);
        let vlen = visible_len(&colored_val);
        if vlen <= val_width {
            out.push_str(&colored_val);
            out.push_str(&" ".repeat(val_width - vlen));
        } else {
            // Wrap long values across multiple lines
            let chunks = wrap_plain_text(v, val_width);
            for (ci, chunk) in chunks.iter().enumerate() {
                if ci > 0 {
                    // continuation line
                    out.push_str(&vert.to_string());
                    out.push(' ');
                    out.push_str(&" ".repeat(key_width));
                    out.push(' ');
                    out.push_str(&vert.to_string());
                    out.push(' ');
                }
                let cv = colorize_info_value(k, chunk);
                let cvlen = visible_len(&cv);
                out.push_str(&cv);
                out.push_str(&" ".repeat(val_width.saturating_sub(cvlen)));
                out.push(' ');
                out.push_str(&vert.to_string());
                out.push('\n');
            }
            continue;
        }
        out.push(' ');
        out.push_str(&vert.to_string());
        out.push('\n');
    }

    // Bottom border
    out.push_str(&ctp("└", SURFACE1).to_string());
    for _ in 0..total_inner + 2 { out.push_str(&border.to_string()); }
    out.push_str(&ctp("┘", SURFACE1).to_string());
    out.push('\n');

    out
}

/// Colorize device info values based on key
fn colorize_info_value(key: &str, val: &str) -> String {
    match key {
        "Address" => ctp_bold(val, ROSEWATER).to_string(),
        "Name" | "Alias" => ctp_bold(val, YELLOW).to_string(),
        "RSSI" | "TX Power" => ctp(val, SKY).to_string(),
        "Connected" => {
            if val == "Yes" { ctp_bold(val, GREEN).to_string() }
            else { ctp(val, RED).to_string() }
        }
        "Paired" | "Trusted" => {
            if val == "Yes" { ctp(val, GREEN).to_string() }
            else { ctp(val, SUBTEXT0).to_string() }
        }
        "Address Type" | "Adapter" => ctp(val, LAVENDER).to_string(),
        _ if key.starts_with("Manufacturer") => ctp(val, FLAMINGO).to_string(),
        "Service UUIDs" => ctp(val, TEAL).to_string(),
        "Appearance" => ctp(val, PINK).to_string(),
        _ => ctp(val, TEXT).to_string(),
    }
}

/// Wrap a plain string into chunks of max `width` characters
fn wrap_plain_text(s: &str, width: usize) -> Vec<String> {
    if width == 0 { return vec![s.to_string()]; }
    let mut chunks = Vec::new();
    let mut remaining = s;
    while !remaining.is_empty() {
        if remaining.len() <= width {
            chunks.push(remaining.to_string());
            break;
        }
        // Try to break at a space or comma
        let break_at = remaining[..width]
            .rfind(|c: char| c == ' ' || c == ',')
            .map(|p| p + 1)
            .unwrap_or(width);
        chunks.push(remaining[..break_at].to_string());
        remaining = &remaining[break_at..];
    }
    if chunks.is_empty() { chunks.push(String::new()); }
    chunks
}

// --- Enumerate table output ---

/// Row data for the enumerate table
pub struct EnumRow {
    pub handles: String,
    pub description: String,
    pub properties: String,
    pub data: String,
    pub ascii: String,
}

/// Format raw bytes as hex string for the Data column
fn format_data_hex(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    format_hex_bytes(data).trim_end().to_string()
}

/// Format raw bytes as ASCII for the ASCII column (non-printable shown as '.')
pub fn format_data_ascii(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    data.iter()
        .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
        .collect()
}

/// Colorize a properties string with Catppuccin colors
fn colorize_props(props: &str) -> String {
    if props.is_empty() { return String::new(); }
    props.split(' ')
        .map(|word| {
            match word {
                "READ" => ctp_bold(word, GREEN).to_string(),
                "WRITE" => ctp_bold(word, PEACH).to_string(),
                "NO" | "RESP" => ctp_bold(word, PEACH).to_string(),
                "NOTIFY" => ctp_bold(word, YELLOW).to_string(),
                "INDICATE" => ctp_bold(word, RED).to_string(),
                "BROADCAST" => ctp(word, SKY).to_string(),
                "EXTENDED" => ctp(word, LAVENDER).to_string(),
                "AUTH" | "SIGNED" => ctp(word, PINK).to_string(),
                _ => ctp(word, TEXT).to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render the enumerate table with box-drawing characters and Catppuccin colors.
/// Data and ASCII columns wrap across multiple lines instead of truncating.
pub fn render_enumerate_table(rows: &[EnumRow]) -> String {
    let tw = term_width();

    // Fixed columns: Handles, Service > Characteristics, Properties
    // These size to content. Data and ASCII get the remaining space.
    let headers = ["Handles", "Service > Characteristics", "Properties", "Data", "ASCII"];
    let ncols = 5;

    // Measure fixed column widths from content
    let mut w_handles = headers[0].len();
    let mut w_desc = headers[1].len();
    let mut w_props = headers[2].len();
    let mut max_data = headers[3].len();
    let mut max_ascii = headers[4].len();

    for row in rows {
        w_handles = w_handles.max(visible_len(&row.handles));
        w_desc = w_desc.max(visible_len(&row.description));
        w_props = w_props.max(visible_len(&row.properties));
        max_data = max_data.max(row.data.len());
        max_ascii = max_ascii.max(row.ascii.len());
    }

    // Calculate overhead: ncols borders + padding (2 per col) + 1 trailing border
    // "│ col │ col │ col │ col │ col │"
    let overhead = ncols + 1 + ncols * 2; // 6 borders + 10 padding = 16
    let fixed_used = w_handles + w_desc + w_props + overhead;

    // Remaining space for data + ascii
    let remaining = tw.saturating_sub(fixed_used);

    // Split remaining: give ASCII ~1/4 (it's compact), Data ~3/4
    // But never less than header width, and never more than content needs
    let w_ascii = max_ascii.min(remaining / 4).max(headers[4].len()).max(5);
    let w_data = remaining.saturating_sub(w_ascii).max(headers[3].len()).max(8);

    let widths = [w_handles, w_desc, w_props, w_data, w_ascii];

    let mut out = String::new();
    let b = ctp("─", SURFACE1);
    let v = ctp("│", SURFACE1);

    // Top border
    out.push_str(&ctp("┌", SURFACE1).to_string());
    for (i, w) in widths.iter().enumerate() {
        for _ in 0..w + 2 { out.push_str(&b.to_string()); }
        out.push_str(&(if i < ncols - 1 { ctp("┬", SURFACE1) } else { ctp("┐", SURFACE1) }).to_string());
    }
    out.push('\n');

    // Header row
    out.push_str(&v.to_string());
    for (i, h) in headers.iter().enumerate() {
        out.push(' ');
        out.push_str(&ctp_bold(h, MAUVE).to_string());
        out.push_str(&" ".repeat(widths[i] - h.len()));
        out.push(' ');
        out.push_str(&v.to_string());
    }
    out.push('\n');

    // Header separator
    out.push_str(&ctp("├", SURFACE1).to_string());
    for (i, w) in widths.iter().enumerate() {
        for _ in 0..w + 2 { out.push_str(&b.to_string()); }
        out.push_str(&(if i < ncols - 1 { ctp("┼", SURFACE1) } else { ctp("┤", SURFACE1) }).to_string());
    }
    out.push('\n');

    // Data rows - with multi-line wrapping for data/ascii
    for row in rows {
        // Colorize fields
        let c_handles = if row.handles.contains("->") {
            ctp_bold(&row.handles, SAPPHIRE).to_string()
        } else if !row.handles.is_empty() {
            ctp(&row.handles, SKY).to_string()
        } else {
            String::new()
        };

        let c_desc = if visible_len(&row.description) > 0 && !row.description.starts_with(' ') {
            // Already colored (service)
            row.description.clone()
        } else if row.description.starts_with("    ") {
            // Descriptor (4-space indent)
            let trimmed = row.description.trim_start();
            format!("    {}", ctp(trimmed, OVERLAY0))
        } else if row.description.starts_with("  ") {
            // Characteristic (2-space indent)
            let trimmed = row.description.trim_start();
            format!("  {}", ctp(trimmed, TEAL))
        } else {
            row.description.clone()
        };

        let c_props = colorize_props(&row.properties);

        // Wrap data and ascii into lines that fit their column widths
        let data_lines = wrap_plain_text(&row.data, w_data);
        let ascii_lines = wrap_plain_text(&row.ascii, w_ascii);
        let num_lines = data_lines.len().max(ascii_lines.len()).max(1);

        for line_idx in 0..num_lines {
            out.push_str(&v.to_string());

            // Handles - only on first line
            out.push(' ');
            if line_idx == 0 {
                let hlen = visible_len(&c_handles);
                out.push_str(&c_handles);
                out.push_str(&" ".repeat(w_handles.saturating_sub(hlen)));
            } else {
                out.push_str(&" ".repeat(w_handles));
            }
            out.push(' ');
            out.push_str(&v.to_string());

            // Description - only on first line
            out.push(' ');
            if line_idx == 0 {
                let dlen = visible_len(&c_desc);
                out.push_str(&c_desc);
                out.push_str(&" ".repeat(w_desc.saturating_sub(dlen)));
            } else {
                out.push_str(&" ".repeat(w_desc));
            }
            out.push(' ');
            out.push_str(&v.to_string());

            // Properties - only on first line
            out.push(' ');
            if line_idx == 0 {
                let plen = visible_len(&c_props);
                out.push_str(&c_props);
                out.push_str(&" ".repeat(w_props.saturating_sub(plen)));
            } else {
                out.push_str(&" ".repeat(w_props));
            }
            out.push(' ');
            out.push_str(&v.to_string());

            // Data
            out.push(' ');
            let data_chunk = data_lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");
            let c_data = if !data_chunk.is_empty() {
                ctp(data_chunk, FLAMINGO).to_string()
            } else {
                String::new()
            };
            let data_vlen = visible_len(&c_data);
            out.push_str(&c_data);
            out.push_str(&" ".repeat(w_data.saturating_sub(data_vlen)));
            out.push(' ');
            out.push_str(&v.to_string());

            // ASCII
            out.push(' ');
            let ascii_chunk = ascii_lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");
            let c_ascii = if !ascii_chunk.is_empty() {
                ctp_bold(ascii_chunk, YELLOW).to_string()
            } else {
                String::new()
            };
            let ascii_vlen = visible_len(&c_ascii);
            out.push_str(&c_ascii);
            out.push_str(&" ".repeat(w_ascii.saturating_sub(ascii_vlen)));
            out.push(' ');
            out.push_str(&v.to_string());

            out.push('\n');
        }
    }

    // Bottom border
    out.push_str(&ctp("└", SURFACE1).to_string());
    for (i, w) in widths.iter().enumerate() {
        for _ in 0..w + 2 { out.push_str(&b.to_string()); }
        out.push_str(&(if i < ncols - 1 { ctp("┴", SURFACE1) } else { ctp("┘", SURFACE1) }).to_string());
    }
    out.push('\n');

    out
}

/// Get the visible length of a string (excluding ANSI escape codes)
fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            len += 1;
        }
    }
    len
}

/// Format a service row for the enumerate table
pub fn enum_service_row(start: u16, end: u16, uuid: &str) -> EnumRow {
    EnumRow {
        handles: format!("{:04x} -> {:04x}", start, end),
        description: ctp_bold(uuid, BLUE).to_string(),
        properties: String::new(),
        data: String::new(),
        ascii: String::new(),
    }
}

/// Format a characteristic row for the enumerate table
pub fn enum_char_row(value_handle: u16, uuid: &str, props_str: &str, data: Option<&[u8]>) -> EnumRow {
    let (data_str, ascii_str) = match data {
        Some(d) => (format_data_hex(d), format_data_ascii(d)),
        None => (String::new(), String::new()),
    };

    EnumRow {
        handles: format!("     {:04x}", value_handle),
        description: format!("  {}", uuid),
        properties: props_str.to_string(),
        data: data_str,
        ascii: ascii_str,
    }
}

/// Format a descriptor row for the enumerate table
pub fn enum_desc_row(handle: u16, uuid: &str) -> EnumRow {
    EnumRow {
        handles: format!("     {:04x}", handle),
        description: format!("    {}", uuid),
        properties: String::new(),
        data: String::new(),
        ascii: String::new(),
    }
}

/// Format an empty separator row
pub fn enum_separator_row() -> EnumRow {
    EnumRow {
        handles: String::new(),
        description: String::new(),
        properties: String::new(),
        data: String::new(),
        ascii: String::new(),
    }
}

// --- Scan results table output ---

/// Render a scan results table with Catppuccin-styled box-drawing
pub fn render_scan_table(devices: &[crate::scan::ScannedDevice]) -> String {
    let tw = term_width();
    let headers = ["Address", "Type", "RSSI", "Name"];
    let ncols = headers.len();

    // Measure column widths from content
    let mut w: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for dev in devices {
        w[0] = w[0].max(17); // MAC address is always 17 chars
        let type_str = format_addr_type(dev.addr_type);
        w[1] = w[1].max(type_str.len());
        let rssi_str = dev.rssi.map(|r| format!("{} dBm", r)).unwrap_or_else(|| "n/a".into());
        w[2] = w[2].max(rssi_str.len());
        let name = dev.name.as_deref().unwrap_or("");
        w[3] = w[3].max(name.len());
    }

    // Cap name column to fit terminal
    let overhead = ncols + 1 + ncols * 2; // borders + padding
    let fixed = w[0] + w[1] + w[2] + overhead;
    let max_name = tw.saturating_sub(fixed).max(headers[3].len()).max(8);
    w[3] = w[3].min(max_name);

    let mut out = String::new();
    let b = ctp("─", SURFACE1);
    let v = ctp("│", SURFACE1);

    // Title
    let title = format!(" {} ({} devices) ", ctp_bold("Scan Results", PEACH), devices.len());
    let title_vis = visible_len(&title);

    let total_inner: usize = w.iter().sum::<usize>() + (ncols - 1) * 3 + 2;

    // Top border with title
    out.push_str(&ctp("┌", SURFACE1).to_string());
    out.push_str(&title);
    let remaining = total_inner.saturating_sub(title_vis);
    for _ in 0..remaining { out.push_str(&b.to_string()); }
    out.push_str(&ctp("┐", SURFACE1).to_string());
    out.push('\n');

    // Header row
    out.push_str(&v.to_string());
    for (i, h) in headers.iter().enumerate() {
        out.push(' ');
        out.push_str(&ctp_bold(h, MAUVE).to_string());
        out.push_str(&" ".repeat(w[i].saturating_sub(h.len())));
        out.push(' ');
        if i < ncols - 1 { out.push_str(&v.to_string()); }
    }
    out.push_str(&v.to_string());
    out.push('\n');

    // Header separator
    out.push_str(&ctp("├", SURFACE1).to_string());
    for (i, width) in w.iter().enumerate() {
        for _ in 0..width + 2 { out.push_str(&b.to_string()); }
        out.push_str(&(if i < ncols - 1 { ctp("┼", SURFACE1) } else { ctp("┤", SURFACE1) }).to_string());
    }
    out.push('\n');

    // Sort devices by RSSI (strongest first), then by address
    let mut sorted: Vec<&crate::scan::ScannedDevice> = devices.iter().collect();
    sorted.sort_by(|a, b| {
        b.rssi.unwrap_or(i16::MIN).cmp(&a.rssi.unwrap_or(i16::MIN))
            .then_with(|| a.address.cmp(&b.address))
    });

    // Data rows
    for dev in &sorted {
        let addr_str = format!("{}", dev.address);
        let type_str = format_addr_type(dev.addr_type);
        let rssi_str = dev.rssi.map(|r| format!("{} dBm", r)).unwrap_or_else(|| "n/a".into());
        let name = dev.name.as_deref().unwrap_or("");
        // Truncate name if too long
        let name_display = if name.len() > w[3] {
            &name[..w[3]]
        } else {
            name
        };

        let fields: Vec<String> = vec![addr_str, type_str, rssi_str, name_display.to_string()];

        out.push_str(&v.to_string());
        for (i, field) in fields.iter().enumerate() {
            out.push(' ');
            let colored = match i {
                0 => ctp_bold(field, ROSEWATER).to_string(),
                1 => ctp(field, LAVENDER).to_string(),
                2 => colorize_rssi(field, dev.rssi),
                3 => {
                    if field.is_empty() {
                        ctp("(unknown)", OVERLAY0).to_string()
                    } else {
                        ctp_bold(field, YELLOW).to_string()
                    }
                }
                _ => field.clone(),
            };
            let vlen = visible_len(&colored);
            out.push_str(&colored);
            out.push_str(&" ".repeat(w[i].saturating_sub(vlen)));
            out.push(' ');
            if i < ncols - 1 { out.push_str(&v.to_string()); }
        }
        out.push_str(&v.to_string());
        out.push('\n');
    }

    // Bottom border
    out.push_str(&ctp("└", SURFACE1).to_string());
    for (i, width) in w.iter().enumerate() {
        for _ in 0..width + 2 { out.push_str(&b.to_string()); }
        out.push_str(&(if i < ncols - 1 { ctp("┴", SURFACE1) } else { ctp("┘", SURFACE1) }).to_string());
    }
    out.push('\n');

    out
}

/// Format an address type for display
fn format_addr_type(addr_type: Option<bluer::AddressType>) -> String {
    match addr_type {
        Some(bluer::AddressType::LePublic) => "public".to_string(),
        Some(bluer::AddressType::LeRandom) => "random".to_string(),
        Some(bluer::AddressType::BrEdr) => "BR/EDR".to_string(),
        None => "unknown".to_string(),
    }
}

/// Colorize RSSI value — green for strong, yellow for medium, red for weak
fn colorize_rssi(s: &str, rssi: Option<i16>) -> String {
    match rssi {
        Some(r) if r >= -50 => ctp_bold(s, GREEN).to_string(),
        Some(r) if r >= -70 => ctp(s, YELLOW).to_string(),
        Some(r) if r >= -85 => ctp(s, PEACH).to_string(),
        Some(_) => ctp(s, RED).to_string(),
        None => ctp(s, OVERLAY0).to_string(),
    }
}
