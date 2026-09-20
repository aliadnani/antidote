# Hardware

This directory contains hardware-related files for the Antidote project, including schematics, PCB layouts, and other stuff.

So far we have:
- `antidote-audio-board/`: KiCAD project for the Antidote audio board.
  - See `antidote-audio-board/schematic.pdf` for a PDF of the schematic.
- `tac5112_i2c_i2s_overlay.dts`: The overlay so we can actually bring up the TAC5112 audio codec on our Radxa Cubie A7Z.

## Audio Bring Up on the Radxa Cubie A7Z

Assuming:
- The TAC5112 (daughter-board) is connected via the GPIO headers
- The TAC5112 drivers are compiled and loaded as a module
  - For kernel: 5.15: https://git.ti.com/cgit/lpaa-android-drivers/tac5x1x-linux-driver/log/?h=tac5x1x_driver_k5.15
- The device overlay is compiled and loaded (e.g. via rsetup)

**To play a test tone:**
```bash
# Left channel (for mono headphones - same signal bridged at T/R pins)
# Not sure if `sudo` is needed to be honest
sudo amixer -c 0 cset name='ASI_RX_CH1_EN Switch' on
sudo amixer -c 0 cset name='OUT1x Source' 'DAC Input'
sudo amixer -c 0 cset name='OUT1x Config' 'Pseudo differential with OUTxM as VCOM'
sudo amixer -c 0 cset name='OUT1x Driver' 'Headphone'

# Right channel for instrument line out
sudo amixer -c 0 cset name='ASI_RX_CH2_EN Switch' on
sudo amixer -c 0 cset name='OUT2x Source' 'DAC Input'
sudo amixer -c 0 cset name='OUT2x Config' 'Mono Single-ended at OUTxP only'
sudo amixer -c 0 cset name='OUT2x Driver' 'Line-out'

speaker-test -D hw:0,0 -c 2 -r 48000 -F S32_LE -t sine -f 440
```

**To record audio (+generate spectrogram and log metrics):**
```
sudo amixer -c 0 cset name='ADC1 Config' 'Single-ended'
sudo amixer -c 0 cset name='ADC1 Common-mode Tolerance' 'AC Coupled with 100mVpp'
sudo amixer -c 0 cset name='ADC1 Full-Scale' '2/10-VRMS'
sudo amixer -c 0 cset name='ASI_TX_CH1 Slot' 'Slot0'
sudo amixer -c 0 cset name='ASI_TX_CH1_EN Capture Switch' on
sudo amixer -c 0 cset name='ADC1 Digital Capture Volume' 161

sudo amixer -c 0 cset name='IN1 Source Mux' 'Analog'
sudo amixer -c 0 cset name='IN2 Source Mux' 'Analog'

arecord -D hw:0,0 -c 2 -f S32_LE -r 48000 -d 5 retest.wav
sox retest.wav -n stat
sox retest.wav -n spectrogram -o retest_spectro.png
```