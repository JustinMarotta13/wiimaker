# Host-first oneshot fixture

PCM16 mono 22050 Hz beep (~80 ms). Copy into a game `assets/` folder:

```bash
wiimaker asset import <game> crates/wiimaker-assets/fixtures/beep.wav
WIIMAKER_AUDIO=0 wiimaker asset play <game> --name beep --json   # CI: skipped
wiimaker asset play <game> --name beep                           # hear it on desktop
```

Do not put clips under committed `games/`.
