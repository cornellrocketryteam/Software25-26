/* hw_config.c
 * Hardware configuration for SD card via SPI
 *
 * SD Card Pinout:
 *   CLK   -> GP10 (SPI1 SCK)
 *   CMD   -> GP11 (SPI1 MOSI)
 *   D0    -> GP12 (SPI1 MISO)
 *   CS/D3 -> GP13 (SPI1 CS)
 *   DET   -> GP22 (Card Detect, active low)
 */

#include <string.h>
#include "hw_config.h"
#include "ff.h"
#include "diskio.h"

// Hardware Configuration of SPI
static spi_t spis[] = {
    {
        .hw_inst = spi1,          // SPI1 component
        .miso_gpio = 12,          // D0 - Data out from SD card
        .mosi_gpio = 11,          // CMD - Data in to SD card
        .sck_gpio = 10,           // CLK - Clock
        .baud_rate = 12500 * 1000 // 12.5 MHz (start conservatively)
    }
};

// Hardware Configuration of SD Card
static sd_card_t sd_cards[] = {
    {
        .pcName = "0:",              // Mount as drive 0:
        .spi = &spis[0],             // Use SPI1
        .ss_gpio = 13,               // CS pin
        .use_card_detect = true,
        .card_detect_gpio = 22,      // Card detect pin
        .card_detected_true = 0      // Active low (0 = card present)
    }
};

// Required functions for the FatFS library
size_t sd_get_num() {
    return count_of(sd_cards);
}

sd_card_t *sd_get_by_num(size_t num) {
    if (num < sd_get_num()) {
        return &sd_cards[num];
    }
    return NULL;
}

size_t spi_get_num() {
    return count_of(spis);
}

spi_t *spi_get_by_num(size_t num) {
    if (num < spi_get_num()) {
        return &spis[num];
    }
    return NULL;
}
