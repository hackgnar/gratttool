//! BD_ADDR (Bluetooth MAC address) changing via vendor-specific HCI commands.
//!
//! This is a Rust reimplementation of the BlueZ `bdaddr` tool. It uses:
//! - The BlueZ Management API (HCI_CHANNEL_CONTROL) to read controller info
//!   (manufacturer ID, current address) — works on modern kernels that restrict
//!   raw HCI sockets to vendor-only commands.
//! - Raw HCI sockets (HCI_CHANNEL_RAW) only for vendor-specific write commands
//!   (OGF 0x3F), which are still permitted on modern kernels.
//! - ioctls for device reset.
//!
//! Requires CAP_NET_ADMIN or root.

use std::fmt;
use std::io;
use std::mem;

// ── Linux Bluetooth socket constants ────────────────────────────────────────

const AF_BLUETOOTH: i32 = 31;
const BTPROTO_HCI: i32 = 1;

const HCI_CHANNEL_RAW: u16 = 0;
const HCI_CHANNEL_CONTROL: u16 = 3;
const HCI_DEV_NONE: u16 = 0xFFFF;

const OGF_VENDOR_CMD: u16 = 0x3F;

const HCI_COMMAND_PKT: u8 = 0x01;

// ── Management API opcodes ──────────────────────────────────────────────────

const MGMT_OP_READ_INFO: u16 = 0x0004;
const MGMT_EV_CMD_COMPLETE: u16 = 0x0001;

// ── HCI ioctl number helpers ────────────────────────────────────────────────

fn hci_ioctl_iow(nr: libc::c_ulong) -> libc::c_ulong {
    // _IOW('H', nr, int): (1 << 30) | (sizeof(int) << 16) | ('H' << 8) | nr
    (1 << 30) | (4 << 16) | (0x48 << 8) | nr
}

fn opcode(ogf: u16, ocf: u16) -> u16 {
    (ogf << 10) | ocf
}

// ── BD address type ─────────────────────────────────────────────────────────

/// Raw BD address (6 bytes, little-endian as stored by BlueZ)
#[derive(Clone, Copy)]
pub struct BdAddr(pub [u8; 6]);

impl BdAddr {
    pub fn from_str(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return Err(format!("Invalid BD address '{}': expected XX:XX:XX:XX:XX:XX", s));
        }
        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[5 - i] = u8::from_str_radix(part, 16)
                .map_err(|_| format!("Invalid BD address byte '{}'", part))?;
        }
        Ok(BdAddr(bytes))
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 6]
    }
}

impl fmt::Display for BdAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[5], self.0[4], self.0[3], self.0[2], self.0[1], self.0[0]
        )
    }
}

// ── sockaddr_hci ────────────────────────────────────────────────────────────

#[repr(C)]
struct SockAddrHci {
    hci_family: libc::sa_family_t,
    hci_dev: u16,
    hci_channel: u16,
}

// ── Controller info from Management API ─────────────────────────────────────

struct ControllerInfo {
    address: BdAddr,
    manufacturer: u16,
}

