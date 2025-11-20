#ifndef ADS1015_DRIVER_HPP
#define ADS1015_DRIVER_HPP

#include "i2c_sensor.hpp"
#include <unistd.h>
#include <cstdio>

/*=========================================================================
    I2C ADDRESS/BITS
    -----------------------------------------------------------------------*/
#define ADS1015_ADDRESS                 (0x48)    // Default I2C address
/*=========================================================================*/

/*=========================================================================
    CONVERSION DELAY (in milliseconds)
    -----------------------------------------------------------------------*/
#define ADS1015_CONVERSIONDELAY         (1)       // 1ms for ADS1015
#define ADS1115_CONVERSIONDELAY         (8)       // 8ms for ADS1115
/*=========================================================================*/

/*=========================================================================
    POINTER REGISTER
    
    The ADS1015 has 4 internal registers. You select which one to 
    read/write by first sending a "pointer" byte.
    -----------------------------------------------------------------------*/
#define ADS1015_REG_POINTER_MASK        (0x03)
#define ADS1015_REG_POINTER_CONVERT     (0x00)    // Conversion result register
#define ADS1015_REG_POINTER_CONFIG      (0x01)    // Configuration register
#define ADS1015_REG_POINTER_LOWTHRESH   (0x02)    // Low threshold register
#define ADS1015_REG_POINTER_HITHRESH    (0x03)    // High threshold register
/*=========================================================================*/

/*=========================================================================
    CONFIG REGISTER
    
    This is where you configure HOW the ADC operates:
    - Which channel to read
    - What voltage range (gain)
    - Single shot vs continuous
    - Sample rate
    -----------------------------------------------------------------------*/

// Operational Status (OS) - bit 15
#define ADS1015_REG_CONFIG_OS_MASK      (0x8000)
#define ADS1015_REG_CONFIG_OS_SINGLE    (0x8000)  // Start a single conversion
#define ADS1015_REG_CONFIG_OS_BUSY      (0x0000)  // Device is busy converting
#define ADS1015_REG_CONFIG_OS_NOTBUSY   (0x8000)  // Device is idle

// Input Multiplexer (MUX) - bits 14:12
// This selects WHICH input channel(s) to measure
#define ADS1015_REG_CONFIG_MUX_MASK     (0x7000)
#define ADS1015_REG_CONFIG_MUX_DIFF_0_1 (0x0000)  // Differential: AIN0 - AIN1
#define ADS1015_REG_CONFIG_MUX_DIFF_0_3 (0x1000)  // Differential: AIN0 - AIN3
#define ADS1015_REG_CONFIG_MUX_DIFF_1_3 (0x2000)  // Differential: AIN1 - AIN3
#define ADS1015_REG_CONFIG_MUX_DIFF_2_3 (0x3000)  // Differential: AIN2 - AIN3
#define ADS1015_REG_CONFIG_MUX_SINGLE_0 (0x4000)  // Single-ended: AIN0
#define ADS1015_REG_CONFIG_MUX_SINGLE_1 (0x5000)  // Single-ended: AIN1
#define ADS1015_REG_CONFIG_MUX_SINGLE_2 (0x6000)  // Single-ended: AIN2
#define ADS1015_REG_CONFIG_MUX_SINGLE_3 (0x7000)  // Single-ended: AIN3

// Programmable Gain Amplifier (PGA) - bits 11:9
// This sets the voltage RANGE the ADC can measure
#define ADS1015_REG_CONFIG_PGA_MASK     (0x0E00)
#define ADS1015_REG_CONFIG_PGA_6_144V   (0x0000)  // +/-6.144V range
#define ADS1015_REG_CONFIG_PGA_4_096V   (0x0200)  // +/-4.096V range
#define ADS1015_REG_CONFIG_PGA_2_048V   (0x0400)  // +/-2.048V range (default)
#define ADS1015_REG_CONFIG_PGA_1_024V   (0x0600)  // +/-1.024V range
#define ADS1015_REG_CONFIG_PGA_0_512V   (0x0800)  // +/-0.512V range
#define ADS1015_REG_CONFIG_PGA_0_256V   (0x0A00)  // +/-0.256V range

// Device operating mode - bit 8
#define ADS1015_REG_CONFIG_MODE_MASK    (0x0100)
#define ADS1015_REG_CONFIG_MODE_CONTIN  (0x0000)  // Continuous conversion
#define ADS1015_REG_CONFIG_MODE_SINGLE  (0x0100)  // Single-shot mode (default)

