//! Small pieces both apps use: an accent picker, a titled section, a labelled row, a slider, a dropdown of fixed
//! options, a stepper and a dragged number, a colour swatch, a centred empty state, an icon
//! button; and the text views: code ([`CodeView`]), diffs ([`DiffView`]) and light markdown
//! ([`TextView`]). Plain data in, no store; colours from the current look
//! ([`crate::theme::color`]).

mod accent_picker;
mod button;
mod code_view;
mod diff_view;
mod empty;
mod menu_select;
mod row;
mod section;
mod slider;
mod stepper;
mod swatch;
mod text_view;

pub use accent_picker::{AccentChanged, AccentPicker};
pub use button::icon_button;
pub use code_view::{CodeChanged, CodeLanguage, CodeView};
pub use diff_view::{DiffView, HunkDecision};
pub use empty::EmptyState;
pub use menu_select::MenuSelect;
pub use row::row;
pub use section::Section;
pub use slider::{OnSlide, slider};
pub use stepper::{OnStep, SCRUB_PX, scrub_label, stepper};
pub use swatch::swatch;
pub use text_view::TextView;
