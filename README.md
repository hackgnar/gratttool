<table width="100%" bgcolor="black" border="0" cellspacing="0" cellpadding="20">
  <tr>
    <td align="center">
      <img src="assets/gratttool_logo.png" alt="gratttool logo" width="400">
    </td>
  </tr>
</table>

# WARNING - This is an ALPHA release. There are still many bugs and full testing has yet to be completed - WARNING

# gratttool

A full-featured Rust reimplementation of the deprecated BlueZ `gatttool` utility for interacting with Bluetooth Low Energy (BLE) devices via GATT (Generic Attribute Profile).

gratttool provides the same CLI argument syntax, input parsing, and output formatting as the original gatttool, while using the modern [bluer](https://crates.io/crates/bluer) crate (official BlueZ D-Bus Rust bindings) instead of raw HCI/ATT sockets.

## Why gratttool?

The original `gatttool` was removed from BlueZ in 2017 but remains one of the most referenced BLE tools in documentation, tutorials, and security research. gratttool fills that gap with a drop-in replacement that:

- Produces **character-for-character identical output** to the original gatttool
- Uses the **modern BlueZ D-Bus API** instead of deprecated raw sockets
- Is written in **safe Rust** with no C dependencies beyond libdbus
- Supports both **non-interactive** (one-shot) and **interactive** (shell) modes
- Adds a **`--enumerate` table view** (inspired by [bleah](https://github.com/evilsocket/bleah)) with Catppuccin-themed colors for at-a-glance device exploration

## Building

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- libdbus development headers
- BlueZ (the Linux Bluetooth stack)

```bash
# Debian/Ubuntu
sudo apt install libdbus-1-dev pkg-config bluetooth bluez

# Fedora
sudo dnf install dbus-devel pkgconf-pkg-config bluez

# Arch
sudo pacman -S dbus bluez
```

### Compile

```bash
cargo build --release
```

The binary is at `./target/release/gratttool`.

## Usage

### Connection Options

| Flag | Long | Description | Default |
|------|------|-------------|---------|
| `-i` | `--adapter` | Local adapter interface | `hci0` |
| `-b` | `--device` | Remote device MAC address | (required for non-interactive) |
| `-t` | `--addr-type` | Address type: `public` or `random` | `public` |
| `-m` | `--mtu` | MTU size (0 = auto) | `0` |
| `-p` | `--psm` | PSM for GATT/ATT over BR/EDR (0 = LE) | `0` |
| `-l` | `--sec-level` | Security level: `low`, `medium`, or `high` | `low` |

### Operations

| Flag | Description |
|------|-------------|
| `--primary` | Discover all primary services |
| `--characteristics` | Discover characteristics |
| `--char-read` | Read characteristic value by handle or UUID |
| `--char-write` | Write command (no response) |
| `--char-write-req` | Write request (with response) |
| `--char-desc` | Discover characteristic descriptors |
| `--listen` | Listen for notifications and indications |
| `-I`, `--interactive` | Enter interactive shell mode |

### Parameters

| Flag | Long | Description | Default |
|------|------|-------------|---------|
| `-s` | `--start` | Start handle for discovery range | `0x0001` |
| `-e` | `--end` | End handle for discovery range | `0xffff` |
| `-u` | `--uuid` | UUID filter (UUID16 like `1800` or full UUID128) | |
| `-a` | `--handle` | Attribute handle for read/write | |
| `-n` | `--value` | Hex byte string value for writes (e.g. `0102ff`) | |

### gratttool Enhanced Features

These flags are **not available in the original gatttool** — they are gratttool extensions designed to streamline common BLE CTF and reverse-engineering workflows.

| Flag | Long | Type | Description |
|------|------|------|-------------|
| | `--enumerate` | bool | Enumerate device info, services, characteristics, and values in a table |
| `-A` | `--ascii` | bool | Output read/notification values as ASCII (`.` for non-printable) |
| `-X` | `--hex-ascii` | bool | Output read/notification values as hex AND ASCII side-by-side |
| `-S` | `--string` | `<TEXT>` | Write an ASCII string value (alternative to `-n` hex) |

**Conflicts:** `-A` and `-X` are mutually exclusive. `-S` and `-n` are mutually exclusive.

#### `-A` / `--ascii` (read output as ASCII)

Converts hex output to readable ASCII, replacing non-printable bytes with `.`:

```bash
# Without -A (default gatttool-compatible output):
gratttool -b AA:BB:CC:DD:EE:FF --char-read -a 0x002a
# Characteristic value/descriptor: 66 6c 61 67 5f 76 61 6c 75 65

# With -A:
gratttool -b AA:BB:CC:DD:EE:FF --char-read -a 0x002a -A
# Characteristic value/descriptor: flag_value

# Also works with --listen:
gratttool -b AA:BB:CC:DD:EE:FF --listen -A
# Notification handle = 0x000e value: hello
```

#### `-X` / `--hex-ascii` (hex + ASCII side-by-side)

Shows both hex and ASCII together, like a hex dump:

```bash
gratttool -b AA:BB:CC:DD:EE:FF --char-read -a 0x002a -X
# Characteristic value/descriptor: 66 6c 61 67 5f 76 61 6c 75 65  flag_value

gratttool -b AA:BB:CC:DD:EE:FF --char-read -u 2a00 -X
# handle: 0x0003 	 value: 48 65 6c 6c 6f  Hello
```

#### `-S` / `--string` (write ASCII strings directly)

Eliminates the common `echo -n "..." | xxd -ps` pipe for writing ASCII values:

```bash
# Before (error-prone — forgetting -n adds a trailing 0a newline byte):
gratttool -b AA:BB:CC:DD:EE:FF --char-write-req -a 0x002c -n $(echo -n "flag_value" | xxd -ps)

# After:
gratttool -b AA:BB:CC:DD:EE:FF --char-write-req -a 0x002c -S "flag_value"
```

## Non-Interactive Mode

Non-interactive mode connects to a device, executes a single operation, prints results, and exits.

### Discover Primary Services

```bash
# All services
gratttool -b AA:BB:CC:DD:EE:FF --primary

# Output:
# attr handle = 0x0001, end grp handle = 0x0005 uuid: 00001800-0000-1000-8000-00805f9b34fb
# attr handle = 0x0006, end grp handle = 0x0009 uuid: 00001801-0000-1000-8000-00805f9b34fb

# Filter by UUID
gratttool -b AA:BB:CC:DD:EE:FF --primary -u 1800

# Output:
# Starting handle: 0001 Ending handle: 0005
```

### Discover Characteristics

```bash
# All characteristics
gratttool -b AA:BB:CC:DD:EE:FF --characteristics

# Output:
# handle = 0x0002, char properties = 0x02, char value handle = 0x0003, uuid = 00002a00-...

# Within a handle range
gratttool -b AA:BB:CC:DD:EE:FF --characteristics -s 0x0001 -e 0x0010

# Filter by UUID
gratttool -b AA:BB:CC:DD:EE:FF --characteristics -u 2a00
```

### Discover Descriptors

```bash
gratttool -b AA:BB:CC:DD:EE:FF --char-desc
gratttool -b AA:BB:CC:DD:EE:FF --char-desc -s 0x000a -e 0x0010

# Output:
# handle = 0x000a, uuid = 00002902-0000-1000-8000-00805f9b34fb
```

### Read Characteristic Value

```bash
# By handle
gratttool -b AA:BB:CC:DD:EE:FF --char-read -a 0x0003

# Output:
# Characteristic value/descriptor: 48 65 6c 6c 6f

# By UUID
gratttool -b AA:BB:CC:DD:EE:FF --char-read -u 2a00

# Output:
# handle: 0x0003 	 value: 48 65 6c 6c 6f
```

### Write Characteristic Value

```bash
# Write command (no response, fire and forget)
gratttool -b AA:BB:CC:DD:EE:FF --char-write -a 0x000a -n 0100

# Write request (with response)
gratttool -b AA:BB:CC:DD:EE:FF --char-write-req -a 0x000a -n 0100

# Output:
# Characteristic value was written successfully
```

### Listen for Notifications

```bash
# Listen until Ctrl-C
gratttool -b AA:BB:CC:DD:EE:FF --listen

# Output:
# Notification handle = 0x000e value: 01 02 03
# Indication   handle = 0x0012 value: ff ee dd

# Combine with another operation (e.g. enable notifications then listen)
gratttool -b AA:BB:CC:DD:EE:FF --char-write-req -a 0x000f -n 0100 --listen
```

### Enumerate Device (Table View)

Inspired by [bleah](https://github.com/hackgnar/bleah)'s `-e` flag, `--enumerate` connects to a device and displays a full overview of all services, characteristics, descriptors, and readable values in a color-coded table view using the [Catppuccin Mocha](https://github.com/catppuccin/catppuccin) color palette.

```bash
gratttool -b AA:BB:CC:DD:EE:FF --enumerate
```

A **device info table** is displayed first with metadata about the target:

```
┌ AA:BB:CC:DD:EE:FF ──────────────────────────────────────┐
│ Address      │ AA:BB:CC:DD:EE:FF                        │
│ Address Type │ Public                                    │
│ Name         │ MyDevice                                  │
│ RSSI         │ -52 dBm                                   │
│ Connected    │ Yes                                       │
│ Paired       │ No                                        │
│ Adapter      │ hci0                                      │
│ Service UUIDs│ 00001800-..., 00001801-..., 000000ff-...  │
└──────────────────────────────────────────────────────────┘
```

Followed by a **GATT attribute table** with five columns:

| Column | Description |
|--------|-------------|
| **Handles** | Service handle range (`0001 -> 0005`) or characteristic/descriptor handle |
| **Service > Characteristics** | UUIDs organized hierarchically (services, characteristics, descriptors) |
| **Properties** | Characteristic properties (READ, WRITE, NOTIFY, INDICATE, etc.) |
| **Data** | Raw hex bytes read from readable characteristics |
| **ASCII** | ASCII representation of the data (`.` for non-printable bytes) |

```
┌──────────────┬────────────────────────────────────────┬───────────┬─────────────────┬───────┐
│ Handles      │ Service > Characteristics              │ Properties│ Data            │ ASCII │
├──────────────┼────────────────────────────────────────┼───────────┼─────────────────┼───────┤
│ 0001 -> 0005 │ 00001801-0000-1000-8000-00805f9b34fb   │           │                 │       │
│         0003 │   00002a05-0000-1000-8000-00805f9b34fb │ INDICATE  │                 │       │
│              │                                        │           │                 │       │
│ 0014 -> 001c │ 00001800-0000-1000-8000-00805f9b34fb   │           │                 │       │
│         0016 │   00002a00-0000-1000-8000-00805f9b34fb │ READ      │ 48 65 6c 6c 6f  │ Hello │
│         0019 │   00002a01-0000-1000-8000-00805f9b34fb │ READ      │ c1 03           │ ..    │
│           1b │     00002902-0000-1000-8000-00805f9b34fb│           │                 │       │
│              │                                        │           │                 │       │
│ 0028 -> ffff │ 000000ff-0000-1000-8000-00805f9b34fb   │           │                 │       │
│         002a │   0000ff01-0000-1000-8000-00805f9b34fb │ READ WRITE│ 01 02 03        │ ...   │
└──────────────┴────────────────────────────────────────┴───────────┴─────────────────┴───────┘
```

The table automatically adapts to your terminal width. Data and ASCII columns wrap across multiple lines instead of truncating, so the full value is always visible. Readable characteristics are automatically read during enumeration (characteristics with only INDICATE are skipped to avoid hangs).

### Additional Options

```bash
# Use random address type
gratttool -b AA:BB:CC:DD:EE:FF -t random --primary

# Use a specific adapter
gratttool -i hci1 -b AA:BB:CC:DD:EE:FF --primary

# Set security level
gratttool -b AA:BB:CC:DD:EE:FF -l medium --char-read -a 0x0003
```

## Interactive Mode

Interactive mode provides a readline shell for exploring a BLE device. It supports command history, tab completion, and asynchronous notification display.

```bash
gratttool -I
gratttool -I -b AA:BB:CC:DD:EE:FF           # pre-set device address
gratttool -I -b AA:BB:CC:DD:EE:FF -t random  # pre-set with random address
```

### Prompt

The prompt shows the connection state and transport type:

```
[                 ][LE]>              # disconnected
[AA:BB:CC:DD:EE:FF][LE]>             # connected (shown in blue)
```

### Commands

| Command | Arguments | Description |
|---------|-----------|-------------|
| `help` | | Show all available commands |
| `exit` / `quit` | | Disconnect and exit |
| `connect` | `[address [address_type]]` | Connect to a device |
| `disconnect` | | Disconnect from current device |
| `primary` | `[UUID]` | Discover primary services |
| `included` | `[start [end]]` | Find included services |
| `characteristics` | `[start [end [UUID]]]` | Discover characteristics |
| `char-desc` | `[start [end]]` | Discover descriptors |
| `char-read-hnd` | `<handle>` | Read value by handle |
| `char-read-uuid` | `<UUID> [start [end]]` | Read value by UUID |
| `char-write-req` | `<handle> <value>` | Write with response |
| `char-write-cmd` | `<handle> <value>` | Write without response |
| `sec-level` | `[low\|medium\|high]` | Get or set security level |
| `mtu` | `<value>` | Exchange MTU |

### Interactive Session Example

```
$ gratttool -I
[                 ][LE]> connect AA:BB:CC:DD:EE:FF
Attempting to connect to AA:BB:CC:DD:EE:FF
Connection successful
[AA:BB:CC:DD:EE:FF][LE]> primary
attr handle: 0x0001, end grp handle: 0x0005 uuid: 00001800-0000-1000-8000-00805f9b34fb
attr handle: 0x0006, end grp handle: 0x0009 uuid: 00001801-0000-1000-8000-00805f9b34fb
[AA:BB:CC:DD:EE:FF][LE]> characteristics
handle: 0x0002, char properties: 0x02, char value handle: 0x0003, uuid: 00002a00-...
handle: 0x0004, char properties: 0x02, char value handle: 0x0005, uuid: 00002a01-...
[AA:BB:CC:DD:EE:FF][LE]> char-read-hnd 0003
Characteristic value/descriptor: 48 65 6c 6c 6f
[AA:BB:CC:DD:EE:FF][LE]> char-write-req 000a 0100
Characteristic value was written successfully
[AA:BB:CC:DD:EE:FF][LE]> sec-level
sec-level: low
[AA:BB:CC:DD:EE:FF][LE]> mtu 200
MTU was exchanged successfully: 200
[AA:BB:CC:DD:EE:FF][LE]> disconnect
[                 ][LE]> quit
```

Notifications and indications are printed asynchronously between prompts while connected.

## Output Format Compatibility

gratttool matches the original gatttool output format exactly:

**Non-interactive mode** (uses `=`):
```
attr handle = 0x0001, end grp handle = 0x0005 uuid: 00001800-...
handle = 0x0002, char properties = 0x02, char value handle = 0x0003, uuid = 00002a00-...
handle = 0x000a, uuid = 00002902-...
Characteristic value/descriptor: 48 65 6c 6c 6f
handle: 0x0003 	 value: 48 65 6c 6c 6f
Characteristic value was written successfully
Notification handle = 0x000e value: 01 02 03
Indication   handle = 0x0012 value: ff ee dd
Starting handle: 0001 Ending handle: 0005
```

**Interactive mode** (uses `:`):
```
attr handle: 0x0001, end grp handle: 0x0005 uuid: 00001800-...
handle: 0x0002, char properties: 0x02, char value handle: 0x0003, uuid: 00002a00-...
handle: 0x000a, uuid: 00002902-...
```

## Architecture

```
src/
  main.rs           Entry point, argument parsing, mode dispatch
  cli.rs            Clap argument definitions (matches gatttool syntax)
  connection.rs     BLE connection management via bluer
  handle_table.rs   ATT handle-to-bluer-object mapping table
  gatt.rs           GATT operations (discover, read, write, notify)
  interactive.rs    Interactive readline shell with async notifications
  output.rs         Output formatting (gatttool formats + Catppuccin enumerate table)
  error.rs          ATT error codes and application error types
```

### Handle Mapping

The original gatttool is handle-centric (`--handle 0x0004`, output shows `handle = 0x0003`), but bluer operates on D-Bus object paths. After connecting, gratttool performs full GATT discovery and builds an internal handle table mapping BlueZ's ATT handle identifiers to bluer Service/Characteristic/Descriptor objects. This allows handle-based input and output while using bluer's safe API underneath.

## License

MIT
