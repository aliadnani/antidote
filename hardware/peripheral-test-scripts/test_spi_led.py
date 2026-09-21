from periphery import SPI
import time

SPI_DEVICE = "/dev/spidev1.0"
SPI_MODE = 0
SPI_SPEED_HZ = 2_400_000  # ~3 SPI bits per WS2812 bit -> ~1.25us bit period

NUM_LEDS = 8

# WS2812 bit encoding: '1' -> 0b110, '0' -> 0b100 (3 SPI bits per data bit)
BIT_1 = 0b110
BIT_0 = 0b100


def encode_byte(byte_val):
    """Encode one 8-bit color byte into 3 SPI bytes (24 encoded bits)."""
    stream = 0
    for i in range(7, -1, -1):
        bit = (byte_val >> i) & 1
        stream = (stream << 3) | (BIT_1 if bit else BIT_0)
    return [
        (stream >> 16) & 0xFF,
        (stream >> 8) & 0xFF,
        stream & 0xFF,
    ]


def encode_pixels(pixels):
    """pixels: list of (r, g, b) tuples -> WS2812 uses GRB order on the wire."""
    out = []
    for r, g, b in pixels:
        for byte_val in (g, r, b):  # GRB order
            out.extend(encode_byte(byte_val))
    return out


def main():
    spi = SPI(SPI_DEVICE, SPI_MODE, SPI_SPEED_HZ)

    try:
        colors = [(255, 0, 0), (0, 255, 0), (0, 0, 255)]
        pixels = [colors[i % 3] for i in range(NUM_LEDS)]

        PAD = [0x00] * 8  # priming bytes -- settle the bus before real data
        data = PAD + encode_pixels(pixels)
        spi.transfer(data)
        print(f"Sent {NUM_LEDS} pixels ({len(data)} SPI bytes).")

        time.sleep(2)

        off_data = PAD + encode_pixels([(0, 0, 0)] * NUM_LEDS)
        spi.transfer(off_data)
        print("Cleared strip.")

    finally:
        spi.close()


if __name__ == "__main__":
    main()