// Data rate - bits 7:5
#define ADS1015_REG_CONFIG_DR_MASK      (0x00E0)
#define ADS1015_REG_CONFIG_DR_128SPS    (0x0000)  // 128 samples per second
#define ADS1015_REG_CONFIG_DR_250SPS    (0x0020)  // 250 samples per second
#define ADS1015_REG_CONFIG_DR_490SPS    (0x0040)  // 490 samples per second
#define ADS1015_REG_CONFIG_DR_920SPS    (0x0060)  // 920 samples per second
#define ADS1015_REG_CONFIG_DR_1600SPS   (0x0080)  // 1600 samples per second (default)
#define ADS1015_REG_CONFIG_DR_2400SPS   (0x00A0)  // 2400 samples per second
#define ADS1015_REG_CONFIG_DR_3300SPS   (0x00C0)  // 3300 samples per second

// Comparator mode - bit 4
#define ADS1015_REG_CONFIG_CMODE_MASK   (0x0010)
#define ADS1015_REG_CONFIG_CMODE_TRAD   (0x0000)  // Traditional comparator
#define ADS1015_REG_CONFIG_CMODE_WINDOW (0x0010)  // Window comparator

// Comparator polarity - bit 3
#define ADS1015_REG_CONFIG_CPOL_MASK    (0x0008)
#define ADS1015_REG_CONFIG_CPOL_ACTVLOW (0x0000)  // Active low (default)
#define ADS1015_REG_CONFIG_CPOL_ACTVHI  (0x0008)  // Active high

// Latching comparator - bit 2
#define ADS1015_REG_CONFIG_CLAT_MASK    (0x0004)
#define ADS1015_REG_CONFIG_CLAT_NONLAT  (0x0000)  // Non-latching (default)
#define ADS1015_REG_CONFIG_CLAT_LATCH   (0x0004)  // Latching

// Comparator queue - bits 1:0
#define ADS1015_REG_CONFIG_CQUE_MASK    (0x0003)
#define ADS1015_REG_CONFIG_CQUE_1CONV   (0x0000)  // Assert after 1 conversion
#define ADS1015_REG_CONFIG_CQUE_2CONV   (0x0001)  // Assert after 2 conversions
#define ADS1015_REG_CONFIG_CQUE_4CONV   (0x0002)  // Assert after 4 conversions
#define ADS1015_REG_CONFIG_CQUE_NONE    (0x0003)  // Disable comparator (default)
/*=========================================================================*/

/**
 * Gain settings for the ADC
 * 
 * The "gain" controls the voltage RANGE the ADC can measure.
 * Higher gain = smaller range but more precision
 * 
 * Example:
 *   GAIN_TWOTHIRDS: Can measure -6.144V to +6.144V
 *   GAIN_SIXTEEN:   Can measure -0.256V to +0.256V (but more precise!)
 */
typedef enum {
    GAIN_TWOTHIRDS = ADS1015_REG_CONFIG_PGA_6_144V,  // +/- 6.144V
    GAIN_ONE       = ADS1015_REG_CONFIG_PGA_4_096V,  // +/- 4.096V
    GAIN_TWO       = ADS1015_REG_CONFIG_PGA_2_048V,  // +/- 2.048V (default)
    GAIN_FOUR      = ADS1015_REG_CONFIG_PGA_1_024V,  // +/- 1.024V
    GAIN_EIGHT     = ADS1015_REG_CONFIG_PGA_0_512V,  // +/- 0.512V
    GAIN_SIXTEEN   = ADS1015_REG_CONFIG_PGA_0_256V   // +/- 0.256V
} adsGain_t;


/**
 * ADS1015 - 12-bit ADC driver
 * 
 * What is an ADC?
 * ---------------
 * ADC = Analog to Digital Converter
 * It converts a voltage (like 2.5V) into a number your code can use.
 * 
 * The ADS1015 is a 12-bit ADC, meaning it converts voltages to numbers
 * from 0 to 4095 (2^12 - 1).
 * 
 * Example:
 *   If measuring 0-6.144V range:
 *   0V     -> 0
 *   3.072V -> 2048 (middle)
 *   6.144V -> 4095
 * 
 * Usage:
 *   ADS1015 adc(1);  // Create ADC on I2C bus 1
 *   adc.begin();     // Initialize
 *   
 *   uint16_t value = adc.readADC_SingleEnded(0);  // Read channel 0
 *   float voltage = value * 6.144 / 2048.0;      // Convert to volts
 */
