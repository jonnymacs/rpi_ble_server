use bluer::{
    AdapterEvent, Session, Uuid, adv::{ Advertisement }, gatt::{
        local::{
            Application, Characteristic, Service, CharacteristicWrite, CharacteristicWriteMethod
        }
    }
};
use std::collections::BTreeSet;
use futures::stream::StreamExt;
use std::process::Command;

#[tokio::main]
async fn main() -> bluer::Result<()> {

    let characteristic_uuid = Uuid::from_u128(0x00002A2B_0000_1000_8000_00805F9B34FB); // Current Time Characteristic UUID
    let service_uuid = Uuid::from_u128(0x00001805_0000_1000_8000_00805F9B34FB); // Service UUID

    let session = Session::new().await?;
    let adapter = session.default_adapter().await?;
    println!("Adapter found: {}", adapter.name());
    adapter.set_powered(true).await?;
    println!("Adapter powered on: {}", adapter.is_powered().await?);
    adapter.set_discoverable(true).await?;
    println!("Adapter discoverable: {}", adapter.is_discoverable().await?);

    println!(
        "Starting BLE server on adapter {} with address {}",
        adapter.name(),
        adapter.address().await?
    );

    // Define the Current Time characteristic
    let current_time_char = Characteristic {
        uuid: characteristic_uuid,
        write: Some(CharacteristicWrite {
            write: true,
            write_without_response: true,
            method: CharacteristicWriteMethod::Fun(Box::new(move |value, _req| {
                Box::pin(async move {
                    println!("Received write: {:?}", value);
                    if value.len() > 8 {
                        // Extract 8-byte timestamp
                        let timestamp_bytes: [u8; 8] = value[..8].try_into().unwrap();
                        let timestamp = f64::from_le_bytes(timestamp_bytes);
                        println!("Received timestamp: {}", timestamp);

                        // Extract timezone (remaining bytes)
                        let timezone_bytes = &value[8..];

                        // Validate timestamp
                        if timestamp >= 0.0 && timestamp <= 4102444800.0 && timestamp.is_finite() {
                            if let Ok(timezone) = String::from_utf8(timezone_bytes.to_vec()) {
                                println!("Received timezone: {}", timezone);
                                // Set system time
                                let timestamp_secs = timestamp as i64;
                                let set_time_cmd = format!("set-time @{} {}", timestamp_secs, timezone);
                                match Command::new("sh")
                                    .arg("-c")
                                    .arg(&set_time_cmd)
                                    .status()
                                {
                                    Ok(status) if status.success() => println!("System time set to: {}, {}", timestamp, timezone),
                                    Ok(status) => println!("Failed to set system time, exit code: {}", status),
                                    Err(e) => println!("Failed to execute date command: {:?}", e),
                                }
                            } else {
                                println!("Invalid timezone data: {:?}", timezone_bytes);
                            }
                        } else {
                            println!("Invalid timestamp: {} (out of range or not finite)", timestamp);
                        }
                    } else {
                        println!("Invalid data length: {}", value.len());
                    }
                    println!("Write handler completed");
                    Ok(())
                }) // Return a pinned future
            })),
            // Disable encryption/bonding
            encrypt_write: false,
            encrypt_authenticated_write: false,
            secure_write: false,
            ..Default::default()
        }),
        ..Default::default()
    };

    // Define the Current Time Service
    let service = Service {
        uuid: service_uuid, // Current Time Service UUID
        primary: true,
        characteristics: vec![current_time_char],
        ..Default::default()
    };

    // Create and register the GATT application
    let app = Application {
        services: vec![service],
        ..Default::default()
    };

    // Create a GATT server and serve the application
    let _server = adapter.serve_gatt_application(app).await?;
    println!("GATT server running!");

    // Define the advertisement
    let adv = Advertisement {
        service_uuids: vec![service_uuid]
            .into_iter()
            .collect::<BTreeSet<Uuid>>(),
        local_name: Some("SetRPITime".to_string()), // Optional: set a friendly name
        discoverable: Some(true),
        //timeout: Some(std::time::Duration::from_secs(180)),  // Optional: stop advertising after 3 minutes
        ..Default::default()
    };

    // Advertise the service using Advertisement
    let _adv_handle = adapter.advertise(adv).await?;
    println!("Advertising TimeCubeBLEServer");

    // Handle adapter-level events
    let mut events = adapter.events().await?;
    while let Some(event) = events.next().await {
        if let AdapterEvent::DeviceAdded(addr) = event {
            println!("Device connected: {}", adapter.device(addr)?.address());
        }
    }
    
    // code to kill the BLE server after 180 seconds
    // we might want to explore this later for lower power consumption
    //
    // tokio::select! {
    //     _ = async {
    //         while let Some(event) = events.next().await {
    //             if let AdapterEvent::DeviceAdded(addr) = event {
    //                 println!("Device connected: {}", adapter.device(addr)?.address());
    //             }
    //         }
    //         Ok::<(), bluer::Error>(())
    //     } => {},
    //     _ = tokio::time::sleep(Duration::from_secs(180)) => {
    //         println!("Shutting down after 3 minutes");
    //     }
    // }

    Ok(())
}
