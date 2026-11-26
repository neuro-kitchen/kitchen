# 0010. Apps are MVVM; views never call the runtime

**Context.** The apps already follow MVVM. Generated interfaces and agents must reach everything a
window can do, and screens must be testable without a window.

**Decision.** Model: the app's domain plus the runtime, through the core's client. ViewModel: one
per screen, holding its state, its commands and what it offers as typed inputs; it alone calls the
runtime. View: drawing only, with the core's interface layer.

**Consequences.** What a screen offers (inputs, commands) is the same thing the runtime API exposes,
so a generated interface or an agent is never second-class. View-models are tested headless. Views
that call the runtime directly are a review failure.
