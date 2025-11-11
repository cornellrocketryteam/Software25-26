#ifndef _LWIPOPTS_H
#define _LWIPOPTS_H

// --- 1. Core / System Options (NO_SYS) ---
#define NO_SYS                          1
#define LWIP_TIMERS                     1
#define PICO_CYW43_ARCH_POLL            1 // Enable polling support

// --- 2. Memory Options (Tuned for low memory) ---
#define MEM_LIBC_MALLOC                 0
#define MEMP_MEM_MALLOC                 1
#define MEM_ALIGNMENT                   4
// Total heap size for lwIP. 16KB is generous for a single MQTT client.
#define MEM_SIZE                        (16 * 1024)

// --- 3. Pbuf (Packet Buffer) Options ---
#define PBUF_POOL_SIZE                  24 // Number of pbufs in the pool
#define PBUF_POOL_BUFSIZE               1536 // Size of each pbuf (large enough for one full Wi-Fi packet)


// --- 4. TCP Options (Required for MQTT) ---
#define LWIP_TCP                        1
#define TCP_MSS                         1460 // Standard for Ethernet/Wi-Fi
#define TCP_WND                         (8 * TCP_MSS) // Receive window
#define TCP_SND_BUF                     (8 * TCP_MSS) // Send buffer
#define TCP_SND_QUEUELEN                ((4 * (TCP_SND_BUF) + (TCP_MSS - 1)) / (TCP_MSS))

#define LWIP_TCP_KEEPALIVE              1 // Keep MQTT connection alive
#define MEMP_NUM_TCP_PCB                4 // Max simultaneous TCP connections (we only need 1)
#define MEMP_NUM_TCP_SEG                32 // Max segments in-flight (Increased, matching SDK default)

// --- 5. UDP Options (Required for DNS) ---
#define LWIP_UDP                        1
#define MEMP_NUM_UDP_PCB                2 // Max simultaneous UDP "connections" (we only need 1 for DNS)

// --- 6. Application Support ---
#define LWIP_DHCP                       1 // Required to get an IP from the Wi-Fi router
#define LWIP_DNS                        1 // Required to find the MQTT broker by name
#define LWIP_NETIF_HOSTNAME             1 // Allow setting a hostname (e.g., "rats-pico")
#define LWIP_NETIF_STATUS_CALLBACK      1
#define LWIP_NETIF_LINK_CALLBACK        1

// Explicitly disable APIs we are not using to save code space.
#define LWIP_SOCKET                     0
#define LWIP_NETCONN                    0

// --- 7. Protocol Options (Disable IPv6) ---
#define LWIP_IPV4                       1
#define LWIP_IPV6                       0

// --- 8. Security Options (Disable TLS/SSL) ---
#define LWIP_ALTCP                      0 // --- DISABLED: No abstract TLS layer
#define LWIP_ALTCP_TLS                  0 // --- DISABLED: No mbedtls

// --- 9. MQTT App Options ---
// These are required by the 'lwip/apps/mqtt.h' header
#define LWIP_CALLBACK_API               1
#define MQTT_OUTPUT_RINGBUF_SIZE        2048 // 2KB buffer for outgoing MQTT messages

// --- 10. Debugging Options ---
#ifndef NDEBUG
#define LWIP_DEBUG                      1
#define LWIP_STATS                      1
#define LWIP_STATS_DISPLAY              1
#endif

// Disable all debug messages by default (can be enabled for specific modules)
#define ETHARP_DEBUG                    LWIP_DBG_OFF
#define NETIF_DEBUG                     LWIP_DBG_OFF
#define PBUF_DEBUG                      LWIP_DBG_OFF
#define API_LIB_DEBUG                   LWIP_DBG_OFF
#define API_MSG_DEBUG                   LWIP_DBG_OFF
#define SOCKETS_DEBUG                   LWIP_DBG_OFF
#define ICMP_DEBUG                      LWIP_DBG_OFF
#define INET_DEBUG                      LWIP_DBG_OFF
#define IP_DEBUG                        LWIP_DBG_OFF
#define IP_REASS_DEBUG                  LWIP_DBG_OFF
#define RAW_DEBUG                       LWIP_DBG_OFF
#define MEM_DEBUG                       LWIP_DBG_OFF
#define MEMP_DEBUG                      LWIP_DBG_OFF
#define SYS_DEBUG                       LWIP_DBG_OFF
#define TCP_DEBUG                       LWIP_DBG_OFF
#define TCP_INPUT_DEBUG                 LWIP_DBG_OFF
#define TCP_OUTPUT_DEBUG                LWIP_DBG_OFF
#define TCP_RTO_DEBUG                   LWIP_DBG_OFF
#define TCP_CWND_DEBUG                  LWIP_DBG_OFF
#define TCP_WND_DEBUG                   LWIP_DBG_OFF
#define TCP_FR_DEBUG                    LWIP_DBG_OFF
#define TCP_QLEN_DEBUG                  LWIP_DBG_OFF
#define TCP_RST_DEBUG                   LWIP_DBG_OFF
#define UDP_DEBUG                       LWIP_DBG_OFF
#define TCPIP_DEBUG                     LWIP_DBG_OFF
#define PPP_DEBUG                       LWIP_DBG_OFF
#define SLIP_DEBUG                      LWIP_DBG_OFF
#define DHCP_DEBUG                      LWIP_DBG_OFF


#endif // _LWIPOPTS_H