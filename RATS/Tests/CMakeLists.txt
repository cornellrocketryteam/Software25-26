# == DO NOT EDIT THE FOLLOWING LINES for the Raspberry Pi Pico VS Code Extension to work ==
if(WIN32)
    set(USERHOME $ENV{USERPROFILE})
else()
    set(USERHOME $ENV{HOME})
endif()
set(sdkVersion 2.2.0)
set(toolchainVersion 14_2_Rel1)
set(picotoolVersion 2.2.0-a4)
set(picoVscode ${USERHOME}/.pico-sdk/cmake/pico-vscode.cmake)
if (EXISTS ${picoVscode})
    include(${picoVscode})
endif()
# ====================================================================================
# mqtt_test CMakeLists.txt
#
# This is an isolated test for publishing simulated packets over MQTT.

cmake_minimum_required(VERSION 3.13)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# --- 1. SET BOARD (Pico 2 W) ---
set(PICO_BOARD pico2_w CACHE STRING "Board type")

# Pull in Raspberry Pi Pico SDK (must be before project)
include(pico_sdk_import.cmake)

project(mqtt_test C CXX ASM)

# Initialise the Raspberry Pi Pico SDK
pico_sdk_init()

# --- 2. ADD EXECUTABLE ---
add_executable(mqtt_test 
    mqtt_test.cpp
    ../RadioPico/mqtt_client.cpp
    ../Common/packet_parser.cpp
    packet_simulator.cpp
)

pico_set_program_name(mqtt_test "mqtt_test")
pico_set_program_version(mqtt_test "0.1")

# Enable USB output, disable UART output
pico_enable_stdio_uart(mqtt_test 0)
pico_enable_stdio_usb(mqtt_test 1)

# --- 3. ADD LIBRARIES ---
# Link all required libraries for MQTT and networking
target_link_libraries(mqtt_test
    pico_stdlib
    hardware_timer # For sleep_ms, get_absolute_time
    pico_cyw43_arch_lwip_poll 
    pico_lwip
    pico_lwip_mqtt      
)

# --- 4. ADD INCLUDE DIRECTORIES ---
# Tell the compiler where to find the .h files
target_include_directories(mqtt_test PRIVATE
    ${CMAKE_CURRENT_LIST_DIR}        # RATS/Tests/mqtt_test/
    ${CMAKE_CURRENT_LIST_DIR}/../Common  # RATS/Common/
    ${CMAKE_CURRENT_LIST_DIR}/../RadioPico # RATS/RadioPico/ (for mqtt_client.h)
)

pico_add_extra_outputs(mqtt_test)