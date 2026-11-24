# kitchen-views

Views the neuro-kitchen apps share, and what views share between them.

| Module | What |
|---|---|
| `container` | the dock every workspace's views live in (moved from kitchen-shell, which re-exports it) |
| `clock` | one time cursor per workspace: every view follows it, any view moves it; playback, frame steps, the `[playback]` keys |
| `sync` | clocks aligned by sync pulses: drift and offset fitted, lost pulses tolerated, the fit's quality kept |
| `video` | frames by index with their timestamps: image sequences, MP4 with H.264 in Rust (feature `h264`), anything else through `ffmpeg` as a process; dropped frames found; a frame cache |
| `video_view` | the video at the clock's moment, an app's overlays over it (outlines, ROIs, markers), frame and times under it |

## One video language for every app

```text
      reference clock (the recording's)
  ┌──────────── Clock ────────────┐
  │ position 1832.415 s           │
  └──┬──────────────┬─────────────┘
     │              │
 traces (dspk)   VideoView ── Alignment (sync pulses: slope, offset, rms)
                    │
                    ├─ FrameTimes.frame_at(t on the video's clock) → frame 54 973
                    └─ overlays painted over it (the app's own)
```

Video is shared because more than one app needs it: video next to ephys (dspk), time-lapse
microscopy and calcium imaging (lucem, dspk), a recording's video in orchestrate's review.
What is drawn over it stays the app's: overlays know their domain, the view does not.

A view moves into this crate when a second app needs it. dspk's traces and lucem's image planes
stay in their apps; they follow the same clock.

## Keys

`[playback]` in `keybindings.toml`, bound in the key context the app names
(`clock::binding`): `space` play/pause, `left` `right` (and `,` `.`) one frame, `shift` + arrow ten
frames, `[` `]` slower and faster.

**Written, not built**: the session that wrote it had no build step. The `mp4`/`openh264` and the
GPUI painting calls follow their documentation; expect small API fixes on the first build.