class ADS1015 {
protected:
    I2CSensor* m_sensor;      // Our I2C communication object
    uint8_t m_i2cAddress;     // Device address (usually 0x48)
    uint8_t m_conversionDelay;// How long to wait for conversion (ms)
    uint8_t m_bitShift;       // Shift for 12-bit vs 16-bit results
    adsGain_t m_gain;         // Current gain setting
    int m_bus;                // I2C bus number
    
    /**
     * Write a 16-bit value to a register
     * 
     * Note: The ADS1015 expects bytes in big-endian order,
     * but SMBus writeWord sends in little-endian, so we swap bytes.
     */
    void writeRegister(uint8_t reg, uint16_t value) {
        // Swap bytes: SMBus is little-endian, ADS1015 wants big-endian
        uint16_t swapped = (value >> 8) | (value << 8);
        m_sensor->write2Byte(reg, swapped);
    }
    
    /**
     * Read a 16-bit value from a register
     * 
     * Note: SMBus readWord returns little-endian, but ADS1015 sends
     * big-endian, so we swap bytes after reading.
     */
    uint16_t readRegister(uint8_t reg) {
        // Read the 16-bit value
        uint16_t reading = m_sensor->read2Byte(reg);
        
        // Swap bytes: SMBus gives little-endian, ADS1015 sent big-endian
        return (reading >> 8) | (reading << 8);
    }

public:
    /**
     * Constructor
     * 
     * @param bus - I2C bus number (e.g., 1 for /dev/i2c-1)
     * @param address - Device address (default 0x48)
     * 
     * Example:
     *   ADS1015 adc(1);        // Bus 1, default address
     *   ADS1015 adc(1, 0x49);  // Bus 1, address 0x49
     */
    ADS1015(int bus, uint8_t address = ADS1015_ADDRESS) 
        : m_sensor(nullptr),
          m_i2cAddress(address),
          m_conversionDelay(ADS1015_CONVERSIONDELAY),
          m_bitShift(4),  // 12-bit ADC, shift right 4 bits
          m_gain(GAIN_TWOTHIRDS),
          m_bus(bus) {
    }
    
    /**
     * Destructor
     */
    virtual ~ADS1015() {
        if (m_sensor) {
            delete m_sensor;
        }
    }
    
    /**
     * Initialize the ADC
     * 
     * Call this before using any other functions!
     * 
     * Example:
     *   ADS1015 adc(1);
     *   adc.begin();  // Now ready to use
     */
    void begin() {
        printf("Setting up ADS1015 on bus %d, address 0x%02X\n", m_bus, m_i2cAddress);
        
        // Create our I2C sensor object
        m_sensor = new I2CSensor(m_bus, m_i2cAddress);
        
        if (m_sensor->isConnected()) {
            printf("ADS1015 initialized successfully!\n");
        } else {
            printf("ERROR: Failed to initialize ADS1015\n");
        }
    }
    
    /**
     * Set the gain (voltage range)
     * 
     * @param gain - One of the adsGain_t values
     * 
     * Example:
     *   adc.setGain(GAIN_ONE);  // Set to +/- 4.096V range
     */
    void setGain(adsGain_t gain) {
        m_gain = gain;
    }
    
    /**
     * Get current gain setting
     */
    adsGain_t getGain() {
        return m_gain;
    }
    