/// Read controller info (address + manufacturer) via the BlueZ Management API.
/// This uses HCI_CHANNEL_CONTROL which is not restricted like raw HCI sockets.
fn mgmt_read_info(dev_id: u16) -> io::Result<ControllerInfo> {
    let fd = unsafe {
        libc::socket(AF_BLUETOOTH, libc::SOCK_RAW | libc::SOCK_CLOEXEC, BTPROTO_HCI)
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    // Bind to the control channel (not a specific device)
    let addr = SockAddrHci {
        hci_family: AF_BLUETOOTH as u16,
        hci_dev: HCI_DEV_NONE,
        hci_channel: HCI_CHANNEL_CONTROL,
    };

    let ret = unsafe {
        libc::bind(
            fd,
            &addr as *const SockAddrHci as *const libc::sockaddr,
            mem::size_of::<SockAddrHci>() as u32,
        )
    };
    if ret < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd); }
        return Err(err);
    }

    // Build management command: Read Controller Information
    // struct mgmt_hdr { le16 opcode; le16 index; le16 len; }
    let mut cmd = [0u8; 6];
    cmd[0..2].copy_from_slice(&MGMT_OP_READ_INFO.to_le_bytes());
    cmd[2..4].copy_from_slice(&dev_id.to_le_bytes());
    cmd[4..6].copy_from_slice(&0u16.to_le_bytes()); // param len = 0

    let written = unsafe {
        libc::write(fd, cmd.as_ptr() as *const libc::c_void, cmd.len())
    };
    if written < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd); }
        return Err(err);
    }

    // Set read timeout
    let tv = libc::timeval { tv_sec: 5, tv_usec: 0 };
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &tv as *const libc::timeval as *const libc::c_void,
            mem::size_of::<libc::timeval>() as u32,
        );
    }

    // Read response
    // Response format: mgmt_hdr(6) + status(1) + opcode(2) + ... controller info
    let mut buf = [0u8; 512];
    let n = unsafe {
        libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
    };
    unsafe { libc::close(fd); }

    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    let n = n as usize;

    // Parse management event response
    // mgmt_hdr: event_opcode(2) + index(2) + param_len(2) = 6 bytes
    // cmd_complete params: cmd_opcode(2) + status(1) + return_params...
    if n < 9 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Short management response",
        ));
    }

    let evt_opcode = u16::from_le_bytes([buf[0], buf[1]]);
    if evt_opcode != MGMT_EV_CMD_COMPLETE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unexpected management event 0x{:04x}", evt_opcode),
        ));
    }

    // cmd_complete params start at buf[6]
    let cmd_opcode = u16::from_le_bytes([buf[6], buf[7]]);
    let status = buf[8];

    if cmd_opcode != MGMT_OP_READ_INFO {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unexpected management response opcode 0x{:04x}", cmd_opcode),
        ));
    }

    if status != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Management command failed with status 0x{:02x}", status),
        ));
    }

    // Return parameters start at buf[9]:
    //   Address (6) + BT_Version (1) + Manufacturer (2) + ...
    if n < 9 + 9 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Short controller info response",
        ));
    }

    let params = &buf[9..];
    let mut address = [0u8; 6];
    address.copy_from_slice(&params[0..6]);
    // params[6] = bluetooth_version
    let manufacturer = u16::from_le_bytes([params[7], params[8]]);

    Ok(ControllerInfo {
        address: BdAddr(address),
        manufacturer,
    })
}

// ── Raw HCI socket (for vendor commands only) ───────────────────────────────

struct HciSocket {
    fd: i32,
    dev_id: u16,
}

impl HciSocket {
    /// Open a raw HCI socket bound to the given device.
    /// On modern kernels, only vendor-specific commands (OGF 0x3F) can be sent.
    fn open(dev_id: u16) -> io::Result<Self> {
        let fd = unsafe {
            libc::socket(AF_BLUETOOTH, libc::SOCK_RAW | libc::SOCK_CLOEXEC, BTPROTO_HCI)
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        let addr = SockAddrHci {
            hci_family: AF_BLUETOOTH as u16,
            hci_dev: dev_id,
            hci_channel: HCI_CHANNEL_RAW,
        };

        let ret = unsafe {
            libc::bind(
                fd,
                &addr as *const SockAddrHci as *const libc::sockaddr,
                mem::size_of::<SockAddrHci>() as u32,
            )
        };
        if ret < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(fd); }
            return Err(err);
        }

