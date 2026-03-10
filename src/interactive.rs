use std::io::Write;
use std::pin::Pin;

use colored::Colorize;
use futures::StreamExt;
use rustyline_async::{Readline, ReadlineEvent};

use crate::connection::{self, Connection};
use crate::error::GrattError;
use crate::gatt;

/// Connection state machine
#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Disconnected,
    Connecting,
    Connected,
}

/// Command table entry
struct Command {
    name: &'static str,
    params: &'static str,
    desc: &'static str,
}

const COMMANDS: &[Command] = &[
    Command {
        name: "help",
        params: "",
        desc: "Show this help",
    },
    Command {
        name: "exit",
        params: "",
        desc: "Exit interactive mode",
    },
    Command {
        name: "quit",
        params: "",
        desc: "Exit interactive mode",
    },
    Command {
        name: "connect",
        params: "[address [address type]]",
        desc: "Connect to a remote device",
    },
    Command {
        name: "disconnect",
        params: "",
        desc: "Disconnect from a remote device",
    },
    Command {
        name: "primary",
        params: "[UUID]",
        desc: "Primary Service Discovery",
    },
    Command {
        name: "included",
        params: "[start hnd [end hnd]]",
        desc: "Find Included Services",
    },
    Command {
        name: "characteristics",
        params: "[start hnd [end hnd [UUID]]]",
        desc: "Characteristics Discovery",
    },
    Command {
        name: "char-desc",
        params: "[start hnd] [end hnd]",
        desc: "Characteristics Descriptor Discovery",
    },
    Command {
        name: "char-read-hnd",
        params: "<handle>",
        desc: "Characteristics Value/Descriptor Read by handle",
    },
    Command {
        name: "char-read-uuid",
        params: "<UUID> [start hnd] [end hnd]",
        desc: "Characteristics Value/Descriptor Read by UUID",
    },
    Command {
        name: "char-write-req",
        params: "<handle> <new value>",
        desc: "Characteristic Value Write (Write Request)",
    },
    Command {
        name: "char-write-cmd",
        params: "<handle> <new value>",
        desc: "Characteristic Value Write (No response)",
    },
    Command {
        name: "sec-level",
        params: "[low | medium | high]",
        desc: "Set security level. Default: low",
    },
    Command {
        name: "mtu",
        params: "<value>",
        desc: "Exchange MTU for GATT/ATT",
    },
];

/// Parse a hex handle from interactive input
fn parse_handle(s: &str) -> Result<u16, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u16::from_str_radix(s, 16).map_err(|_| format!("Invalid handle: {}", s))
}

/// Parse hex byte string from interactive input
fn parse_hex_value(s: &str) -> Result<Vec<u8>, String> {
    hex::decode(s).map_err(|_| "Invalid value".to_string())
}

/// Build the prompt string matching gatttool's format
fn get_prompt(state: State, device: Option<&str>, psm: u16) -> String {
    let addr_part = device.unwrap_or("                 ");
    let addr_display = format!("{:>17}", addr_part);

    let transport = if psm != 0 { "[BR]" } else { "[LE]" };

    if state == State::Connected {
        format!(
            "\x1b[34m[{}]\x1b[0m{}> ",
            addr_display, transport
        )
    } else {
        format!("[{}]{}> ", addr_display, transport)
    }
}