    /**
     * Read a single channel (single-ended measurement)
     * 
     * @param channel - Which channel to read (0, 1, 2, or 3)
     * @return The ADC reading (0-4095 for ADS1015)
     * 
     * "Single-ended" means we measure the voltage between the channel
     * and ground (GND).
     * 
     * Example:
     *   uint16_t value = adc.readADC_SingleEnded(0);  // Read channel 0
     *   
     *   // Convert to voltage (assuming GAIN_TWOTHIRDS = 6.144V range)
     *   float voltage = value * 6.144 / 2048.0;
     *   printf("Voltage: %.3f V\n", voltage);
     */
    uint16_t readADC_SingleEnded(uint8_t channel) {
        if (channel > 3) {
            return 0;  // Invalid channel
        }
        
        // Build the configuration value
        // Start with default settings for a basic single-shot reading
        uint16_t config = 
            ADS1015_REG_CONFIG_CQUE_NONE    |  // Disable comparator
            ADS1015_REG_CONFIG_CLAT_NONLAT  |  // Non-latching
            ADS1015_REG_CONFIG_CPOL_ACTVLOW |  // Alert active low
            ADS1015_REG_CONFIG_CMODE_TRAD   |  // Traditional comparator
            ADS1015_REG_CONFIG_DR_1600SPS   |  // 1600 samples per second
            ADS1015_REG_CONFIG_MODE_SINGLE;    // Single-shot mode
        
        // Add the gain setting
        config |= m_gain;
        
        // Select which channel to read
        switch (channel) {
            case 0: config |= ADS1015_REG_CONFIG_MUX_SINGLE_0; break;
            case 1: config |= ADS1015_REG_CONFIG_MUX_SINGLE_1; break;
            case 2: config |= ADS1015_REG_CONFIG_MUX_SINGLE_2; break;
            case 3: config |= ADS1015_REG_CONFIG_MUX_SINGLE_3; break;
        }
        
        // Set the "start conversion" bit
        config |= ADS1015_REG_CONFIG_OS_SINGLE;
        
        // Write config to start the conversion
        writeRegister(ADS1015_REG_POINTER_CONFIG, config);
        
        // Wait for conversion to complete
        usleep(1000 * m_conversionDelay);
        
        // Read and return the result
        // Shift right by m_bitShift (4 for ADS1015, 0 for ADS1115)
        return readRegister(ADS1015_REG_POINTER_CONVERT) >> m_bitShift;
    }
    
    /**
     * Read differential between channels 0 and 1
     * 
     * "Differential" means we measure the voltage DIFFERENCE between
     * two channels: (AIN0 - AIN1)
     * 
     * This can be negative if AIN1 > AIN0!
     * 
     * Example:
     *   int16_t diff = adc.readADC_Differential_0_1();
     *   // If AIN0 = 2V and AIN1 = 1V, diff will be positive
     *   // If AIN0 = 1V and AIN1 = 2V, diff will be negative
     */
    int16_t readADC_Differential_0_1() {
        uint16_t config = 
            ADS1015_REG_CONFIG_CQUE_NONE    |
            ADS1015_REG_CONFIG_CLAT_NONLAT  |
            ADS1015_REG_CONFIG_CPOL_ACTVLOW |
            ADS1015_REG_CONFIG_CMODE_TRAD   |
            ADS1015_REG_CONFIG_DR_1600SPS   |
            ADS1015_REG_CONFIG_MODE_SINGLE;
        
        config |= m_gain;
        config |= ADS1015_REG_CONFIG_MUX_DIFF_0_1;  // Differential: AIN0 - AIN1
        config |= ADS1015_REG_CONFIG_OS_SINGLE;
        
        writeRegister(ADS1015_REG_POINTER_CONFIG, config);
        usleep(1000 * m_conversionDelay);
        
        uint16_t res = readRegister(ADS1015_REG_POINTER_CONVERT) >> m_bitShift;
        
        // Handle sign extension for 12-bit values
        if (m_bitShift == 0) {
            return (int16_t)res;
        } else {
            // For 12-bit results, extend sign bit if negative
            if (res > 0x07FF) {
                res |= 0xF000;
            }
            return (int16_t)res;
        }
    }
    
    /**
     * Read differential between channels 2 and 3
     */
    int16_t readADC_Differential_2_3() {
        uint16_t config = 
            ADS1015_REG_CONFIG_CQUE_NONE    |
            ADS1015_REG_CONFIG_CLAT_NONLAT  |
            ADS1015_REG_CONFIG_CPOL_ACTVLOW |
            ADS1015_REG_CONFIG_CMODE_TRAD   |
            ADS1015_REG_CONFIG_DR_1600SPS   |
            ADS1015_REG_CONFIG_MODE_SINGLE;
        
        config |= m_gain;
        config |= ADS1015_REG_CONFIG_MUX_DIFF_2_3;  // Differential: AIN2 - AIN3
        config |= ADS1015_REG_CONFIG_OS_SINGLE;
        
        writeRegister(ADS1015_REG_POINTER_CONFIG, config);
        usleep(1000 * m_conversionDelay);
        
        uint16_t res = readRegister(ADS1015_REG_POINTER_CONVERT) >> m_bitShift;
        
        if (m_bitShift == 0) {
            return (int16_t)res;
        } else {
            if (res > 0x07FF) {
                res |= 0xF000;
            }
            return (int16_t)res;
        }
    }
    
