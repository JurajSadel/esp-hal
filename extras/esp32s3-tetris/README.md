# ESP32-S3-BOX Tetris

This is a standalone ESP-IDF + LVGL Tetris game for **ESP32-S3-BOX** (`espressif/esp-box` BSP).

## Features

- 10x20 Tetris board
- 7 tetrominoes (I, O, T, S, Z, J, L)
- Rotation with simple wall-kick offsets
- Soft drop, hard drop, line clear scoring, level-based speedup
- On-screen touch controls (LEFT, RIGHT, ROT, DOWN, DROP, RST)
- Next-piece preview and score/lines/level HUD

## Build and flash

1. Install ESP-IDF (v5.1+).
2. Open this project directory:

   ```bash
   cd extras/esp32s3-tetris
   ```

3. Set target and build:

   ```bash
   idf.py set-target esp32s3
   idf.py build
   ```

4. Flash and monitor:

   ```bash
   idf.py -p /dev/ttyUSB0 flash monitor
   ```

## Controls

- **LEFT / RIGHT**: Move piece horizontally
- **ROT**: Rotate clockwise
- **DOWN**: Soft drop by one row
- **DROP**: Hard drop to lock instantly
- **RST**: Restart game
