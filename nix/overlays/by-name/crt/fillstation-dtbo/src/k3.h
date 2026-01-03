#ifndef _DT_BINDINGS_PINCTRL_TI_K3_H
#define _DT_BINDINGS_PINCTRL_TI_K3_H

#define PULLUDEN_SHIFT		(16)
#define PULLTYPESEL_SHIFT	(17)
#define RXACTIVE_SHIFT		(18)
#define DEBOUNCE_SHIFT		(11)

#define PULL_DISABLE		(1 << PULLUDEN_SHIFT)
#define PULL_ENABLE		(0 << PULLUDEN_SHIFT)

#define PULL_UP			(1 << PULLTYPESEL_SHIFT | PULL_ENABLE)
#define PULL_DOWN		(0 << PULLTYPESEL_SHIFT | PULL_ENABLE)

#define INPUT_EN		(1 << RXACTIVE_SHIFT)
#define INPUT_DISABLE		(0 << RXACTIVE_SHIFT)

/* Only these macros are expected be used directly in device tree files */
#define PIN_OUTPUT          0x00010000
#define PIN_OUTPUT_PULLUP   0x00020000
#define PIN_OUTPUT_PULLDOWN 0x00000000
#define PIN_INPUT           0x00050000
#define PIN_INPUT_PULLUP    0x00060000
#define PIN_INPUT_PULLDOWN  0x00040000

#define PIN_DEBOUNCE_DISABLE	(0 << DEBOUNCE_SHIFT)
#define PIN_DEBOUNCE_CONF1	(1 << DEBOUNCE_SHIFT)
#define PIN_DEBOUNCE_CONF2	(2 << DEBOUNCE_SHIFT)
#define PIN_DEBOUNCE_CONF3	(3 << DEBOUNCE_SHIFT)
#define PIN_DEBOUNCE_CONF4	(4 << DEBOUNCE_SHIFT)
#define PIN_DEBOUNCE_CONF5	(5 << DEBOUNCE_SHIFT)
#define PIN_DEBOUNCE_CONF6	(6 << DEBOUNCE_SHIFT)

#define AM62AX_IOPAD(pa, val, muxmode)		(((pa) & 0x1fff)) ((val) | (muxmode))
#define AM62AX_MCU_IOPAD(pa, val, muxmode)	(((pa) & 0x1fff)) ((val) | (muxmode))

#define AM62X_IOPAD(pa, val, muxmode)		(((pa) & 0x1fff)) ((val) | (muxmode))
#define AM62X_MCU_IOPAD(pa, val, muxmode)	(((pa) & 0x1fff)) ((val) | (muxmode))

#define AM64X_IOPAD(pa, val, muxmode)  (pa), ((val) + (muxmode))
#define AM64X_MCU_IOPAD(pa, val, muxmode)	(((pa) & 0x1fff)) ((val) | (muxmode))

#define AM65X_IOPAD(pa, val, muxmode)		(((pa) & 0x1fff)) ((val) | (muxmode))
#define AM65X_WKUP_IOPAD(pa, val, muxmode)	(((pa) & 0x1fff)) ((val) | (muxmode))

#define J721E_IOPAD(pa, val, muxmode)		(((pa) & 0x1fff)) ((val) | (muxmode))
#define J721E_WKUP_IOPAD(pa, val, muxmode)	(((pa) & 0x1fff)) ((val) | (muxmode))

#define J721S2_IOPAD(pa, val, muxmode)		(((pa) & 0x1fff)) ((val) | (muxmode))
#define J721S2_WKUP_IOPAD(pa, val, muxmode)	(((pa) & 0x1fff)) ((val) | (muxmode))

#define J784S4_IOPAD(pa, val, muxmode)		(((pa) & 0x1fff)) ((val) | (muxmode))
#define J784S4_WKUP_IOPAD(pa, val, muxmode)	(((pa) & 0x1fff)) ((val) | (muxmode))

#endif