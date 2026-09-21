# Test Scripts

This directory contains test scripts to test that the peripherals are indeed wired up correctly. Run these on the SBC.


Install the following dependencies first:
```bash
sudo pip3 install git+https://github.com/vsergeev/python-periphery.git
sudo pip3 install luma.oled
```

Ensure the following overlays are loaded in `rsetup`:
- `spidev1`
- `TWI7`

```bash
# Test OLED (over I2C) + FootSwitch
sudo python3 ./test_oled_button.py

# Test WS2812B (over SPI)
sudo python3 ./test_spi_led.py
```