/// Interactive mode entry point
pub async fn run(
    adapter_name: &str,
    device_addr: Option<&str>,
    addr_type: &str,
    psm: u16,
    sec_level: &str,
) -> Result<(), GrattError> {
    let mut state = State::Disconnected;
    let mut conn: Option<Connection> = None;
    let mut opt_dst: Option<String> = device_addr.map(|s| s.to_string());
    let mut opt_dst_type: String = addr_type.to_string();
    let mut opt_sec_level: String = sec_level.to_string();
    let adapter = adapter_name.to_string();
    let mut opt_mtu: u16 = 0;

    let prompt = get_prompt(state, opt_dst.as_deref(), psm);
    let (mut rl, mut stdout) = Readline::new(prompt)
        .map_err(|e| GrattError::Io(std::io::Error::other(e.to_string())))?;

    // Notification streams: box-pinned for Unpin
    let mut notification_streams: Vec<
        Pin<Box<dyn futures::Stream<Item = (u16, bool, Vec<u8>)> + Send>>,
    > = Vec::new();

    loop {
        // Merge notification streams into a select_all, or use a pending future
        let mut combined = if !notification_streams.is_empty() {
            Some(futures::stream::select_all(
                notification_streams.drain(..).collect::<Vec<_>>(),
            ))
        } else {
            None
        };

        tokio::select! {
            event = rl.readline() => {
                // Re-collect streams back
                match event {
                    Ok(ReadlineEvent::Line(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        rl.add_history_entry(line.clone());

                        let parts: Vec<&str> = line.split_whitespace().collect();
                        let cmd = parts[0].to_lowercase();
                        let args = &parts[1..];

                        match cmd.as_str() {
                            "help" => {
                                for c in COMMANDS {
                                    writeln!(stdout, "{:<15} {:<30} {}", c.name, c.params, c.desc).ok();
                                }
                            }
                            "exit" | "quit" => {
                                if let Some(ref c) = conn {
                                    connection::disconnect(&c.device).await.ok();
                                }
                                break;
                            }
                            "connect" => {
                                if state != State::Disconnected {
                                    continue;
                                }

                                if !args.is_empty() {
                                    opt_dst = Some(args[0].to_string());
                                    if args.len() >= 2 {
                                        opt_dst_type = args[1].to_string();
                                    } else {
                                        opt_dst_type = "public".to_string();
                                    }
                                }

                                let dst = match &opt_dst {
                                    Some(d) => d.clone(),
                                    None => {
                                        writeln!(stdout, "{}", "Error: Remote Bluetooth address required".red()).ok();
                                        continue;
                                    }
                                };

                                writeln!(stdout, "Attempting to connect to {}", dst).ok();
                                state = State::Connecting;
                                rl.update_prompt(&get_prompt(state, opt_dst.as_deref(), psm)).ok();

                                let at = connection::parse_addr_type(&opt_dst_type);
                                match connection::parse_address(&dst) {
                                    Ok(addr) => {
                                        match connection::connect(&adapter, addr, at, &opt_sec_level, psm).await {
                                            Ok(c) => {
                                                // Subscribe to notifications
                                                notification_streams.clear();
                                                for obj in c.handle_table.notifiable_characteristics() {
                                                    if let crate::handle_table::GattObject::Characteristic {
                                                        characteristic,
                                                        value_handle,
                                                        properties,
                                                        ..
                                                    } = obj
                                                    {
                                                        let handle = *value_handle;
                                                        let is_ind = (properties.0 & 0x20) != 0;
                                                        if let Ok(stream) = characteristic.notify().await {
                                                            notification_streams.push(Box::pin(
                                                                stream.map(move |data| (handle, is_ind, data)),
                                                            ));
                                                        }
                                                    }
                                                }
                                                conn = Some(c);
                                                state = State::Connected;
                                                writeln!(stdout, "Connection successful").ok();
                                            }
                                            Err(e) => {
                                                state = State::Disconnected;
                                                writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        state = State::Disconnected;
                                        writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                    }
                                }
                                rl.update_prompt(&get_prompt(state, opt_dst.as_deref(), psm)).ok();
                            }
                            "disconnect" => {
                                if let Some(ref c) = conn {
                                    connection::disconnect(&c.device).await.ok();
                                }
                                conn = None;
                                notification_streams.clear();
                                state = State::Disconnected;
                                opt_mtu = 0;
                                rl.update_prompt(&get_prompt(state, opt_dst.as_deref(), psm)).ok();
                            }
                            "primary" => {
                                if state != State::Connected {
                                    writeln!(stdout, "{}", "Command Failed: Disconnected".red()).ok();
                                    continue;
                                }
                                let c = conn.as_ref().unwrap();
                                let uuid_filter = args.first().copied();

                                match gatt::discover_primary_interactive(c, uuid_filter).await {
                                    Ok(lines) => {
                                        for line in lines {
                                            writeln!(stdout, "{}", line).ok();
                                        }
                                    }
                                    Err(e) => {
                                        writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                    }
                                }
                            }
                            "included" => {
                                if state != State::Connected {
                                    writeln!(stdout, "{}", "Command Failed: Disconnected".red()).ok();
                                    continue;
                                }
                                let c = conn.as_ref().unwrap();

                                let start = if !args.is_empty() {
                                    match parse_handle(args[0]) {
                                        Ok(h) => h,
                                        Err(_) => {
                                            writeln!(stdout, "{}{}", "Error: Invalid start handle: ".red(), args[0]).ok();
                                            continue;
                                        }
                                    }
                                } else {
                                    0x0001
                                };

                                let end = if args.len() > 1 {
                                    match parse_handle(args[1]) {
                                        Ok(h) => h,
                                        Err(_) => {
                                            writeln!(stdout, "{}{}", "Error: Invalid end handle: ".red(), args[1]).ok();
                                            continue;
                                        }
                                    }
                                } else if !args.is_empty() {
                                    start
                                } else {
                                    0xffff
                                };

                                match gatt::find_included_interactive(c, start, end).await {
                                    Ok(lines) => {
                                        for line in lines {
                                            writeln!(stdout, "{}", line).ok();
                                        }
                                    }
                                    Err(e) => {
                                        writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                    }
                                }
                            }
                            "characteristics" => {
                                if state != State::Connected {
                                    writeln!(stdout, "{}", "Command Failed: Disconnected".red()).ok();
                                    continue;
                                }
                                let c = conn.as_ref().unwrap();

                                let start = if !args.is_empty() {
                                    match parse_handle(args[0]) {
                                        Ok(h) => h,
                                        Err(_) => {
                                            writeln!(stdout, "{}{}", "Error: Invalid start handle: ".red(), args[0]).ok();
                                            continue;
                                        }
                                    }
                                } else {
                                    0x0001
                                };

                                let end = if args.len() > 1 {
                                    match parse_handle(args[1]) {
                                        Ok(h) => h,
                                        Err(_) => {
                                            writeln!(stdout, "{}{}", "Error: Invalid end handle: ".red(), args[1]).ok();
                                            continue;
                                        }
                                    }
                                } else {
                                    0xffff
                                };

                                let uuid_filter = if args.len() > 2 {
                                    Some(args[2])
                                } else {
                                    None
                                };

                                match gatt::discover_characteristics_interactive(c, start, end, uuid_filter).await {
                                    Ok(lines) => {
                                        for line in lines {
                                            writeln!(stdout, "{}", line).ok();
                                        }
                                    }
                                    Err(e) => {
                                        writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                    }
                                }
                            }
                            "char-desc" => {
                                if state != State::Connected {
                                    writeln!(stdout, "{}", "Command Failed: Disconnected".red()).ok();
                                    continue;
                                }
                                let c = conn.as_ref().unwrap();

                                let start = if !args.is_empty() {
                                    match parse_handle(args[0]) {
                                        Ok(h) => h,
                                        Err(_) => {
                                            writeln!(stdout, "{}{}", "Error: Invalid start handle: ".red(), args[0]).ok();
                                            continue;
                                        }
                                    }
                                } else {
                                    0x0001
                                };

                                let end = if args.len() > 1 {
                                    match parse_handle(args[1]) {
                                        Ok(h) => h,
                                        Err(_) => {
                                            writeln!(stdout, "{}{}", "Error: Invalid end handle: ".red(), args[1]).ok();
                                            continue;
                                        }
                                    }
                                } else {
                                    0xffff
                                };

                                match gatt::discover_descriptors_interactive(c, start, end).await {
                                    Ok(lines) => {
                                        for line in lines {
                                            writeln!(stdout, "{}", line).ok();
                                        }
                                    }
                                    Err(e) => {
                                        writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                    }
                                }
                            }
                            "char-read-hnd" => {
                                if state != State::Connected {
                                    writeln!(stdout, "{}", "Command Failed: Disconnected".red()).ok();
                                    continue;
                                }
                                let c = conn.as_ref().unwrap();

                                if args.is_empty() {
                                    writeln!(stdout, "{}", "Error: Missing argument: handle".red()).ok();
                                    continue;
                                }

                                let handle = match parse_handle(args[0]) {
                                    Ok(h) => h,
                                    Err(_) => {
                                        writeln!(stdout, "{}{}", "Error: Invalid handle: ".red(), args[0]).ok();
                                        continue;
                                    }
                                };

                                match gatt::read_by_handle_interactive(c, handle).await {
                                    Ok(line) => {
                                        writeln!(stdout, "{}", line).ok();
                                    }
                                    Err(e) => {
                                        writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                    }
                                }
                            }
                            "char-read-uuid" => {
                                if state != State::Connected {
                                    writeln!(stdout, "{}", "Command Failed: Disconnected".red()).ok();
                                    continue;
                                }
                                let c = conn.as_ref().unwrap();

                                if args.is_empty() {
                                    writeln!(stdout, "{}", "Error: Missing argument: UUID".red()).ok();
                                    continue;
                                }

                                let uuid = args[0];
                                let start = if args.len() > 1 {
                                    match parse_handle(args[1]) {
                                        Ok(h) => h,
                                        Err(_) => {
                                            writeln!(stdout, "{}{}", "Error: Invalid start handle: ".red(), args[1]).ok();
                                            continue;
                                        }
                                    }
                                } else {
                                    0x0001
                                };

                                let end = if args.len() > 2 {
                                    match parse_handle(args[2]) {
                                        Ok(h) => h,
                                        Err(_) => {
                                            writeln!(stdout, "{}{}", "Error: Invalid end handle: ".red(), args[2]).ok();
                                            continue;
                                        }
                                    }
                                } else {
                                    0xffff
                                };

                                match gatt::read_by_uuid_interactive(c, uuid, start, end).await {
                                    Ok(lines) => {
                                        for line in lines {
                                            writeln!(stdout, "{}", line).ok();
                                        }
                                    }
                                    Err(e) => {
                                        writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                    }
                                }
                            }
                            "char-write-req" | "char-write-cmd" => {
                                if state != State::Connected {
                                    writeln!(stdout, "{}", "Command Failed: Disconnected".red()).ok();
                                    continue;
                                }
                                let c = conn.as_ref().unwrap();

                                if args.len() < 2 {
                                    writeln!(stdout, "Usage: {} <handle> <new value>", cmd).ok();
                                    continue;
                                }

                                let handle = match parse_handle(args[0]) {
                                    Ok(h) if h > 0 => h,
                                    _ => {
                                        writeln!(stdout, "{}", "Error: A valid handle is required".red()).ok();
                                        continue;
                                    }
                                };

                                let value = match parse_hex_value(args[1]) {
                                    Ok(v) if !v.is_empty() => v,
                                    _ => {
                                        writeln!(stdout, "{}", "Error: Invalid value".red()).ok();
                                        continue;
                                    }
                                };

                                if cmd == "char-write-req" {
                                    match gatt::write_request_interactive(c, handle, &value).await {
                                        Ok(msg) => {
                                            writeln!(stdout, "{}", msg).ok();
                                        }
                                        Err(e) => {
                                            writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                        }
                                    }
                                } else {
                                    match gatt::write_command_interactive(c, handle, &value).await {
                                        Ok(()) => {}
                                        Err(e) => {
                                            writeln!(stdout, "{}{}", "Error: ".red(), e).ok();
                                        }
                                    }
                                }
                            }
                            "sec-level" => {
                                if args.is_empty() {
                                    writeln!(stdout, "sec-level: {}", opt_sec_level).ok();
                                    continue;
                                }

                                let level = args[0].to_lowercase();
                                match level.as_str() {
                                    "low" | "medium" | "high" => {
                                        opt_sec_level = level;
                                        if state == State::Connected && psm != 0 {
                                            writeln!(stdout, "Change will take effect on reconnection").ok();
                                        }
                                    }
                                    _ => {
                                        writeln!(stdout, "Allowed values: low | medium | high").ok();
                                    }
                                }
                            }
                            "mtu" => {
                                if state != State::Connected {
                                    writeln!(stdout, "{}", "Command Failed: Disconnected".red()).ok();
                                    continue;
                                }

                                if psm != 0 {
                                    writeln!(stdout, "{}", "Command Failed: Operation is only available for LE transport.".red()).ok();
                                    continue;
                                }

                                if args.is_empty() {
                                    writeln!(stdout, "Usage: mtu <value>").ok();
                                    continue;
                                }

                                if opt_mtu != 0 {
                                    writeln!(stdout, "{}", "Command Failed: MTU exchange can only occur once per connection.".red()).ok();
                                    continue;
                                }

                                let mtu_val: u16 = match args[0].parse() {
                                    Ok(v) if v >= 23 => v,
                                    _ => {
                                        writeln!(stdout, "{}", "Error: Invalid value. Minimum MTU size is 23".red()).ok();
                                        continue;
                                    }
                                };

                                opt_mtu = mtu_val;
                                writeln!(stdout, "MTU was exchanged successfully: {}", mtu_val).ok();
                            }
                            _ => {
                                writeln!(stdout, "{}{}: command not found", "Error: ".red(), cmd).ok();
                            }
                        }
                    }
                    Ok(ReadlineEvent::Eof) => {
                        writeln!(stdout).ok();
                        if let Some(ref c) = conn {
                            connection::disconnect(&c.device).await.ok();
                        }
                        break;
                    }
                    Ok(ReadlineEvent::Interrupted) => {
                        continue;
                    }
                    Err(e) => {
                        writeln!(stdout, "Readline error: {}", e).ok();
                        break;
                    }
                }
            }
            Some(notif) = async {
                match combined.as_mut() {
                    Some(c) => c.next().await,
                    None => std::future::pending().await,
                }
            } => {
                let (handle, is_indication, data) = notif;
                if is_indication {
                    writeln!(
                        stdout,
                        "{}",
                        crate::output::fmt_indication(handle, &data, crate::output::OutputMode::Hex)
                    ).ok();
                } else {
                    writeln!(
                        stdout,
                        "{}",
                        crate::output::fmt_notification(handle, &data, crate::output::OutputMode::Hex)
                    ).ok();
                }
            }
        }
    }

    Ok(())
}
