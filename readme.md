<img src="./antidote.png" width="200" alt="antidote logo" />

Budget neural amp modelling on a Radxa Cubie A7Z.

---

Very much WIP.

Currently works on MacOS in loading a NAM A2 model via FFI and logging some of its parameters (i.e. sample rate). A passthrough audio callback is set up - this will later be replaced with NAM processing.

```bash
# Clone all dependencies
git submodule update --init --recursive

# Build and run
cargo run --release

...
2026-09-14T17:12:10.860018Z  INFO antidote: Loading NAM A2 model via FFI.
2026-09-14T17:12:10.891034Z  INFO antidote: Loaded NAM A2 model with expected sample rate: 48000
2026-09-14T17:12:10.891057Z  INFO antidote: Starting CPAL.
2026-09-14T17:12:10.923167Z  INFO antidote: Acquired default input device: Device { audio_device_id: 78, name: Ok("MacBook Air Microphone") }
2026-09-14T17:12:10.978795Z  INFO antidote: Playing back audio for 3 seconds.
2026-09-14T17:12:10.991660Z  INFO antidote::modeller: Processed block of size: 512, min: -0.004048766, max: 0.005367287
2026-09-14T17:12:11.002032Z  INFO antidote::modeller: Processed block of size: 512, min: -0.00038602977, max: 0.0011092573
2026-09-14T17:12:11.013661Z  INFO antidote::modeller: Processed block of size: 512, min: -0.00038826824, max: 0.00010735291
...
2026-09-14T17:12:13.962307Z  INFO antidote::modeller: Processed block of size: 512, min: -0.0083647, max: 0.0049650515
2026-09-14T17:12:13.974047Z  INFO antidote::modeller: Processed block of size: 512, min: -0.0036972684, max: 0.005473257
2026-09-14T17:12:13.983811Z  INFO antidote: Stopping audio playback.
2026-09-14T17:12:13.992350Z  INFO antidote: Exiting Antidote.
```

Much still to do.

---

Built on top of:
- [NAM A2 Source Code](https://github.com/sdatkinson/NeuralAmpModelerCore)
- [NAM A2 Reference](https://www.tone3000.com/guides/nam-a2-the-complete-guide)