    /**
     * Start comparator mode (continuous reading with alert)
     * 
     * The comparator will trigger an alert when the reading exceeds
     * the threshold. Useful for detecting when a sensor value goes
     * above a certain level.
     * 
     * @param channel - Which channel to monitor
     * @param threshold - Alert when reading exceeds this value
     */
    void startComparator_SingleEnded(uint8_t channel, int16_t threshold) {
        uint16_t config = 
            ADS1015_REG_CONFIG_CQUE_1CONV   |  // Alert after 1 conversion
            ADS1015_REG_CONFIG_CLAT_LATCH   |  // Latching mode
            ADS1015_REG_CONFIG_CPOL_ACTVLOW |
            ADS1015_REG_CONFIG_CMODE_TRAD   |
            ADS1015_REG_CONFIG_DR_1600SPS   |
            ADS1015_REG_CONFIG_MODE_CONTIN;    // Continuous mode
        
        config |= m_gain;
        
        switch (channel) {
            case 0: config |= ADS1015_REG_CONFIG_MUX_SINGLE_0; break;
            case 1: config |= ADS1015_REG_CONFIG_MUX_SINGLE_1; break;
            case 2: config |= ADS1015_REG_CONFIG_MUX_SINGLE_2; break;
            case 3: config |= ADS1015_REG_CONFIG_MUX_SINGLE_3; break;
        }
        
        // Set high threshold
        writeRegister(ADS1015_REG_POINTER_HITHRESH, threshold << m_bitShift);
        
        // Write config to start continuous conversion
        writeRegister(ADS1015_REG_POINTER_CONFIG, config);
    }
    
    /**
     * Get the last conversion result (for comparator mode)
     */
    int16_t getLastConversionResults() {
        usleep(1000 * m_conversionDelay);
        
        uint16_t res = readRegister(ADS1015_REG_POINTER_CONVERT) >> m_bitShift;
        
        if (m_bitShift == 0) {
            return (int16_t)res;
        } else {
            if (res > 0x07FF) {
                res |= 0xF000;
            }
            return (int16_t)res;
        }
    }
    
    /**
     * Convert raw ADC reading to voltage
     * 
     * @param reading - The raw ADC value
     * @return Voltage in volts
     * 
     * Example:
     *   uint16_t raw = adc.readADC_SingleEnded(0);
     *   float volts = adc.toVoltage(raw);
     *   printf("Voltage: %.3f V\n", volts);
     */
    float toVoltage(uint16_t reading) {
        float fsRange;
        
        // Determine full-scale range based on gain
        switch (m_gain) {
            case GAIN_TWOTHIRDS: fsRange = 6.144f; break;
            case GAIN_ONE:       fsRange = 4.096f; break;
            case GAIN_TWO:       fsRange = 2.048f; break;
            case GAIN_FOUR:      fsRange = 1.024f; break;
            case GAIN_EIGHT:     fsRange = 0.512f; break;
            case GAIN_SIXTEEN:   fsRange = 0.256f; break;
            default:             fsRange = 6.144f; break;
        }
        
        // For ADS1015: 12-bit = 4096 counts for full scale
        float divisor = (m_bitShift == 0) ? 32768.0f : 2048.0f;
        
        return (reading * fsRange) / divisor;
    }
};


// /**
//  * ADS1115 - 16-bit ADC driver
//  * 
//  * Same as ADS1015 but with higher resolution (16-bit instead of 12-bit).
//  * This means more precise readings but slower conversion.
//  * 
//  * ADS1015: 0-4095 (12-bit)
//  * ADS1115: 0-65535 (16-bit)
//  */
// class ADS1115 : public ADS1015 {
// public:
//     ADS1115(int bus, uint8_t address = ADS1015_ADDRESS) 
//         : ADS1015(bus, address) {
//         m_conversionDelay = ADS1115_CONVERSIONDELAY;  // 8ms instead of 1ms
//         m_bitShift = 0;  // No shift needed for 16-bit
//     }
// };

#endif // ADS1115_DRIVER_HPP