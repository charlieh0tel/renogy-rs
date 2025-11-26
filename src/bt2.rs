use crate::error::{RenogyError, Result};
use crate::pdu::Pdu;
use crate::transport::Transport;
use bluebus::{DeviceProxy, GattCharacteristic1Proxy};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use zbus::Connection;
use zbus::zvariant::OwnedValue;

/// BT-2 device name prefix for discovery
pub const BT2_NAME_PREFIX: &str = "BT-TH-";

/// GATT characteristic UUIDs for Renogy BT-2
/// These are the UUIDs used by the BT-2 for Modbus communication
pub const BT2_SERVICE_UUID: &str = "0000ffd0-0000-1000-8000-00805f9b34fb";
pub const BT2_WRITE_CHAR_UUID: &str = "0000ffd1-0000-1000-8000-00805f9b34fb";
pub const BT2_NOTIFY_CHAR_UUID: &str = "0000fff1-0000-1000-8000-00805f9b34fb";

/// Default timeout for Modbus responses
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// BT-2 Bluetooth transport for communicating with Renogy BMS devices.
///
/// The BT-2 acts as a transparent bridge between BLE and the RS-485 Modbus bus.
/// Modbus RTU frames are sent via GATT write and responses received via notifications.
pub struct Bt2Transport {
    connection: Arc<Connection>,
    write_char_path: String,
    notify_rx: mpsc::Receiver<Vec<u8>>,
    timeout: Duration,
}

impl Bt2Transport {
    /// Connect to a BT-2 device by its D-Bus object path.
    ///
    /// # Arguments
    /// * `device_path` - The D-Bus object path (e.g., "/org/bluez/hci0/dev_XX_XX_XX_XX_XX_XX")
    pub async fn connect(device_path: &str) -> Result<Self> {
        let connection = Arc::new(
            bluebus::get_system_connection()
                .await
                .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?,
        );

        // Build the device proxy
        let device = DeviceProxy::builder(&connection)
            .path(device_path)
            .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?
            .build()
            .await
            .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?;

        // Connect if not already connected
        if !device
            .connected()
            .await
            .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?
        {
            device
                .connect()
                .await
                .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?;

            // Wait for services to be resolved
            Self::wait_for_services(&device).await?;
        }

        // Find the GATT characteristic paths
        let (write_char_path, notify_char_path) =
            Self::find_characteristic_paths(&connection, device_path).await?;

        // Set up notification channel
        let (notify_tx, notify_rx) = mpsc::channel(16);
        Self::setup_notifications(Arc::clone(&connection), &notify_char_path, notify_tx).await?;

        Ok(Self {
            connection,
            write_char_path,
            notify_rx,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    /// Connect to a BT-2 device by its MAC address.
    ///
    /// # Arguments
    /// * `mac_address` - The MAC address (e.g., "FD:86:6D:73:XX:XX")
    /// * `adapter` - The adapter name (default "hci0")
    pub async fn connect_by_address(mac_address: &str, adapter: &str) -> Result<Self> {
        let mac_formatted = mac_address.replace(':', "_").to_uppercase();
        let device_path = format!("/org/bluez/{}/dev_{}", adapter, mac_formatted);
        Self::connect(&device_path).await
    }

    /// Set the timeout for Modbus responses.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    async fn wait_for_services(device: &DeviceProxy<'_>) -> Result<()> {
        for _ in 0..50 {
            if device
                .services_resolved()
                .await
                .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(RenogyError::Io(std::io::Error::other(
            "Timeout waiting for services to resolve",
        )))
    }

    async fn find_characteristic_paths(
        connection: &Connection,
        device_path: &str,
    ) -> Result<(String, String)> {
        let mut write_char_path = None;
        let mut notify_char_path = None;

        // Enumerate characteristic paths (charXXXX under the service)
        for service_idx in 0..10 {
            let service_path = format!("{}/service{:04x}", device_path, service_idx);

            for char_idx in 0..20 {
                let char_path = format!("{}/char{:04x}", service_path, char_idx);

                let builder_result =
                    GattCharacteristic1Proxy::builder(connection).path(char_path.as_str());

                if let Ok(builder) = builder_result
                    && let Ok(proxy) = builder.build().await
                    && let Ok(uuid) = proxy.uuid().await
                {
                    let uuid_lower = uuid.to_lowercase();
                    if uuid_lower == BT2_WRITE_CHAR_UUID {
                        write_char_path = Some(char_path.clone());
                    } else if uuid_lower == BT2_NOTIFY_CHAR_UUID {
                        notify_char_path = Some(char_path.clone());
                    }
                }

                if write_char_path.is_some() && notify_char_path.is_some() {
                    break;
                }
            }

            if write_char_path.is_some() && notify_char_path.is_some() {
                break;
            }
        }

        match (write_char_path, notify_char_path) {
            (Some(w), Some(n)) => Ok((w, n)),
            _ => Err(RenogyError::Io(std::io::Error::other(
                "Could not find BT-2 GATT characteristics",
            ))),
        }
    }

    async fn setup_notifications(
        connection: Arc<Connection>,
        notify_char_path: &str,
        tx: mpsc::Sender<Vec<u8>>,
    ) -> Result<()> {
        let notify_path = notify_char_path.to_string();

        // Spawn a task that owns the path and creates/manages the proxy
        tokio::spawn(async move {
            // Build proxy inside the task so lifetimes work out
            let proxy_result = GattCharacteristic1Proxy::builder(&connection)
                .path(notify_path.as_str())
                .ok()
                .map(|b| b.build());

            let Some(proxy_future) = proxy_result else {
                return;
            };

            let Ok(mut notify_char) = proxy_future.await else {
                return;
            };

            // Start notifications
            if notify_char.start_notify().await.is_err() {
                return;
            }

            // Monitor the Value property for changes
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                if let Ok(value) = notify_char.value().await
                    && let Some(data) = value.as_ref()
                    && !data.is_empty()
                {
                    let _ = tx.send(data.clone()).await;
                }
            }
        });

        // Give the task a moment to start notifications
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    async fn get_write_proxy(&self) -> Result<GattCharacteristic1Proxy<'_>> {
        GattCharacteristic1Proxy::builder(&self.connection)
            .path(self.write_char_path.as_str())
            .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?
            .build()
            .await
            .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))
    }
}

