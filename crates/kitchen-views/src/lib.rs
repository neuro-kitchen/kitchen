//! Views the neuro-kitchen apps share, and what views share between them.
//!
//! - [`container`]: the dock every workspace's views live in (tabs dragged into splits, the
//!   "Add view or container" dialog). Moved from kitchen-shell, which re-exports it.
//! - [`clock`]: one time cursor per workspace that every view follows and any view moves;
//!   playback, frame steps, and the playback keys (`[playback]`).
//! - [`pose`]: keypoints over video (markerless pose estimation's output): skeleton, points
//!   with scores, the long CSV every tool writes, painted over the video.
//! - [`sync`]: clocks aligned by sync pulses (drift and offset fitted, lost pulses tolerated).
//! - [`video`]: video frames by index with their timestamps (image sequences, MP4 with H.264 in
//!   Rust, anything else through ffmpeg as a process), dropped frames found, a frame cache.
//! - [`video_view`]: the video at the clock's moment with keypoints over it, frame and times under
//!   it, the playback keys over it.
//!
//! A view moves here when a second app needs it, not before: dspk's traces and lucem's image
//! planes are tuned to their data and stay in their apps, behind the same clock.

// GPUI's derive macros name `gpui`; through the kit, that is gpui-kit
extern crate gpui_kit as gpui;

pub mod clock;
pub mod container;
pub mod pose;
pub mod sync;
pub mod video;
pub mod video_view;

pub use clock::{Clock, ClockEvent};
pub use container::{AddViewDialog, Arrangement, DialogEvent, ViewOption};
pub use pose::{Pose, PoseStyle, Skeleton};
pub use sync::Alignment;
pub use video_view::VideoView;
