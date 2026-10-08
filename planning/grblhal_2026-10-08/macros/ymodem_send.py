#!/usr/bin/env python3
"""Send one file to the grblHAL SD card with YModem.

Usage: python3 ymodem_send.py /dev/ttyACM0 ymtest.txt [name_on_card]

The grblHAL receiver (sdcard/ymodem.c) does not send the initial 'C'.
The sender starts the transfer with the first packet (ymodem.c:8-9,
trap_initial_soh at ymodem.c:358). This script does that.

Requirements:
- pyserial (python3 -m pip install pyserial).
- No other program holds the port. Disconnect gSender first.
- The controller is Idle, and the SD card is mounted.

Protocol facts from sdcard/ymodem.c (driver commit bfe5d4f):
- Packet = SOH (128 bytes) or STX (1024 bytes), number, 255 - number,
  payload, CRC16 high byte first (await_crc, ymodem.c:204-263).
- The CRC is CCITT, polynomial 0x1021, start value 0 (grbl/crc.c:62-73).
  binascii.crc_hqx(data, 0) gives the same value.
- Packet 0 holds "name NUL length NUL". The receiver replies ACK and 'C'
  (ymodem.c:220-235, 305-309).
- After EOT the receiver replies ACK and 'C' (end_transfer, ymodem.c:115-130).
- A packet 0 with an empty name ends the batch with ACK (ymodem.c:311-314).
"""

import binascii
import os
import sys
import time

SOH, STX, EOT, ACK, NAK, CAN = 0x01, 0x02, 0x04, 0x06, 0x15, 0x18


def packet(num, payload, size):
    data = payload.ljust(size, b"\x1a" if num else b"\x00")
    crc = binascii.crc_hqx(data, 0)
    head = SOH if size == 128 else STX
    return bytes([head, num & 0xFF, 0xFF - (num & 0xFF)]) + data + bytes([crc >> 8, crc & 0xFF])


def wait_reply(port, timeout=5.0):
    """Return ACK, NAK or CAN. Ignore other bytes (text output of the controller)."""
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        b = port.read(1)
        if not b:
            continue
        if b[0] in (ACK, NAK, CAN):
            return b[0]
    return None


def send_packet(port, pkt, what):
    for attempt in range(5):
        port.write(pkt)
        port.flush()
        r = wait_reply(port)
        if r == ACK:
            return
        if r == CAN:
            sys.exit(f"The controller cancelled the transfer at {what}.")
        print(f"{what}: no ACK (reply {r}), attempt {attempt + 1} of 5")
    sys.exit(f"Transfer failed at {what}.")


def send(port, path, name):
    data = open(path, "rb").read()
    header = name.encode("ascii") + b"\x00" + str(len(data)).encode("ascii") + b"\x00"
    if len(header) > 128:
        sys.exit("The file name is too long.")
    if len(name) > 31:
        sys.exit("The receiver keeps 31 characters of the name (ymodem.c:52).")
    port.reset_input_buffer()
    send_packet(port, packet(0, header, 128), "packet 0 (file name)")
    num = 1
    for i in range(0, len(data), 1024):
        send_packet(port, packet(num, data[i:i + 1024], 1024), f"packet {num}")
        num += 1
    send_packet(port, bytes([EOT]), "EOT")
    time.sleep(0.2)
    send_packet(port, packet(0, b"", 128), "end of batch")
    print(f"Sent {len(data)} bytes as {name}. Check it with $F+ and $F<={name}")


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    import serial  # pyserial

    dev, path = sys.argv[1], sys.argv[2]
    name = sys.argv[3] if len(sys.argv) > 3 else os.path.basename(path)
    with serial.Serial(dev, 115200, timeout=0.1) as port:
        send(port, path, name)


if __name__ == "__main__":
    main()
