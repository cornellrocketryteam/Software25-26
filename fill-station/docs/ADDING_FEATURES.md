# Adding New Features to Fill Station

This guide walks you through adding new hardware components, WebSocket commands, and functionality to the fill station server.

## Table of Contents
1. [Architecture Overview](#architecture-overview)
2. [Adding a New Hardware Component](#adding-a-new-hardware-component)
3. [Adding a New WebSocket Command](#adding-a-new-websocket-command)
4. [Adding Background Tasks](#adding-background-tasks)
5. [Integrating with Main](#integrating-with-main)
6. [Testing Your Changes](#testing-your-changes)
7. [Example: Adding a Valve Controller](#example-adding-a-valve-controller)

---

## Architecture Overview

The fill station follows a clean modular architecture:

```
fill-station/
├── src/
│   ├── main.rs           # WebSocket server, background tasks, command routing
│   ├── command.rs        # Command/response definitions (WebSocket protocol)
│   ├── hardware.rs       # Hardware initialization and aggregation
│   ├── lib.rs            # Public module exports
│   └── components/       # Individual hardware drivers
│       ├── mod.rs        # Component module exports
│       ├── igniter.rs    # Example: GPIO-based igniter control
│       └── ads1015.rs    # Example: I2C ADC driver
```

### Data Flow

1. **WebSocket Client** sends JSON command → `main.rs`
2. **main.rs** deserializes → `Command` enum in `command.rs`
3. **execute_command()** routes to appropriate handler
4. Handler accesses **Hardware** (via `Arc<Mutex<>>`)
5. Hardware calls **Component** methods
6. Response serialized → sent back to client

---

## Adding a New Hardware Component

### Step 1: Create Component File

Create `src/components/your_component.rs`:

```rust
use anyhow::Result;

// Import platform-specific dependencies
#[cfg(any(target_os = "linux", target_os = "android"))]
use some_linux_crate::Device;

/// Your component struct
pub struct YourComponent {
    // Add fields for device handles, state, etc.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    device: Device,
    
    // Configuration
    name: String,
}

impl YourComponent {
    /// Initialize the component
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new(/* parameters */) -> Result<Self> {
        // Initialize hardware
        let device = Device::open("/dev/your_device")?;
        
        Ok(Self {
            device,
            name: "YourComponent".to_string(),
        })
    }
    
    /// Stub for non-Linux platforms
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new(/* parameters */) -> Result<Self> {
        Ok(Self {
            name: "YourComponent".to_string(),
        })
    }
    
    /// Example method: perform an action
    pub async fn do_something(&self) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            // Linux-specific implementation
            self.device.send_command()?;
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            // Stub for development/testing on macOS
            println!("Would do something if on Linux");
        }
        
        Ok(())
    }
    
    /// Example method: read state
    pub async fn get_state(&self) -> Result<bool> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            Ok(self.device.read_state()?)
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            Ok(false) // Mock value
        }
    }
}
```

### Step 2: Export Component

Edit `src/components/mod.rs`:

```rust
// Conditionally compile based on platform if needed
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod igniter;

pub mod ads1015;
pub mod your_component;  // Add this line
```

### Step 3: Add to Hardware Struct

Edit `src/hardware.rs`:

```rust
use crate::components::your_component::YourComponent;

pub struct Hardware {
    // Existing components
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub ig1: Igniter,
    
    pub adc1: Ads1015,
    pub adc2: Ads1015,
    
    // Add your component
    pub your_component: YourComponent,
}

impl Hardware {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new() -> Result<Self> {
        let chip = Chip::new(GPIO_CHIP).await?;
        let ig1 = Igniter::new(&chip, 18, 16).await?;
        let ig2 = Igniter::new(&chip, 24, 22).await?;
        
        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;
        
        // Initialize your component
        let your_component = YourComponent::new(/* params */).await?;
        
        Ok(Self { ig1, ig2, adc1, adc2, your_component })
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new() -> Result<Self> {
        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;
        let your_component = YourComponent::new(/* params */).await?;
        
        Ok(Self { adc1, adc2, your_component })
    }
}
```

---

## Adding a New WebSocket Command

### Step 1: Define Command Variant

Edit `src/command.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Ignite,
    StartAdcStream,
    StopAdcStream,
    
    // Add your command (with optional parameters)
    YourCommand { param1: String, param2: Option<u32> },
}
```

**JSON format from client:**
```json
{
  "command": "your_command",
  "param1": "value",
  "param2": 42
}
```

### Step 2: Add Response Type (if needed)

If your command returns data, add a response variant:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResponse {
    Success,
    Error,
    AdcData { /* ... */ },
    
    // Add your response type
    YourData {
        field1: String,
        field2: f32,
    },
}
```

### Step 3: Implement Command Handler

Edit `src/main.rs` in the `execute_command()` function:

```rust
async fn execute_command(
    command: Command, 
    hardware: &Arc<Mutex<Hardware>>,
    streaming_enabled: &mut bool,
) -> CommandResponse {
    match command {
        Command::Ignite => { /* ... */ }
        Command::StartAdcStream => { /* ... */ }
        Command::StopAdcStream => { /* ... */ }
        
        // Add your command handler
        Command::YourCommand { param1, param2 } => {
            info!("Executing your command with param1={}, param2={:?}", param1, param2);
            
            // Access hardware (async-safe)
            let hw = hardware.lock().await;
            
            // Call your component
            match hw.your_component.do_something().await {
                Ok(_) => {
                    // Optionally get state and return data
                    let state = hw.your_component.get_state().await.unwrap_or(false);
                    CommandResponse::YourData {
                        field1: format!("Processed {}", param1),
                        field2: param2.unwrap_or(0) as f32,
                    }
                }
                Err(e) => {
                    error!("Failed to execute command: {}", e);
                    CommandResponse::Error
                }
            }
        }
    }
}
```

---

## Adding Background Tasks

Background tasks run continuously and update shared state. See ADC monitoring as an example.

### Step 1: Define Shared State

In `src/main.rs`:

```rust
/// Shared state for your background task
#[derive(Debug, Clone)]
pub struct YourTaskState {
    pub timestamp_ms: u64,
    pub valid: bool,
    pub data: f32,
}

impl Default for YourTaskState {
    fn default() -> Self {
        Self {
            timestamp_ms: 0,
            valid: false,
            data: 0.0,
        }
    }
}
```

### Step 2: Create Background Task Function

```rust
/// Background task that runs continuously
async fn your_background_task(
    hardware: Arc<Mutex<Hardware>>,
    state: Arc<Mutex<YourTaskState>>,
) {
    let interval = Duration::from_millis(100); // 10 Hz
    
    info!("Your background task started");
    
    loop {
        let start = std::time::Instant::now();
        
        // Do work
        let result = {
            let hw = hardware.lock().await;
            hw.your_component.get_state().await
        };
        
        // Update shared state
        match result {
            Ok(data) => {
                let mut s = state.lock().await;
                s.timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                s.valid = true;
                s.data = data as f32;
            }
            Err(e) => {
                error!("Background task error: {}", e);
                let mut s = state.lock().await;
                s.valid = false;
            }
        }
        
        // Sleep for remainder of interval
        let elapsed = start.elapsed();
        if elapsed < interval {
            Timer::after(interval - elapsed).await;
        }
    }
}
```

### Step 3: Spawn Task in Main

In `main()` function:

```rust
smol::block_on(async {
    info!("Initializing fill station...");
    let hardware = Arc::new(Mutex::new(Hardware::new().await?));
    let adc_readings = Arc::new(Mutex::new(AdcReadings::default()));
    
    // Add your task state
    let your_state = Arc::new(Mutex::new(YourTaskState::default()));
    
    // Spawn ADC task
    let adc_hw = hardware.clone();
    let adc_readings_clone = adc_readings.clone();
    smol::spawn(adc_monitoring_task(adc_hw, adc_readings_clone)).detach();
    
    // Spawn your background task
    info!("Starting your background task...");
    let your_hw = hardware.clone();
    let your_state_clone = your_state.clone();
    smol::spawn(your_background_task(your_hw, your_state_clone)).detach();
    
    // ... rest of main
})
```

---

## Integrating with Main

### Complete Integration Checklist

When adding a new feature, update these files in order:

1. **`src/components/your_component.rs`** - Create hardware driver
2. **`src/components/mod.rs`** - Export module
3. **`src/hardware.rs`** - Add to Hardware struct and initialize
4. **`src/command.rs`** - Add command and response types
5. **`src/main.rs`** - Add command handler in `execute_command()`
6. **`src/main.rs`** (optional) - Add background task if needed
7. **`Cargo.toml`** (if needed) - Add dependencies

### Updating Dependencies

If your component needs external crates, edit `Cargo.toml`:

```toml
[dependencies]
your-crate = "1.0.0"

# Platform-specific dependencies
[target.'cfg(target_os = "linux")'.dependencies]
linux-only-crate = "2.0.0"
```

---

## Testing Your Changes

### 1. Compile Check
```bash
cd fill-station
cargo check
```

### 2. Run Server Locally
```bash
cargo run --release
```

### 3. Test WebSocket Commands

Using Python:
```python
import asyncio
import websockets
import json

async def test():
    async with websockets.connect("ws://localhost:9000") as ws:
        # Send your command
        await ws.send(json.dumps({
            "command": "your_command",
            "param1": "test",
            "param2": 42
        }))
        
        # Receive response
        response = await ws.recv()
        print(json.loads(response))

asyncio.run(test())
```

Using `websocat`:
```bash
websocat ws://localhost:9000
# Then type:
{"command": "your_command", "param1": "test", "param2": 42}
```

### 4. Unit Tests (Optional)

Add tests to your component:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_component() {
        // Test logic here
    }
}
```

---

## Example: Adding a Valve Controller

Let's walk through a complete example of adding a solenoid valve controller.

### 1. Create Component (`src/components/valve.rs`)

```rust
use anyhow::Result;

#[cfg(any(target_os = "linux", target_os = "android"))]
use async_gpiod::{Chip, LineId, Lines, Options, Output};

pub struct Valve {
    name: String,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    control_line: Lines<Output>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pin: LineId,
}

impl Valve {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new(chip: &Chip, pin: LineId, name: &str) -> Result<Self> {
        let options = Options::output([pin])
            .values([false])  // Start closed
            .consumer("fill-station-valve");
        let control_line = chip.request_lines(options).await?;
        
        Ok(Self {
            name: name.to_string(),
            control_line,
            pin,
        })
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub async fn new(_chip: &(), _pin: u8, name: &str) -> Result<Self> {
        Ok(Self {
            name: name.to_string(),
        })
    }
    
    pub async fn open(&self) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.control_line.set_values([true]).await?;
        }
        
        println!("Valve '{}' opened", self.name);
        Ok(())
    }
    
    pub async fn close(&self) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.control_line.set_values([false]).await?;
        }
        
        println!("Valve '{}' closed", self.name);
        Ok(())
    }
    
    pub async fn is_open(&self) -> Result<bool> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            Ok(*self.control_line.get_values([false]).await?.get(0).unwrap())
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            Ok(false)
        }
    }
}
```

### 2. Export Module (`src/components/mod.rs`)

```rust
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod igniter;

pub mod ads1015;
pub mod valve;  // Add this
```

### 3. Add to Hardware (`src/hardware.rs`)

```rust
use crate::components::valve::Valve;

pub struct Hardware {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub ig1: Igniter,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub ig2: Igniter,
    pub adc1: Ads1015,
    pub adc2: Ads1015,
    
    // Add valves
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub valve_lox: Valve,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub valve_fuel: Valve,
}

impl Hardware {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub async fn new() -> Result<Self> {
        let chip = Chip::new(GPIO_CHIP).await?;
        
        let ig1 = Igniter::new(&chip, 18, 16).await?;
        let ig2 = Igniter::new(&chip, 24, 22).await?;
        
        // Initialize valves on GPIO pins 20 and 21
        let valve_lox = Valve::new(&chip, 20, "LOX").await?;
        let valve_fuel = Valve::new(&chip, 21, "Fuel").await?;
        
        let adc1 = Ads1015::new(I2C_BUS, ADC1_ADDRESS)?;
        let adc2 = Ads1015::new(I2C_BUS, ADC2_ADDRESS)?;
        
        Ok(Self { ig1, ig2, adc1, adc2, valve_lox, valve_fuel })
    }
    
    // ... update non-Linux version too
}
```

### 4. Add Commands (`src/command.rs`)

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Ignite,
    StartAdcStream,
    StopAdcStream,
    
    // Valve commands
    OpenValve { valve: String },
    CloseValve { valve: String },
    GetValveState { valve: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResponse {
    Success,
    Error,
    AdcData { /* ... */ },
    
    // Valve response
    ValveState {
        valve: String,
        open: bool,
    },
}
```

### 5. Implement Handlers (`src/main.rs`)

```rust
async fn execute_command(
    command: Command, 
    hardware: &Arc<Mutex<Hardware>>,
    streaming_enabled: &mut bool,
) -> CommandResponse {
    match command {
        // ... existing commands ...
        
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Command::OpenValve { valve } => {
            let hw = hardware.lock().await;
            let result = match valve.as_str() {
                "lox" | "LOX" => hw.valve_lox.open().await,
                "fuel" | "Fuel" => hw.valve_fuel.open().await,
                _ => {
                    warn!("Unknown valve: {}", valve);
                    return CommandResponse::Error;
                }
            };
            
            match result {
                Ok(_) => CommandResponse::Success,
                Err(e) => {
                    error!("Failed to open valve: {}", e);
                    CommandResponse::Error
                }
            }
        }
        
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Command::CloseValve { valve } => {
            let hw = hardware.lock().await;
            let result = match valve.as_str() {
                "lox" | "LOX" => hw.valve_lox.close().await,
                "fuel" | "Fuel" => hw.valve_fuel.close().await,
                _ => {
                    warn!("Unknown valve: {}", valve);
                    return CommandResponse::Error;
                }
            };
            
            match result {
                Ok(_) => CommandResponse::Success,
                Err(e) => {
                    error!("Failed to close valve: {}", e);
                    CommandResponse::Error
                }
            }
        }
        
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Command::GetValveState { valve } => {
            let hw = hardware.lock().await;
            let result = match valve.as_str() {
                "lox" | "LOX" => hw.valve_lox.is_open().await,
                "fuel" | "Fuel" => hw.valve_fuel.is_open().await,
                _ => {
                    warn!("Unknown valve: {}", valve);
                    return CommandResponse::Error;
                }
            };
            
            match result {
                Ok(open) => CommandResponse::ValveState { valve, open },
                Err(e) => {
                    error!("Failed to get valve state: {}", e);
                    CommandResponse::Error
                }
            }
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        Command::OpenValve { .. } | 
        Command::CloseValve { .. } | 
        Command::GetValveState { .. } => {
            warn!("Valve commands not supported on this platform");
            CommandResponse::Error
        }
    }
}
```

### 6. Test It!

```python
import asyncio
import websockets
import json

async def test_valves():
    async with websockets.connect("ws://localhost:9000") as ws:
        # Open LOX valve
        await ws.send(json.dumps({"command": "open_valve", "valve": "lox"}))
        print(await ws.recv())
        
        # Check state
        await ws.send(json.dumps({"command": "get_valve_state", "valve": "lox"}))
        print(await ws.recv())
        
        # Close valve
        await ws.send(json.dumps({"command": "close_valve", "valve": "lox"}))
        print(await ws.recv())

asyncio.run(test_valves())
```

---

## Best Practices

1. **Use `#[cfg]` for platform-specific code** - Keeps code compilable on macOS for development
2. **Always use `Arc<Mutex<>>` for shared state** - Thread-safe across async tasks
3. **Log liberally** - Use `info!`, `warn!`, `error!` for debugging
4. **Handle errors gracefully** - Don't panic, return `CommandResponse::Error`
5. **Keep components independent** - Each component should work standalone
6. **Document your commands** - Add comments explaining JSON format
7. **Test on target platform** - GPIO/I2C only work on Linux

---

## Troubleshooting

### "Type not found" errors
- Make sure you added `use` statements in the file
- Check `#[cfg]` conditions match

### Hardware not initialized
- Did you add it to `Hardware::new()`?
- Check both Linux and non-Linux versions

### Command not recognized
- Verify JSON format matches serde tags
- Check spelling (snake_case conversion)

### Compile errors on macOS
- Add `#[cfg(not(any(target_os = "linux", target_os = "android")))]` stubs
- Use mock implementations for development

---

## Additional Resources

- **ADC Streaming Example**: See `docs/ADC_STREAMING.md`
- **Igniter Component**: See `src/components/igniter.rs`
- **Rust Async Book**: https://rust-lang.github.io/async-book/
- **Smol Runtime Docs**: https://docs.rs/smol/