        Ok(HciSocket { fd, dev_id })
    }

    /// Send a vendor-specific HCI command (fire-and-forget).
    /// On modern kernels (5.x+), raw HCI sockets cannot reliably read back
    /// command-complete events, so we just send and trust the controller.
    fn vendor_send(&self, ocf: u16, params: &[u8]) -> io::Result<()> {
        let op = opcode(OGF_VENDOR_CMD, ocf);

        let mut cmd = Vec::with_capacity(4 + params.len());
        cmd.push(HCI_COMMAND_PKT);
        cmd.push((op & 0xFF) as u8);
        cmd.push((op >> 8) as u8);
        cmd.push(params.len() as u8);
        cmd.extend_from_slice(params);

        let written = unsafe {
            libc::write(self.fd, cmd.as_ptr() as *const libc::c_void, cmd.len())
        };
        if written < 0 {
            return Err(io::Error::last_os_error());
        }

        // Brief delay for the controller to process the command
        std::thread::sleep(std::time::Duration::from_millis(100));
        Ok(())
    }

    /// Send a vendor command for writing a 6-byte BD address
    fn vendor_write_bdaddr(&self, ocf: u16, bdaddr: &BdAddr) -> io::Result<()> {
        self.vendor_send(ocf, &bdaddr.0)
    }

    /// Reset the HCI device via ioctl
    fn reset_device(&self) -> io::Result<()> {
        let ret = unsafe {
            libc::ioctl(self.fd, hci_ioctl_iow(203), self.dev_id as libc::c_int)
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for HciSocket {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd); }
    }
}

// ── Vendor-specific write implementations ───────────────────────────────────

/// Ericsson (compid 0, 57) — OCF 0x000D
fn ericsson_write(sock: &HciSocket, bdaddr: &BdAddr) -> io::Result<()> {
    sock.vendor_write_bdaddr(0x000D, bdaddr)
}

