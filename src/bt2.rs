use crate::error::{RenogyError, Result};
use crate::pdu::{FunctionCode, Pdu};
use crate::transport::Transport;
use bluebus::{DeviceProxy, GattCharacteristic1Proxy};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use zbus::Connection;

pub const BT2_NAME_PREFIX: &str = "BT-TH-";
pub const BT2_WRITE_CHAR_UUID: &str = "0000ffd1-0000-1000-8000-00805f9b34fb";
pub const BT2_NOTIFY_CHAR_UUID: &str = "0000fff1-0000-1000-8000-00805f9b34fb";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// BT-2 Bluetooth transport for communicating with Renogy BMS devices.
pub struct Bt2Transport {
    connection: Arc<Connection>,
    write_char_path: String,
    notify_rx: mpsc::Receiver<Vec<u8>>,
    timeout: Duration,
}

impl Bt2Transport {
    pub async fn connect(device_path: &str) -> Result<Self> {
        let connection = Arc::new(bluebus::get_system_connection().await?);

        let device = DeviceProxy::builder(&connection)
            .path(device_path)?
            .build()
            .await?;

        if !device.connected().await? {
            device.connect().await?;
            Self::wait_for_services(&device).await?;
        }

        let (write_char_path, notify_char_path) =
            Self::find_characteristics(&connection, device_path).await?;

        let (tx, notify_rx) = mpsc::channel(16);
        Self::spawn_notification_listener(Arc::clone(&connection), notify_char_path, tx);

        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(Self {
            connection,
            write_char_path,
            notify_rx,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    pub async fn connect_by_address(mac_address: &str, adapter: &str) -> Result<Self> {
        let mac_formatted = mac_address.replace(':', "_").to_uppercase();
        Self::connect(&format!("/org/bluez/{adapter}/dev_{mac_formatted}")).await
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    async fn wait_for_services(device: &DeviceProxy<'_>) -> Result<()> {
        for _ in 0..50 {
            if device.services_resolved().await? {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(RenogyError::Io(std::io::Error::other(
            "Timeout waiting for services to resolve",
        )))
    }

    async fn find_characteristics(
        connection: &Connection,
        device_path: &str,
    ) -> Result<(String, String)> {
        let mut write_path = None;
        let mut notify_path = None;

        for service_idx in 0..10 {
            for char_idx in 0..20 {
                let path = format!("{device_path}/service{service_idx:04x}/char{char_idx:04x}");

                let Some(proxy) = GattCharacteristic1Proxy::builder(connection)
                    .path(path.as_str())
                    .ok()
                    .map(zbus::proxy::Builder::build)
                else {
                    continue;
                };
                let Ok(proxy) = proxy.await else {
                    continue;
                };

                let Ok(uuid) = proxy.uuid().await else {
                    continue;
                };

                match uuid.to_lowercase().as_str() {
                    BT2_WRITE_CHAR_UUID => write_path = Some(path),
                    BT2_NOTIFY_CHAR_UUID => notify_path = Some(path),
                    _ => {}
                }

                if let (Some(w), Some(n)) = (&write_path, &notify_path) {
                    return Ok((w.clone(), n.clone()));
                }
            }
        }

        write_path
            .zip(notify_path)
            .ok_or_else(|| RenogyError::Io(std::io::Error::other("BT-2 characteristics not found")))
    }

    fn spawn_notification_listener(
        connection: Arc<Connection>,
        notify_path: String,
        tx: mpsc::Sender<Vec<u8>>,
    ) {
        tokio::spawn(async move {
            let Some(proxy) = GattCharacteristic1Proxy::builder(&connection)
                .path(notify_path.as_str())
                .ok()
                .map(zbus::proxy::Builder::build)
            else {
                return;
            };
            let Ok(mut char) = proxy.await else {
                return;
            };

            if char.start_notify().await.is_err() {
                return;
            }

            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                if let Ok(value) = char.value().await
                    && let Some(data) = value.as_ref()
                    && !data.is_empty()
                {
                    let _ = tx.send(data.clone()).await;
                }
            }
        });
    }

    async fn send_pdu(&mut self, pdu: &Pdu) -> Result<Pdu> {
        let frame = pdu.serialize();

        while self.notify_rx.try_recv().is_ok() {}

        let mut write_char = GattCharacteristic1Proxy::builder(&self.connection)
            .path(self.write_char_path.as_str())?
            .build()
            .await?;

        write_char
            .write_value(frame, std::collections::HashMap::new())
            .await?;

        let response = timeout(self.timeout, self.notify_rx.recv())
            .await
            .map_err(|_| RenogyError::Io(std::io::Error::other("Timeout waiting for response")))?
            .ok_or_else(|| RenogyError::Io(std::io::Error::other("Notification channel closed")))?;

        Pdu::deserialize(&response)
    }
}

impl Transport for Bt2Transport {
    async fn read_holding_registers(
        &mut self,
        slave: u8,
        addr: u16,
        quantity: u16,
    ) -> Result<Vec<u16>> {
        let mut payload = Vec::with_capacity(4);
        payload.extend_from_slice(&addr.to_be_bytes());
        payload.extend_from_slice(&quantity.to_be_bytes());

        let response = self
            .send_pdu(&Pdu::new(
                slave,
                FunctionCode::ReadHoldingRegisters,
                payload,
            ))
            .await?;

        let byte_count = *response.payload.first().ok_or(RenogyError::InvalidData)? as usize;
        if response.payload.len() < 1 + byte_count {
            return Err(RenogyError::InvalidData);
        }

        Ok(response.payload[1..]
            .chunks_exact(2)
            .take(quantity as usize)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect())
    }

    async fn write_single_register(&mut self, slave: u8, addr: u16, value: u16) -> Result<()> {
        let mut payload = Vec::with_capacity(4);
        payload.extend_from_slice(&addr.to_be_bytes());
        payload.extend_from_slice(&value.to_be_bytes());

        self.send_pdu(&Pdu::new(slave, FunctionCode::WriteSingleRegister, payload))
            .await?;
        Ok(())
    }

    async fn write_multiple_registers(
        &mut self,
        slave: u8,
        addr: u16,
        values: &[u16],
    ) -> Result<()> {
        let mut payload = Vec::with_capacity(5 + values.len() * 2);
        payload.extend_from_slice(&addr.to_be_bytes());
        payload.extend_from_slice(&(values.len() as u16).to_be_bytes());
        payload.push((values.len() * 2) as u8);
        for value in values {
            payload.extend_from_slice(&value.to_be_bytes());
        }

        self.send_pdu(&Pdu::new(
            slave,
            FunctionCode::WriteMultipleRegisters,
            payload,
        ))
        .await?;
        Ok(())
    }

    async fn send_custom(&mut self, slave: u8, function_code: u8, data: &[u8]) -> Result<Vec<u8>> {
        let fc = FunctionCode::from_u8(function_code).ok_or(RenogyError::InvalidData)?;
        Ok(self
            .send_pdu(&Pdu::new(slave, fc, data.to_vec()))
            .await?
            .payload)
    }
}

pub async fn discover_bt2_devices() -> Result<Vec<bluebus::DeviceInfo>> {
    Ok(bluebus::list_devices()
        .await
        .into_iter()
        .filter(|d| {
            d.name
                .as_ref()
                .is_some_and(|n| n.starts_with(BT2_NAME_PREFIX))
        })
        .collect())
}
