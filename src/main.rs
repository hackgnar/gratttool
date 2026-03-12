mod bdaddr;
mod cli;
mod connection;
mod error;
mod gatt;
mod handle_table;
mod interactive;
mod output;

use clap::Parser;
use std::process;

use cli::Cli;
use error::GrattError;

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    // Handle --mtu (modifies system BlueZ config)
    if let Some(ref mtu_arg) = args.mtu {
        if mtu_arg.eq_ignore_ascii_case("show") {
            match connection::get_exchange_mtu_config() {
                Ok(val) => println!("ExchangeMTU = {} (from /etc/bluetooth/main.conf)", val),
                Err(e) => {
                    eprintln!("{}", e);
                    process::exit(1);
                }
            }
            return;
        }

        let value: u16 = match mtu_arg.parse() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("Invalid MTU value '{}': must be 23-517", mtu_arg);
                process::exit(1);
            }
        };
        if let Err(e) = connection::set_exchange_mtu_config(value) {
            eprintln!("{}", e);
            process::exit(1);
        }
        eprintln!("ExchangeMTU set to {}. Restarted bluetoothd.", value);
        if value != 517 {
            eprintln!("Remember to reset back to 517 when done: gratttool -m 517");
        }

        // If no other operation requested, just exit
        if !args.has_operation() && !args.listen && !args.interactive {
            return;
        }

        // Wait for bluetoothd to fully come back before connecting
        eprintln!("Waiting for adapter to come back online...");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // Handle --bdaddr before anything else (operates on adapter, not remote device)
    if let Some(ref bdaddr_arg) = args.bdaddr {
        if bdaddr_arg.eq_ignore_ascii_case("show") {
            if let Err(e) = bdaddr::show(&args.adapter) {
                eprintln!("{}", e);
                process::exit(1);
            }
        } else {
            if let Err(e) = bdaddr::change(
                &args.adapter,
                bdaddr_arg,
                !args.bdaddr_no_reset,
                args.bdaddr_transient,
            ) {
                eprintln!("{}", e);
                process::exit(1);
            }
        }
        return;
    }

    if args.interactive {
        if let Err(e) = interactive::run(
            &args.adapter,
            args.device.as_deref(),
            &args.addr_type,
            args.psm,
            &args.sec_level,
        )
        .await
        {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
        return;
    }

    // Non-interactive mode: need an operation
    if !args.has_operation() && !args.listen {
        // Print help via clap
        Cli::parse_from(["gratttool", "--help"]);
        process::exit(1);
    }

    // Need a device address for non-interactive operations
    let device_addr = match &args.device {
        Some(d) => d.clone(),
        None => {
            println!("Remote Bluetooth address required");
            process::exit(1);
        }
    };

    // Connect
    let address = match connection::parse_address(&device_addr) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    let addr_type = connection::parse_addr_type(&args.addr_type);

    let conn = match connection::connect(
        &args.adapter,
        address,
        addr_type,
        &args.sec_level,
        args.psm,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // If --listen is combined with another operation, subscribe to notifications
    // BEFORE running the operation so we don't miss notifications triggered by it.
    let notify_stream = if args.listen && args.has_operation() {
        match gatt::subscribe_notifications(&conn).await {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("{}", e);
                None
            }
        }
    } else {
        None
    };

    // Execute the requested operation
    let result = run_operation(&args, &conn).await;

    if let Err(e) = &result {
        eprintln!("{}", e);
    }

    // If --listen is set, wait for notifications
    if args.listen {
        if args.has_operation() {
            // We already subscribed above; just wait on the stream
            if let Err(e) = gatt::listen_on_stream(notify_stream, args.output_mode()).await {
                eprintln!("{}", e);
            }
        } else {
            // --listen alone: subscribe and wait
            if let Err(e) = gatt::listen(&conn, args.output_mode()).await {
                eprintln!("{}", e);
            }
        }
    }

    // Disconnect
    connection::disconnect(&conn.device).await.ok();

    if result.is_err() {
        process::exit(1);
    }
}

async fn run_operation(args: &Cli, conn: &connection::Connection) -> Result<(), GrattError> {
    let mode = args.output_mode();

    if args.primary {
        gatt::discover_primary(conn, args.uuid.as_deref()).await?;
    } else if args.characteristics {
        gatt::discover_characteristics(conn, args.start, args.end, args.uuid.as_deref()).await?;
    } else if args.char_read {
        if let Some(uuid) = &args.uuid {
            gatt::read_by_uuid(conn, uuid, args.start, args.end, mode).await?;
        } else if let Some(handle) = args.handle {
            gatt::read_by_handle(conn, handle, mode).await?;
        } else {
            return Err(GrattError::InvalidHandle(
                "A valid handle is required".into(),
            ));
        }
    } else if args.char_write {
        let handle = args.handle.ok_or_else(|| {
            GrattError::InvalidHandle("A valid handle is required".into())
        })?;
        let value = args.effective_value().ok_or_else(|| {
            GrattError::InvalidValue("A value is required".into())
        })?.map_err(|e| GrattError::InvalidValue(e))?;
        gatt::write_command(conn, handle, &value).await?;
    } else if args.char_write_req {
        let handle = args.handle.ok_or_else(|| {
            GrattError::InvalidHandle("A valid handle is required".into())
        })?;
        let value = args.effective_value().ok_or_else(|| {
            GrattError::InvalidValue("A value is required".into())
        })?.map_err(|e| GrattError::InvalidValue(e))?;
        gatt::write_request(conn, handle, &value).await?;
    } else if args.char_desc {
        gatt::discover_descriptors(conn, args.start, args.end).await?;
    } else if args.enumerate {
        gatt::enumerate(conn).await?;
    }

    Ok(())
}
