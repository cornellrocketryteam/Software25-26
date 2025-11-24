#include "ads1015_driver.hpp"

int main() {
    // Create and initialize
    ADS1015 adc(1, 0x48);  // Bus 1, address 0x48
    adc.begin();
    
    // Set voltage range
    adc.setGain(GAIN_TWOTHIRDS);  // +/- 6.144V
    
    // Read channel 0
    uint16_t raw = adc.readADC_SingleEnded(0);
    
    // Convert to voltage
    float voltage = adc.toVoltage(raw);
    printf("Voltage: %.3f V | Raw Reading: %u ADC\n", voltage, raw);
    
    return 0;
}