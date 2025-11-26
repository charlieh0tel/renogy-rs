use crate::error::Result;
use std::future::Future;

/// Transport trait for Modbus communication over any physical layer.
///
/// Implementations handle the physical layer details (serial, BLE, TCP, etc.)
/// while providing a consistent high-level Modbus API.
pub trait Transport {
    /// Read holding registers from a device.
    ///
    /// # Arguments
    /// * `slave` - Modbus slave address
    /// * `addr` - Starting register address
    /// * `quantity` - Number of registers to read
    ///
    /// # Returns
    /// Vector of register values (u16)
    fn read_holding_registers(
        &mut self,
        slave: u8,
        addr: u16,
        quantity: u16,
    ) -> impl Future<Output = Result<Vec<u16>>> + Send;

    /// Write a single register to a device.
    ///
    /// # Arguments
    /// * `slave` - Modbus slave address
    /// * `addr` - Register address
    /// * `value` - Value to write
    fn write_single_register(
        &mut self,
        slave: u8,
        addr: u16,
        value: u16,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Write multiple registers to a device.
    ///
    /// # Arguments
    /// * `slave` - Modbus slave address
    /// * `addr` - Starting register address
    /// * `values` - Values to write
    fn write_multiple_registers(
        &mut self,
        slave: u8,
        addr: u16,
        values: &[u16],
    ) -> impl Future<Output = Result<()>> + Send;

    /// Send a custom function code request.
    ///
    /// Used for non-standard Modbus functions like RestoreFactoryDefault (0x78)
    /// and ClearHistory (0x79).
    ///
    /// # Arguments
    /// * `slave` - Modbus slave address
    /// * `function_code` - Custom function code
    /// * `data` - Request data
    ///
    /// # Returns
    /// Response data (without address, function code, or CRC)
    fn send_custom(
        &mut self,
        slave: u8,
        function_code: u8,
        data: &[u8],
    ) -> impl Future<Output = Result<Vec<u8>>> + Send;
}
