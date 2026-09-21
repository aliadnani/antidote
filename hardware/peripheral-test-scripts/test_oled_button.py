from periphery import GPIO
from luma.core.interface.serial import i2c
from luma.core.render import canvas
from luma.oled.device import sh1106
import time

I2C_PORT = 7
I2C_ADDRESS = 0x3C

GPIO_CHIP = "/dev/gpiochip1"
GPIO_LINE = 5


def draw_screen(device, pressed):
    if pressed:
        bg, fg = "white", "black"
        label = "PRESSED"
    else:
        bg, fg = "black", "white"
        label = "RELEASED"

    with canvas(device) as draw:
        draw.rectangle(device.bounding_box, outline=bg, fill=bg)
        draw.text((30, 25), label, fill=fg)


def main():
    serial = i2c(port=I2C_PORT, address=I2C_ADDRESS)
    device = sh1106(serial, width=128, height=64)

    # bias="pull_up" replicates `gpiomon --bias=pull-up`; active-low wiring
    # means a press pulls the line to 0.
    button = GPIO(GPIO_CHIP, GPIO_LINE, "in", bias="pull_up")

    print(f"Watching {GPIO_CHIP} line {GPIO_LINE}. Ctrl+C to exit.")

    last_state = None
    try:
        while True:
            pressed = not button.read()
            if pressed != last_state:
                draw_screen(device, pressed)
                last_state = pressed
            time.sleep(0.02)  # simple 20ms poll/debounce interval
    except KeyboardInterrupt:
        print("\nExiting.")
    finally:
        button.close()
        device.clear()


if __name__ == "__main__":
    main()

