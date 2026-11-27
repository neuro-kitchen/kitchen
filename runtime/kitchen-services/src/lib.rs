//! The services every app's frame shows, without the frame: what runs in the background, what
//! happened, who is working, the scripts' environments, and what changed in the project's files.
//! kitchen-shell draws them (status bar, timeline, Settings, the Changes panel); an agent, a test
//! or a server reads the same state here.
//!
//! - [`jobs`]: what runs in the background, with progress and cancel.
//! - [`activity`]: what happened, for the timeline: commands (who, how, with what), time in each
//!   workspace, jobs, what steps produced. In memory, never written.
//! - [`people`]: who is working (the lab's people, ORCID iDs), so records say who did what.
//! - [`env`]: a project's environments for its scripts (which Python, which packages).
//! - [`changes`]: what the Changes panel compares (baselines, normalised JSON, the last commit).
//!
//! Moved out of kitchen-shell in phase 2, step 3c: no drawing here.

pub mod activity;
pub mod changes;
pub mod env;
pub mod jobs;
pub mod people;