impl Transport for Bt2Transport {
    async fn send_receive(&mut self, pdu: &Pdu) -> Result<Pdu> {
        // Serialize the PDU to Modbus RTU frame
        let frame = pdu.serialize();

        // Clear any pending notifications
        while self.notify_rx.try_recv().is_ok() {}

        // Get write proxy and send
        let mut write_char = self.get_write_proxy().await?;
        let options: HashMap<String, OwnedValue> = HashMap::new();
        write_char
            .write_value(frame, options)
            .await
            .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?;

        // Wait for response with timeout
        let response = timeout(self.timeout, self.notify_rx.recv())
            .await
            .map_err(|_| {
                RenogyError::Io(std::io::Error::other("Timeout waiting for BT-2 response"))
            })?
            .ok_or_else(|| RenogyError::Io(std::io::Error::other("Notification channel closed")))?;

        // Deserialize the response
        Pdu::deserialize(&response)
    }

    async fn send(&mut self, pdu: &Pdu) -> Result<()> {
        let frame = pdu.serialize();
        let mut write_char = self.get_write_proxy().await?;
        let options: HashMap<String, OwnedValue> = HashMap::new();
        write_char
            .write_value(frame, options)
            .await
            .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?;
        Ok(())
    }
}

/// Helper to discover BT-2 devices.
pub async fn discover_bt2_devices() -> Result<Vec<bluebus::DeviceInfo>> {
    let devices = bluebus::list_devices().await;
    Ok(devices
        .into_iter()
        .filter(|d| {
            d.name
                .as_ref()
                .is_some_and(|n| n.starts_with(BT2_NAME_PREFIX))
        })
        .collect())
}
