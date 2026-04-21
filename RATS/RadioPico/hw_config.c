/* hw_config.c
 * Hardware configuration for SD card via SPI
 * Using no-OS-FatFS-SD-SDIO-SPI-RPi-Pico library
 *
 * SD Card Pinout:
 *   CLK   -> GP10 (SPI1 SCK)
 *   CMD   -> GP11 (SPI1 MOSI)
 *   D0    -> GP12 (SPI1 MISO)
 *   CS/D3 -> GP13 (SPI1 CS)
 *   DET   -> GP22 (Card Detect, active low)
 */

#include "hw_config.h"

// Hardware Configuration of SPI
static spi_t spi = {
    .hw_inst = spi1,          // SPI1 component
    .sck_gpio = 10,           // CLK - Clock
    .mosi_gpio = 11,          // CMD - Data in to SD card
    .miso_gpio = 12,          // D0 - Data out from SD card
    .baud_rate = 12500 * 1000 // 12.5 MHz (conservative, can increase to 25MHz for better performance)
};

// SPI Interface
static sd_spi_if_t spi_if = {
    .spi = &spi,              // Pointer to the SPI driving this card
    .ss_gpio = 13             // CS pin
};

// Configuration of the SD Card socket object
static sd_card_t sd_card = {
    .type = SD_IF_SPI,
    .spi_if_p = &spi_if,      // Pointer to the SPI interface driving this card
    .use_card_detect = false, // DISABLED - not reliable on this breakout
    .card_detect_gpio = 22,   // Card detect pin (not used)
    .card_detected_true = 1   // Active HIGH (1 = card present) - breakout logic is inverted
};

/* ********************************************************************** */

size_t sd_get_num() {
    return 1;
}

sd_card_t *sd_get_by_num(size_t num) {
    if (0 == num) {
        return &sd_card;
    } else {
        return NULL;
    }
}
