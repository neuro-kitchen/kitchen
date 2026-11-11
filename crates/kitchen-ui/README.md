# kitchen-ui

The shared UI of the neuro-kitchen apps, for desktop apps built on
[GPUI](https://www.gpui.rs) through [gpui-kit](https://crates.io/crates/gpui-kit):

- **Design tokens**:
  - spacing on an 8 px grid;
  - a type scale on a 15 px base;
  - radii by size and gaps between panes;
  - plain grey neutrals for light and dark themes, plus status colours.
- **Accents**:
  - each app supplies a default palette (`Palette::LIME`, `Palette::EOSIN`, or its own);
  - the user can pick any colour instead. `Palette::from_rgb` keeps its hue and changes only its
    lightness until text, fills and marks pass WCAG contrast in both themes (in the dark theme it
    also makes the colour less saturated).
- **Widgets**: accent picker, titled section, labelled row (the control drops under its label
  when the panel is narrow), slider, dropdown, stepper, dragged number, colour swatch, empty
  state, icon button.
- **Parameter editor**: edits any op that serialises to a JSON object. Numbers become inputs,
  choices become menus and switches become checkboxes.
- **Icons**:
  - `AppAssets` ships gpui-kit's icons plus the ones an app adds;
  - `assert_icons_bundled` is a test helper that scans an app's sources for every icon name it
    uses (aliases included) and fails if any of them is not shipped.

## Usage

```rust,ignore
use kitchen_ui::theme::{self, Look, Palette, Rgb};
use kitchen_ui::widgets::{row, slider, Section};

gpui_kit::application().with_assets(kitchen_ui::icons::AppAssets::new()).run(|cx| {
    gpui_kit::init(cx);
    // The app's palette, or the colour the user picked (saved in their preferences)
    let accent: Option<Rgb> = None;
    theme::use_look(Look::new(Palette::LIME).with_accent(accent), cx);
    // open windows with gpui_kit::open_window(…)
});

// In a view's render:
Section::new("Display").child(row("Opacity", slider("opacity", 0.8, (0.0, 1.0), |v| format!("{v:.2}"), |v, done, _, cx| {
    /* store v; commit an undo step when `done` */
})));
```

## Gallery

`cargo run -p kitchen-ui --example gallery` (in the repository) opens a window showing every
widget and token:

- the labelled rows in a wide pane and a narrow one;
- a boxed section and icon buttons;
- the parameter editor on a demo band filter;
- an empty state;
- every colour token with its hex value, the type scale and the spacing scale.

Its header switches dark and light, the app palette, the accent (suggested swatches or any
colour) and the text size. A headless test draws the gallery in every look.

## Licence

MIT OR Apache-2.0.
