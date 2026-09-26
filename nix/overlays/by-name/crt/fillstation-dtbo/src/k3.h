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

/* Literal constants (not shifts) so fold-expressions.py can evaluate them */
#define PIN_DEBOUNCE_DISABLE	0x00000000
#define PIN_DEBOUNCE_CONF1	0x00000800
#define PIN_DEBOUNCE_CONF2	0x00001000
#define PIN_DEBOUNCE_CONF3	0x00001800
#define PIN_DEBOUNCE_CONF4	0x00002000
#define PIN_DEBOUNCE_CONF5	0x00002800
#define PIN_DEBOUNCE_CONF6	0x00003000

/* PADCONFIG ST_EN (bit 14); spelling matches TI SysConfig output */
#define PIN_SCMITT_TRIGGER_ENABLE	0x00004000

/* Drive-strength flags used by TI SysConfig-generated pin mux files */
#define PIN_DRIVE_STRENGTH_NOMINAL	0
#define PIN_DRIVE_STRENGTH_HIGH		0x00080000

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