/// CSR (compid 10) — OCF 0x0000, BCCMD with custom byte layout
fn csr_write(sock: &HciSocket, bdaddr: &BdAddr, transient: bool) -> io::Result<()> {
    let mut cmd: [u8; 24] = [
        0x02, 0x00, 0x0c, 0x00, 0x11, 0x47, 0x03, 0x70,
        0x00, 0x00, 0x01, 0x00, 0x04, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    if transient {
        cmd[14] = 0x08;
    }

    // CSR uses a peculiar byte ordering
    cmd[16] = bdaddr.0[2];
    cmd[17] = 0x00;
    cmd[18] = bdaddr.0[0];
    cmd[19] = bdaddr.0[1];
    cmd[20] = bdaddr.0[3];
    cmd[21] = 0x00;
    cmd[22] = bdaddr.0[4];
    cmd[23] = bdaddr.0[5];

    // CSR commands are wrapped with a 0xc2 prefix
    let mut cp = [0u8; 25];
    cp[0] = 0xc2;
    cp[1..25].copy_from_slice(&cmd);

    // Fire-and-forget the vendor command
    sock.vendor_send(0x0000, &cp)
}

/// CSR reset — warm reset via BCCMD (vendor command, so allowed on raw socket)
fn csr_reset(sock: &HciSocket, transient: bool) -> io::Result<()> {
    let mut cmd: [u8; 18] = [
        0x02, 0x00, 0x09, 0x00,
        0x00, 0x00, 0x01, 0x40, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    if transient {
        cmd[6] = 0x02;
    }

    let mut cp = [0u8; 19];
    cp[0] = 0xc2;
    cp[1..19].copy_from_slice(&cmd);

    let op = opcode(OGF_VENDOR_CMD, 0x0000);
    let mut pkt = Vec::with_capacity(4 + cp.len());
    pkt.push(HCI_COMMAND_PKT);
    pkt.push((op & 0xFF) as u8);
    pkt.push((op >> 8) as u8);
    pkt.push(cp.len() as u8);
    pkt.extend_from_slice(&cp);

    let written = unsafe {
        libc::write(sock.fd, pkt.as_ptr() as *const libc::c_void, pkt.len())
    };
    if written < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// TI (compid 13) — OCF 0x0006
fn ti_write(sock: &HciSocket, bdaddr: &BdAddr) -> io::Result<()> {
    sock.vendor_write_bdaddr(0x0006, bdaddr)
}

/// Broadcom (compid 15) — OCF 0x0001
fn bcm_write(sock: &HciSocket, bdaddr: &BdAddr) -> io::Result<()> {
    sock.vendor_write_bdaddr(0x0001, bdaddr)
}

/// Intel (compid 2) — OCF 0x0031
fn intel_write(sock: &HciSocket, bdaddr: &BdAddr) -> io::Result<()> {
    sock.vendor_write_bdaddr(0x0031, bdaddr)
}

/// Cypress/Infineon (compid 305) — OCF 0x0001
fn cypress_write(sock: &HciSocket, bdaddr: &BdAddr) -> io::Result<()> {
    sock.vendor_write_bdaddr(0x0001, bdaddr)
}

/// Zeevo (compid 18) — OCF 0x0001
fn zeevo_write(sock: &HciSocket, bdaddr: &BdAddr) -> io::Result<()> {
    sock.vendor_write_bdaddr(0x0001, bdaddr)
}

/// ST Microelectronics (compid 48) — uses Ericsson store-in-flash (OCF 0x0022, vendor OGF)
fn st_write(sock: &HciSocket, bdaddr: &BdAddr) -> io::Result<()> {
    let mut params = [0u8; 255];
    params[0] = 0xFE; // user_id
    params[1] = 6;    // flash_length
    params[2..8].copy_from_slice(&bdaddr.0);

    sock.vendor_send(0x0022, &params)
}

// ── Vendor dispatch table ───────────────────────────────────────────────────

struct VendorEntry {
    compid: u16,
    name: &'static str,
    write_fn: fn(&HciSocket, &BdAddr, bool) -> io::Result<()>,
    has_generic_reset: bool,
    has_custom_reset: bool,
}

fn wrap_ericsson(s: &HciSocket, b: &BdAddr, _t: bool) -> io::Result<()> { ericsson_write(s, b) }
fn wrap_csr(s: &HciSocket, b: &BdAddr, t: bool) -> io::Result<()> { csr_write(s, b, t) }
fn wrap_ti(s: &HciSocket, b: &BdAddr, _t: bool) -> io::Result<()> { ti_write(s, b) }
fn wrap_bcm(s: &HciSocket, b: &BdAddr, _t: bool) -> io::Result<()> { bcm_write(s, b) }
fn wrap_intel(s: &HciSocket, b: &BdAddr, _t: bool) -> io::Result<()> { intel_write(s, b) }
fn wrap_cypress(s: &HciSocket, b: &BdAddr, _t: bool) -> io::Result<()> { cypress_write(s, b) }
fn wrap_zeevo(s: &HciSocket, b: &BdAddr, _t: bool) -> io::Result<()> { zeevo_write(s, b) }
fn wrap_st(s: &HciSocket, b: &BdAddr, _t: bool) -> io::Result<()> { st_write(s, b) }

static VENDORS: &[VendorEntry] = &[
    VendorEntry { compid: 0,   name: "Ericsson",                write_fn: wrap_ericsson, has_generic_reset: false, has_custom_reset: false },
    VendorEntry { compid: 2,   name: "Intel",                   write_fn: wrap_intel,    has_generic_reset: false, has_custom_reset: true  },
    VendorEntry { compid: 10,  name: "Cambridge Silicon Radio", write_fn: wrap_csr,      has_generic_reset: false, has_custom_reset: true  },
    VendorEntry { compid: 13,  name: "Texas Instruments",       write_fn: wrap_ti,       has_generic_reset: false, has_custom_reset: false },
    VendorEntry { compid: 15,  name: "Broadcom",                write_fn: wrap_bcm,      has_generic_reset: true,  has_custom_reset: false },
    VendorEntry { compid: 18,  name: "Zeevo",                   write_fn: wrap_zeevo,    has_generic_reset: false, has_custom_reset: false },
    VendorEntry { compid: 48,  name: "ST Microelectronics",     write_fn: wrap_st,       has_generic_reset: true,  has_custom_reset: false },
    VendorEntry { compid: 57,  name: "Ericsson (57)",           write_fn: wrap_ericsson, has_generic_reset: true,  has_custom_reset: false },
    VendorEntry { compid: 305, name: "Cypress/Infineon",        write_fn: wrap_cypress,  has_generic_reset: false, has_custom_reset: true  },
];

fn manufacturer_name(compid: u16) -> &'static str {
    for v in VENDORS {
        if v.compid == compid {
            return v.name;
        }
    }
    "Unknown"
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Parse adapter name (e.g. "hci0", "hci1") to device ID
pub fn parse_dev_id(adapter: &str) -> Result<u16, String> {
    let s = adapter.strip_prefix("hci").unwrap_or(adapter);
    s.parse::<u16>().map_err(|_| format!("Invalid adapter '{}': expected hciN", adapter))
}

/// Show the current BD address and manufacturer info for an adapter.
pub fn show(adapter: &str) -> Result<(), String> {
    let dev_id = parse_dev_id(adapter)?;

    let info = mgmt_read_info(dev_id)
        .map_err(|e| format!("Can't read controller info for {}: {} (are you root?)", adapter, e))?;

    println!("Manufacturer:   {} ({})", manufacturer_name(info.manufacturer), info.manufacturer);
    println!("Device address: {}", info.address);

    Ok(())
}

/// Change the BD address of an adapter.
pub fn change(adapter: &str, new_addr_str: &str, reset: bool, transient: bool) -> Result<(), String> {
    let dev_id = parse_dev_id(adapter)?;
    let new_addr = BdAddr::from_str(new_addr_str)?;

    if new_addr.is_zero() {
        return Err("Cannot set address to 00:00:00:00:00:00".into());
    }

    // Read controller info via management API (works on modern kernels)
    let info = mgmt_read_info(dev_id)
        .map_err(|e| format!("Can't read controller info for {}: {} (are you root?)", adapter, e))?;

    println!("Manufacturer:   {} ({})", manufacturer_name(info.manufacturer), info.manufacturer);
    println!("Device address: {}", info.address);
    println!("New BD address: {}", new_addr);
    println!();

    // Find vendor
    let vendor = VENDORS.iter().find(|v| v.compid == info.manufacturer);
    let vendor = match vendor {
        Some(v) => v,
        None => {
            return Err(format!(
                "Unsupported manufacturer: {} ({}). \
                 You may be able to use `hcitool cmd` to send vendor-specific commands manually.",
                manufacturer_name(info.manufacturer),
                info.manufacturer
            ));
        }
    };

    // Open raw HCI socket for vendor-specific write command
    let sock = HciSocket::open(dev_id)
        .map_err(|e| format!("Can't open device {}: {} (are you root?)", adapter, e))?;

    // Write the new address (vendor-specific command, OGF 0x3F — allowed on raw sockets)
    (vendor.write_fn)(&sock, &new_addr, transient)
        .map_err(|e| format!("Can't write new address: {}", e))?;

    print!("Address changed - ");

    if reset {
        if vendor.has_generic_reset {
            // Use ioctl reset (modern kernels block HCI_Reset on raw sockets)
            match sock.reset_device() {
                Ok(()) => println!("Device reset successfully"),
                Err(_) => println!("Reset device manually"),
            }
        } else if vendor.has_custom_reset {
            if vendor.compid == 10 {
                // CSR warm reset (vendor command, allowed on raw socket)
                match csr_reset(&sock, transient) {
                    Ok(()) => println!("Device reset successfully"),
                    Err(_) => println!("Reset device manually"),
                }
            } else {
                // Intel, Cypress — require manual reset
                println!("Reset device manually");
            }
        } else {
            println!("Reset device now");
        }
    } else {
        println!("Reset device now");
    }

    Ok(())
}
