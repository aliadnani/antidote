# Hardware

This directory contains hardware-related files for the Antidote project, including schematics, PCB layouts, and other stuff.

So far we have:
- `antidote-audio-board/`: KiCAD project for the Antidote audio board.
  - See `antidote-audio-board/schematic.pdf` for a PDF of the schematic.
- `tac5112_i2c_i2s_overlay.dts`: The overlay so we can actually bring up the TAC5112 audio codec on our Radxa Cubie A7Z.