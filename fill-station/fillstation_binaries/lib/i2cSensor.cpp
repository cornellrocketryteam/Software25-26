#ifndef I2C_SENSOR_HPP
#define I2C_SENSOR_HPP

#include <cstdint>
#include <string>
#include <stdexcept>

#include <linux/i2c-dev.h>
#include <i2c/smbus.h>
#include <sys/ioctl.h>
#include <fcntl.h>
#include <unistd.h>
    

class I2CSensor {
private:
    int file_descriptor;
    int bus_number;
    int device_address;
public:
    I2CSensor(int bus, int address) 
        : bus_number(bus), device_address(address), file_descriptor(-1) {
        
        // Build the device path
        std::string device_path = "/dev/i2c-" + std::to_string(bus);
        
        // Open the I2C device file
        file_descriptor = open(device_path.c_str(), O_RDWR); // O_RDWR means we want to both read and write
        
        if (file_descriptor < 0) {
            throw std::runtime_error(
                "Failed to open I2C bus: " + device_path
            );
        }
        
        // Set which device address & check if it works 
        if (ioctl(file_descriptor, I2C_SLAVE, address) < 0) {
            close(file_descriptor);
            throw std::runtime_error(
                "Failed to set I2C slave address: 0x" + 
                std::to_string(address)
            );
        }
    }
    
    /**
     * Destructor - Closes the I2C connection
     */
    ~I2CSensor() {
        if (file_descriptor >= 0) {
            close(file_descriptor);
        }
    }
    
    /** 
     * Prevent Copying - Need this becasue I2CSensor owns a file descriptor (file_descriptor) — an OS resource that needs to be closed exactly once.
     * ie these are not allowed
     * I2CSensor s1(...);
     * I2CSensor s2 = s1; 
     * s2 = s1; 
     * */
    I2CSensor(const I2CSensor&) = delete;
    I2CSensor& operator=(const I2CSensor&) = delete;
    
    /**
     * Allow Moving
     * ir. allows the follwing; 
     * I2CSensor makeSensor() {
     *      I2CSensor tmp(1, 0x48);
     *      return tmp;
     * }
     * I2CSensor s = makeSensor(); <- moved from tmp
     */
    I2CSensor(I2CSensor&& other) noexcept 
        : file_descriptor(other.file_descriptor),
          bus_number(other.bus_number),
          device_address(other.device_address) {
        other.file_descriptor = -1;
    }
    

    // WRITE FUNCTIONS - Send data to the sensor
    
    /**
     * Write a single byte to a register
     * @param reg - The register address (where to write)
     * @param value - The byte value to write
     */
    void writeByte(uint8_t reg, uint8_t value) {
        int32_t result = i2c_smbus_write_byte_data(file_descriptor, reg, value);
        
        if (result < 0) {
            throw std::runtime_error(
                "Failed to write byte to register 0x" + 
                std::to_string(reg)
            );
        }
    }
    
    /**
     * Write 2 bytes to a register
     * @param reg - The register address
     * @param value - The 16-bit value to write
     */
    void write2Byte(uint8_t reg, uint16_t value) {
        int32_t result = i2c_smbus_write_word_data(file_descriptor, reg, value);
        
        if (result < 0) {
            throw std::runtime_error(
                "Failed to write word to register 0x" + 
                std::to_string(reg)
            );
        }
    }
    
    /**
     * Write multiple bytes to a register (block write)
     * @param reg - The starting register address
     * @param data - Pointer to the data to write
     * @param length - Number of bytes to write (max 32)
     */
    void writeBlock(uint8_t reg, const uint8_t* data, uint8_t length) {
        if (length > 32) {
            throw std::runtime_error("Block write limited to 32 bytes");
        }
        
        int32_t result = i2c_smbus_write_block_data(file_descriptor, reg, length, const_cast<uint8_t*>(data));
        
        if (result < 0) {
            throw std::runtime_error(
                "Failed to write block to register 0x" + 
                std::to_string(reg)
            );
        }
    }
    
    // READ FUNCTIONS - Get data from the sensor
    
    /**
     * Read a single byte from a register
     * @param reg - The register address to read from
     * @return The byte value
     */
    uint8_t readByte(uint8_t reg) {
        int32_t result = i2c_smbus_read_byte_data(file_descriptor, reg);
        
        if (result < 0) {
            throw std::runtime_error(
                "Failed to read byte from register 0x" + 
                std::to_string(reg)
            );
        }
        
        return static_cast<uint8_t>(result);
    }
    
    /**
     * Read 2 bytes from a register
     * @param reg - The register address
     * @return The 2 byte value
     */
    uint16_t readWord(uint8_t reg) {
        int32_t result = i2c_smbus_read_word_data(file_descriptor, reg);
        
        if (result < 0) {
            throw std::runtime_error(
                "Failed to read word from register 0x" + 
                std::to_string(reg)
            );
        }
        
        return static_cast<uint16_t>(result);
    }
    
    /**
     * Read multiple bytes from a register (block read)
     * @param reg - The starting register address
     * @param buffer - Where to store the read data
     * @param max_length - Maximum bytes to read (buffer size, max 32)
     * @return Number of bytes actually read
     */
    int readBlock(uint8_t reg, uint8_t* buffer, uint8_t max_length) {
        if (max_length > 32) {
            max_length = 32;  // SMBus limit
        }
        
        int32_t result = i2c_smbus_read_block_data(file_descriptor, reg, buffer);
    
        if (result < 0) {
            throw std::runtime_error(
                "Failed to read block from register 0x" + 
                std::to_string(reg)
            );
        }
        return result;
    }
    
    // utility functions
    
    /**
     * Gets the device address
     */
    int getAddress() const {
        return device_address;
    }
    
    /**
     * Gets the bus number
     */
    int getBus() const {
        return bus_number;
    }
    
    /**
     * Check if connection is valid
     */
    bool isConnected() const {
        return file_descriptor >= 0;
    }
};
#endif