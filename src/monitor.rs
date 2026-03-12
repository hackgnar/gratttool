//! HCI monitor socket for catching notification/indication PDUs that the
//! BlueZ D-Bus API silently drops (e.g., characteristics without the NOTIFY
//! property bit).  Uses the same kernel interface as `btmon`.

use std::io;
use tokio::sync::mpsc;

// Bluetooth socket constants
const AF_BLUETOOTH: i32 = 31;
const BTPROTO_HCI: i32 = 1;
const HCI_CHANNEL_MONITOR: u16 = 2;
const HCI_DEV_NONE: u16 = 0xFFFF;

// Monitor header opcode for incoming ACL data (controller → host)
const HCI_MON_ACL_RX_PKT: u16 = 5;

// ATT fixed-channel CID over LE
const ATT_CID: u16 = 0x0004;

// ATT opcodes we care about
const ATT_OP_HANDLE_NOTIFY: u8 = 0x1B;
const ATT_OP_HANDLE_IND: u8 = 0x1D;

/// A notification or indication event parsed from a raw HCI monitor packet.
pub struct NotificationEvent {
    pub handle: u16,
    pub is_indication: bool,
    pub value: Vec<u8>,
}

#[repr(C)]
struct SockaddrHci {
    hci_family: u16,
    hci_dev: u16,
    hci_channel: u16,
}

/// Parse an adapter name like "hci0" or "hci1" to its numeric index.
fn adapter_index(name: &str) -> u16 {
    name.strip_prefix("hci")
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0)
}

/// Open an HCI monitor socket and spawn a background thread that watches for
/// ATT notification/indication PDUs on the given adapter.  Returns an
/// `mpsc::UnboundedReceiver` that yields parsed events.
///
/// Requires `CAP_NET_RAW` or root.
pub fn open(adapter_name: &str) -> io::Result<mpsc::UnboundedReceiver<NotificationEvent>> {
    let target_index = adapter_index(adapter_name);

    let fd = unsafe { libc::socket(AF_BLUETOOTH, libc::SOCK_RAW, BTPROTO_HCI) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let addr = SockaddrHci {
        hci_family: AF_BLUETOOTH as u16,
        hci_dev: HCI_DEV_NONE,
        hci_channel: HCI_CHANNEL_MONITOR,
    };

    let ret = unsafe {
        libc::bind(
            fd,
            &addr as *const SockaddrHci as *const libc::sockaddr,
            std::mem::size_of::<SockaddrHci>() as libc::socklen_t,
        )
    };
    if ret < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd); }
        return Err(err);
    }

    let (tx, rx) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                break;
            }
            let n = n as usize;

            // Monitor header: opcode(2) + index(2) + len(2) = 6 bytes
            if n < 6 {
                continue;
            }
            let opcode = u16::from_le_bytes([buf[0], buf[1]]);
            let index = u16::from_le_bytes([buf[2], buf[3]]);

            // Only ACL RX packets from our adapter
            if opcode != HCI_MON_ACL_RX_PKT || index != target_index {
                continue;
            }

            let data = &buf[6..n];

            // HCI ACL header: conn_handle(2) + total_len(2)
            // L2CAP header:   len(2) + cid(2)
            // ATT PDU:        opcode(1) + handle(2) + value(...)
            // Minimum: 4 + 4 + 1 + 2 = 11 bytes
            if data.len() < 11 {
                continue;
            }

            let l2cap_cid = u16::from_le_bytes([data[6], data[7]]);
            if l2cap_cid != ATT_CID {
                continue;
            }

            let att_opcode = data[8];
            let is_notification = att_opcode == ATT_OP_HANDLE_NOTIFY;
            let is_indication = att_opcode == ATT_OP_HANDLE_IND;
            if !is_notification && !is_indication {
                continue;
            }

            let att_handle = u16::from_le_bytes([data[9], data[10]]);
            let value = data[11..].to_vec();

            if tx.send(NotificationEvent {
                handle: att_handle,
                is_indication,
                value,
            }).is_err() {
                break; // Receiver dropped
            }
        }
        unsafe { libc::close(fd); }
    });

    Ok(rx)
}
