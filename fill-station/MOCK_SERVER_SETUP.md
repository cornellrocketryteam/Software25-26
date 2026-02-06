# Mock Fill Station Server Setup

This guide describes how to run the Python-based Mock Fill Station Server. This is useful for frontend development (Dashboard) or testing clients without needing the actual Rust server or hardware.

## Prerequisites

- Python 3.9+
- Git

## Setup Instructions

### 1. Repository Setup

Ensure you have the repository cloned and are on the correct branch.

```bash
git clone https://github.com/cornellrocketryteam/Software25-26.git
cd Software25-26
git checkout leaktest_ready
```

### 2. Environment Setup

Navigate to the client directory:

```bash
cd fill-station/client
```

It is highly recommended to use a virtual environment to manage dependencies.

**Create a virtual environment:**

```bash
python3 -m venv client_env
```

**Activate the virtual environment:**

- **macOS/Linux:**
  ```bash
  source client_env/bin/activate
  ```
- **Windows:**
  ```bash
  .\client_env\Scripts\activate
  ```

### 3. Install Dependencies

Install the required Python packages:

```bash
pip install -r requirements.txt
```

## Running the Mock Server

Start the server using Python:

```bash
python mock_server.py
```

You should see output indicating the server has started:
```
Mock Server started on ws://0.0.0.0:9000
```

The server listens on port `9000`.

## Verification

To confirm the server is working correctly, you can run the included test script (in a separate terminal window, with the environment activated):

```bash
python test_mock.py
```

This script will:
1. Connect to the mock server.
2. Query valve states.
3. Start an ADC stream.
4. Verify data reception.

## Troubleshooting

- **Port in use:** If you see an error about the address being in use, check if another instance of the server (or the real Rust server) is running on port 9000.
- **Dependencies:** Ensure you have activated your virtual environment before running the scripts.
