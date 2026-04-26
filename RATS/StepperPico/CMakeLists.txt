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
set(PICO_BOARD pico2_w CACHE STRING "Board type")

cmake_minimum_required(VERSION 3.13)

# Initialize the Pico SDK
include(pico_sdk_import.cmake OPTIONAL)

project(StepperPico C CXX ASM)
set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

# This checks if the SDK is actually installed 
if (COMMAND pico_sdk_init)
    pico_sdk_init()

    add_executable(StepperPico
        main.cpp
        StepperMotor.cpp
        packet_simulator.cpp
        KalmanCV.cpp
        NMEAParser.cpp
        GeoMath.cpp
        AccelStepper-1.64.0/src/AccelStepper.cpp
        AccelStepper-1.64.0/src/MultiStepper.cpp
    )

    # Add the current directory (for our fake Arduino.h) and the AccelStepper source folder
    target_include_directories(StepperPico PRIVATE 
        ${CMAKE_CURRENT_LIST_DIR} 
        ${CMAKE_CURRENT_LIST_DIR}/../Common
        ${CMAKE_CURRENT_LIST_DIR}/AccelStepper-1.64.0/src
    )

    # Trick AccelStepper into thinking it's inside a modern Arduino IDE so it includes Arduino.h instead of WProgram.h
    target_compile_definitions(StepperPico PRIVATE ARDUINO=100)

    # Link the standard Pico SDK hardware libraries we need for GPIO and Timers
    target_link_libraries(StepperPico pico_stdlib hardware_gpio hardware_timer hardware_uart)

    # Enable stdio over USB and disable over UART (since GPS uses UART0)
    pico_enable_stdio_usb(StepperPico 1)
    pico_enable_stdio_uart(StepperPico 0)

    # Create the .uf2 file for dragging and dropping onto the Pico!
    pico_add_extra_outputs(StepperPico)
else()
    message(WARNING "Pico SDK not found! Please ensure 'pico_sdk_import.cmake' is present and PICO_SDK_PATH is set.")
endif()
