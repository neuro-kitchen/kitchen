//! Small pieces both apps use: an accent picker, a titled section, a labelled row, a slider, a dropdown of fixed
//! options, a stepper and a dragged number, a colour swatch, a centred empty state, an icon
//! button. Plain data in, no store; colours from the current look ([`crate::theme::color`]).

mod accent_picker;
mod button;
mod empty;
mod menu_select;
mod row;
mod section;
mod slider;
mod stepper;
mod swatch;

pub use accent_picker::{AccentChanged, AccentPicker};
pub use button::icon_button;
pub use empty::EmptyState;
pub use menu_select::MenuSelect;
pub use row::row;
pub use section::Section;
pub use slider::{OnSlide, slider};
pub use stepper::{OnStep, SCRUB_PX, scrub_label, stepper};
pub use swatch::swatch;
