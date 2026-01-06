// I2C Configuration

/// I2C bus frequency in Hz (400kHz - Fast Mode)
pub const I2C_FREQUENCY: u32 = 400_000;

// I2C Pin Assignments

/// I2C0 SDA (Data) pin
pub const I2C_SDA_PIN: u8 = 0;

/// I2C0 SCL (Clock) pin
pub const I2C_SCL_PIN: u8 = 1;

// SPI Configuration

/// SPI bus frequency in Hz (1MHz for FRAM)
pub const SPI_FREQUENCY: u32 = 1_000_000;

// SPI Pin Assignments

/// SPI0 MISO (Master In, Slave Out) pin
pub const SPI_MISO_PIN: u8 = 16;

/// SPI0 CS (Chip Select) pin for FRAM
pub const SPI_CS_PIN: u8 = 17;

/// SPI0 CLK (Clock) pin
pub const SPI_CLK_PIN: u8 = 18;

/// SPI0 MOSI (Master Out, Slave In) pin
pub const SPI_MOSI_PIN: u8 = 19;

// UART Configuration

/// UART1 baudrate for RFD900x radio (115200 baud, 8N1)
pub const UART_BAUDRATE: u32 = 115200;

// UART Pin Assignments

/// UART1 TX (Transmit) pin for RFD900x radio
pub const UART_TX_PIN: u8 = 4;

/// UART1 RX (Receive) pin for RFD900x radio
pub const UART_RX_PIN: u8 = 5;

// GPIO Pin Assignments

/// Onboard LED pin
pub const LED_PIN: u8 = 25;

// Timing Configuration

/// Main loop cycle time in milliseconds
pub const MAIN_LOOP_DELAY_MS: u64 = 1000;

// USB Logger Configuration

/// USB logger ring buffer size in bytes
pub const USB_LOGGER_BUFFER_SIZE: usize = 1024;
