use clap::Parser;

/// Parse a hex handle value like "0x0004" or "0004" into u16
pub fn parse_handle(s: &str) -> Result<u16, String> {
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    u16::from_str_radix(s, 16).map_err(|e| format!("Invalid handle '{}': {}", s, e))
}

/// Parse a hex byte string like "0102ff" or "0x0102ff" into Vec<u8>
pub fn parse_hex_value(s: &str) -> Result<Vec<u8>, String> {
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    hex::decode(s).map_err(|e| format!("Invalid value '{}': {}", s, e))
}

#[derive(Parser, Debug)]
#[command(name = "gratttool", about = "BLE GATT tool (gatttool reimplementation)")]
pub struct Cli {
    /// Specify local adapter interface
    #[arg(short = 'i', long = "adapter", default_value = "hci0")]
    pub adapter: String,

    /// Specify remote Bluetooth address
    #[arg(short = 'b', long = "device")]
    pub device: Option<String>,

    /// Set LE address type. Default: public
    #[arg(short = 't', long = "addr-type", default_value = "public")]
    pub addr_type: String,

    /// Set BlueZ ExchangeMTU (23-517), or "show" to display current value [requires root to set]
    #[arg(short = 'm', long = "mtu")]
    pub mtu: Option<String>,

    /// Specify the PSM for GATT/ATT over BR/EDR
    #[arg(short = 'p', long = "psm", default_value_t = 0)]
    pub psm: u16,

    /// Set security level. Default: low
    #[arg(short = 'l', long = "sec-level", default_value = "low")]
    pub sec_level: String,

    /// Use interactive mode
    #[arg(short = 'I', long = "interactive")]
    pub interactive: bool,

    /// Primary Service Discovery
    #[arg(long = "primary")]
    pub primary: bool,

    /// Characteristics Discovery
    #[arg(long = "characteristics")]
    pub characteristics: bool,

    /// Characteristics Value/Descriptor Read
    #[arg(long = "char-read")]
    pub char_read: bool,

    /// Characteristics Value Write Without Response (Write Command)
    #[arg(long = "char-write")]
    pub char_write: bool,

    /// Characteristics Value Write (Write Request)
    #[arg(long = "char-write-req")]
    pub char_write_req: bool,

    /// Characteristics Descriptor Discovery
    #[arg(long = "char-desc")]
    pub char_desc: bool,

    /// Listen for notifications and indications (use sudo to catch hidden notifications)
    #[arg(long = "listen")]
    pub listen: bool,

    /// Enumerate all services, characteristics, and values in a table view
    #[arg(long = "enumerate", help_heading = "gratttool Enhanced Features")]
    pub enumerate: bool,

    /// Starting handle (optional)
    #[arg(short = 's', long = "start", value_parser = parse_handle, default_value = "0x0001")]
    pub start: u16,

    /// Ending handle (optional)
    #[arg(short = 'e', long = "end", value_parser = parse_handle, default_value = "0xffff")]
    pub end: u16,

    /// UUID16 or UUID128 (optional)
    #[arg(short = 'u', long = "uuid")]
    pub uuid: Option<String>,

    /// Read/Write characteristic by handle
    #[arg(short = 'a', long = "handle", value_parser = parse_handle)]
    pub handle: Option<u16>,

    /// Write characteristic value (hex bytes, e.g. 0102ff)
    #[arg(short = 'n', long = "value", num_args = 1.., trailing_var_arg = false)]
    pub value: Option<Vec<String>>,

    // --- gratttool-enhanced flags (not in original gatttool) ---

    /// Output read/notification values as ASCII (. for non-printable)
    #[arg(short = 'A', long = "ascii", help_heading = "gratttool Enhanced Features")]
    pub ascii: bool,

    /// Output read/notification values as hex AND ASCII side-by-side
    #[arg(short = 'X', long = "hex-ascii", conflicts_with = "ascii", help_heading = "gratttool Enhanced Features")]
    pub hex_ascii: bool,

    /// Write an ASCII string value (alternative to -n hex)
    #[arg(short = 'S', long = "string", conflicts_with = "value", help_heading = "gratttool Enhanced Features")]
    pub string: Option<String>,

    /// Change adapter BD_ADDR (MAC address), or "show" to display current address [requires root]
    #[arg(long = "bdaddr", help_heading = "gratttool Enhanced Features")]
    pub bdaddr: Option<String>,

    /// Don't reset adapter after BD_ADDR change
    #[arg(long = "bdaddr-no-reset", requires = "bdaddr", help_heading = "gratttool Enhanced Features")]
    pub bdaddr_no_reset: bool,

    /// CSR only: use transient mode (address lost on power cycle)
    #[arg(long = "bdaddr-transient", requires = "bdaddr", help_heading = "gratttool Enhanced Features")]
    pub bdaddr_transient: bool,
}

impl Cli {
    pub fn has_operation(&self) -> bool {
        self.primary
            || self.characteristics
            || self.char_read
            || self.char_write
            || self.char_write_req
            || self.char_desc
            || self.enumerate
    }

    /// Derive the output mode from -A / -X flags
    pub fn output_mode(&self) -> crate::output::OutputMode {
        if self.ascii {
            crate::output::OutputMode::Ascii
        } else if self.hex_ascii {
            crate::output::OutputMode::HexAscii
        } else {
            crate::output::OutputMode::Hex
        }
    }

    /// Return the write value from either -n (hex) or -S (string) flags.
    /// For -n, multiple arguments and whitespace are concatenated to handle
    /// shell-expanded `xxd -ps` output that wraps at 60 hex characters.
    pub fn effective_value(&self) -> Option<Result<Vec<u8>, String>> {
        if let Some(ref s) = self.string {
            Some(Ok(s.as_bytes().to_vec()))
        } else {
            self.value.as_ref().map(|parts| {
                let combined: String = parts.concat();
                parse_hex_value(&combined)
            })
        }
    }
}
