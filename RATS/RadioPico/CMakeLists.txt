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
# RadioPico CMakeLists.txt

cmake_minimum_required(VERSION 3.13)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Initialise pico_sdk from installed location
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

set(PICO_BOARD pico2_w CACHE STRING "Board type")

# Pull in Raspberry Pi Pico SDK (must be before project)
include(pico_sdk_import.cmake)

project(RadioPico C CXX ASM)


# Initialise the Raspberry Pi Pico SDK
pico_sdk_init()

# Add executable with all source files
add_executable(RadioPico 
    RadioPico.cpp
    rfd900x_uart.cpp
    mqtt_client.cpp
    ../Common/packet_parser.cpp
    ../Tests/packet_simulator.cpp
)

pico_set_program_name(RadioPico "RadioPico")
pico_set_program_version(RadioPico "0.1")

# Enable USB output, disable UART output
pico_enable_stdio_uart(RadioPico 0)
pico_enable_stdio_usb(RadioPico 1)

# Add libraries
target_link_libraries(RadioPico
    pico_stdlib
    hardware_uart
    hardware_irq
    pico_multicore
    pico_cyw43_arch_lwip_poll
    pico_lwip_mqtt
)

# Include directories
target_include_directories(RadioPico PRIVATE
    ${CMAKE_CURRENT_LIST_DIR}
    ${CMAKE_CURRENT_LIST_DIR}/../Common
    ${CMAKE_CURRENT_LIST_DIR}/../Tests
)

pico_add_extra_outputs(RadioPico)