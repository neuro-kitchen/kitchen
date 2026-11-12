//! What an app can do, declared once: each [`CommandSpec`] has an id, a title, an icon, a line
//! saying what it does, a group, a key, the JSON Schema of its arguments, when it is enabled (and
//! why not), and the function that runs it. The [`Registry`] turns that one list into the menus
//! ([`Registry::menu_button`]), the search rows ([`crate::search`]), the key bindings
//! ([`Registry::bind_keys`]) and an agent's tools (`kitchen-agent`), so none drifts from the
//! others: a command is in the search because it is in the list, greyed with its reason
//! wherever it cannot run.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, v_flex};
use gpui_kit::{Action, App, Context, IntoElement, KeyBinding, ParentElement as _, SharedString, Styled as _, Window, div};
use kitchen_ui::theme::color;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Runs a command with its arguments (`null` when it takes none); its result is JSON (`null`
/// for a plain action), and an error says why it did not run.
pub type RunFn = Rc<dyn Fn(Value, &mut Window, &mut App) -> Result<Value, String>>;
/// Whether a command can run now; `Err` says why not (shown greyed in menus and search).
pub type EnabledFn = Rc<dyn Fn(&App) -> Result<(), String>>;
/// Whether a toggle command is on (a check mark in menus).
pub type CheckedFn = Rc<dyn Fn(&App) -> bool>;
/// Makes a menu's entries when it opens, from what the app holds then (its images, recent
/// files, the models that fit): see [`Registry::provide`] and [`MenuItem::Provided`].
pub type MenuProvider = Rc<dyn Fn(&App) -> Vec<MenuItem>>;

/// The action every command key and menu item dispatches: run the command `id` (no arguments).
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = kitchen, no_json)]
pub struct RunCommand {
    /// The command's id.
    pub id: SharedString,
}

/// One thing an app can do.
#[derive(Clone)]
pub struct CommandSpec {
    /// Unique, dotted by area: `file.open`, `hist.find_cells`, `pipeline.add_op`.
    pub id: SharedString,
    /// What menus and search show: "Open image…".
    pub title: SharedString,
    /// Its icon, in menus and search.
    pub icon: IconName,
    /// One line saying what it does (search, tooltips, the agent's tool description).
    pub hint: SharedString,
    /// The group it is listed under in search ("File", "Process").
    pub group: SharedString,
    /// Its key in GPUI's notation (`secondary-o`: Cmd on macOS, Ctrl elsewhere), if any.
    pub key: Option<SharedString>,
    /// The JSON Schema of its arguments, when an agent can pass some ([`CommandSpec::with_args`]).
    pub args: Option<Value>,
    /// Whether it only reads (an agent may run it without asking).
    pub read_only: bool,
    /// Whether it can run now, and why not.
    pub enabled: EnabledFn,
    /// For a toggle: whether it is on.
    pub checked: Option<CheckedFn>,
    /// What it does.
    pub run: RunFn,
}

impl CommandSpec {
    /// A plain command: no arguments, no result.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>, icon: IconName, run: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self::raw(id, title, icon, None, Rc::new(move |_, window, cx| {
            run(window, cx);
            Ok(Value::Null)
        }))
    }

    /// A command taking arguments `A` (their JSON Schema comes from `A`, and a call whose
    /// arguments do not read as `A` is refused with the reason) and returning JSON.
    pub fn with_args<A: DeserializeOwned + JsonSchema + 'static>(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        icon: IconName,
        run: impl Fn(A, &mut Window, &mut App) -> Result<Value, String> + 'static,
    ) -> Self {
        let schema = serde_json::to_value(schemars::schema_for!(A)).ok();
        Self::raw(id, title, icon, schema, Rc::new(move |args, window, cx| {
            // A command whose arguments are all optional also runs with none
            let args = if args.is_null() { Value::Object(Default::default()) } else { args };
            let a: A = serde_json::from_value(args).map_err(|e| format!("arguments: {e}"))?;
            run(a, window, cx)
        }))
    }

    /// A command from its parts: `args` is its JSON Schema (or `None`), `run` gets the raw JSON.
    pub fn raw(id: impl Into<SharedString>, title: impl Into<SharedString>, icon: IconName, args: Option<Value>, run: RunFn) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon,
            hint: SharedString::default(),
            group: "Commands".into(),
            key: None,
            args,
            read_only: false,
            enabled: Rc::new(|_| Ok(())),
            checked: None,
            run,
        }
    }

    /// Sets the line saying what it does.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Sets the group it is listed under.
    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = group.into();
        self
    }

    /// Sets its key (`secondary-o`, `f3`, `secondary-shift-s`).
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Marks it as only reading (an agent may run it without asking).
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Sets when it can run (`Err(why)` otherwise).
    pub fn enabled(mut self, enabled: impl Fn(&App) -> Result<(), String> + 'static) -> Self {
        self.enabled = Rc::new(enabled);
        self
    }

    /// Makes it a toggle, on when `checked` says so.
    pub fn checked(mut self, checked: impl Fn(&App) -> bool + 'static) -> Self {
        self.checked = Some(Rc::new(checked));
        self
    }

    /// Its key as people read it ("Ctrl+Shift+S", "⌘⇧S" on macOS).
    pub fn key_label(&self) -> Option<String> {
        self.key.as_deref().map(key_label)
    }
}

/// An entry of a menu.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuItem {
    /// A registered command, by id.
    Command(SharedString),
    /// A registered command run with arguments, under its own title: one entry per image, per
    /// recent file, per model. Greyed with the command's reason when it cannot run.
    Run {
        /// What the entry says.
        title: SharedString,
        /// Its icon (the command's when `None`).
        icon: Option<IconName>,
        /// The command's id.
        command: SharedString,
        /// Its arguments.
        args: Value,
        /// A check mark (the entry is the current choice).
        checked: bool,
    },
    /// The entries the provider `id` makes when the menu opens ([`Registry::provide`]).
    Provided(SharedString),
    /// A line between groups.
    Separator,
    /// A heading (not clickable).
    Label(SharedString),
    /// A menu inside the menu.
    Submenu {
        /// What the entry says.
        title: SharedString,
        /// Its icon.
        icon: Option<IconName>,
        /// What it holds.
        items: Vec<MenuItem>,
    },
}

impl MenuItem {
    /// The command `id`.
    pub fn command(id: impl Into<SharedString>) -> Self {
        MenuItem::Command(id.into())
    }

    /// A submenu `title` holding `items`.
    pub fn submenu(title: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        MenuItem::Submenu { title: title.into(), icon: None, items }
    }

    /// A submenu `title` with `icon`, holding `items`.
    pub fn submenu_with_icon(title: impl Into<SharedString>, icon: IconName, items: Vec<MenuItem>) -> Self {
        MenuItem::Submenu { title: title.into(), icon: Some(icon), items }
    }

    /// The command `command` run with `args`, titled `title`.
    pub fn run(title: impl Into<SharedString>, command: impl Into<SharedString>, args: Value) -> Self {
        MenuItem::Run { title: title.into(), icon: None, command: command.into(), args, checked: false }
    }

    /// The entries of the provider `id`.
    pub fn provided(id: impl Into<SharedString>) -> Self {
        MenuItem::Provided(id.into())
    }

    /// A heading.
    pub fn label(text: impl Into<SharedString>) -> Self {
        MenuItem::Label(text.into())
    }

    /// Sets the icon of a [`MenuItem::Run`] or a submenu (other entries are unchanged).
    pub fn icon(mut self, with: IconName) -> Self {
        match &mut self {
            MenuItem::Run { icon, .. } | MenuItem::Submenu { icon, .. } => *icon = Some(with),
            _ => {}
        }
        self
    }

    /// Sets the check mark of a [`MenuItem::Run`] (other entries are unchanged).
    pub fn checked(mut self, on: bool) -> Self {
        if let MenuItem::Run { checked, .. } = &mut self {
            *checked = on;
        }
        self
    }
}

/// A menu of a menu row or title bar: its title and its entries.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuSpec {
    /// What the button says ("File", "Process").
    pub title: SharedString,
    /// The tooltip of its button.
    pub hint: SharedString,
    /// Its entries.
    pub items: Vec<MenuItem>,
}

impl MenuSpec {
    /// A menu `title` with `items`.
    pub fn new(title: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        Self { title: title.into(), hint: SharedString::default(), items }
    }

    /// Sets the tooltip of its button.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = hint.into();
        self
    }

    /// The ids of the commands it holds, submenus included, in order.
    pub fn command_ids(&self) -> Vec<SharedString> {
        fn walk(items: &[MenuItem], out: &mut Vec<SharedString>) {
            for i in items {
                match i {
                    MenuItem::Command(id) | MenuItem::Run { command: id, .. } => out.push(id.clone()),
                    MenuItem::Submenu { items, .. } => walk(items, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.items, &mut out);
        out
    }
}

/// Every command of an app, shared by its menus, search, keys and agent (a cheap handle: clones
/// share the same list).
#[derive(Clone, Default)]
pub struct Registry {
    commands: Rc<RefCell<Vec<CommandSpec>>>,
    providers: Rc<RefCell<Vec<(SharedString, MenuProvider)>>>,
}

impl Registry {
    /// A registry holding `commands`.
    pub fn new(commands: Vec<CommandSpec>) -> Self {
        let r = Self::default();
        r.extend(commands);
        r
    }

    /// Adds a command, replacing one with the same id.
    pub fn add(&self, command: CommandSpec) {
        let mut list = self.commands.borrow_mut();
        match list.iter_mut().find(|c| c.id == command.id) {
            Some(c) => *c = command,
            None => list.push(command),
        }
    }

    /// Adds several commands ([`Registry::add`] each).
    pub fn extend(&self, commands: impl IntoIterator<Item = CommandSpec>) {
        for c in commands {
            self.add(c);
        }
    }

    /// Removes the command `id`; whether it was there.
    pub fn remove(&self, id: &str) -> bool {
        let mut list = self.commands.borrow_mut();
        let n = list.len();
        list.retain(|c| c.id != id);
        list.len() != n
    }

    /// The command `id`.
    pub fn get(&self, id: &str) -> Option<CommandSpec> {
        self.commands.borrow().iter().find(|c| c.id == id).cloned()
    }

    /// Every command, in the order added.
    pub fn all(&self) -> Vec<CommandSpec> {
        self.commands.borrow().clone()
    }

    /// How many commands it holds.
    pub fn len(&self) -> usize {
        self.commands.borrow().len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.commands.borrow().is_empty()
    }

    /// Registers the menu provider `id` (replacing one with the same id): a
    /// [`MenuItem::Provided`] naming it shows the entries it makes when the menu opens.
    pub fn provide(&self, id: impl Into<SharedString>, provider: impl Fn(&App) -> Vec<MenuItem> + 'static) {
        let id = id.into();
        let mut list = self.providers.borrow_mut();
        list.retain(|(p, _)| *p != id);
        list.push((id, Rc::new(provider)));
    }

    /// The entries the provider `id` makes now (none when there is no such provider).
    pub fn provided(&self, id: &str, cx: &App) -> Vec<MenuItem> {
        let provider = self.providers.borrow().iter().find(|(p, _)| p == id).map(|(_, f)| f.clone());
        provider.map(|f| f(cx)).unwrap_or_default()
    }

    /// Whether `id` can run now (`Err` says why not, or that there is no such command).
    pub fn check(&self, id: &str, cx: &App) -> Result<(), String> {
        let c = self.get(id).ok_or_else(|| format!("no command {id:?}"))?;
        (c.enabled)(cx)
    }

    /// Runs `id` with `args` (`null` for none), when it can run now, as the person did.
    pub fn run(&self, id: &str, args: Value, window: &mut Window, cx: &mut App) -> Result<Value, String> {
        self.run_as(crate::activity::Actor::Person, "app", id, args, window, cx)
    }

    /// Runs `id` with `args` for `actor`, started from `via` (menu, key, search, title bar, agent),
    /// and records it in the activity log ([`crate::activity`]), refused or not.
    pub fn run_as(&self, actor: crate::activity::Actor, via: &str, id: &str, args: Value, window: &mut Window, cx: &mut App) -> Result<Value, String> {
        let Some(c) = self.get(id) else { return Err(format!("no command {id:?}")) };
        let result = (c.enabled)(cx).and_then(|_| (c.run)(args.clone(), window, cx));
        crate::activity::command(actor, id, &c.title, &c.group, via, &args, &result);
        result
    }

    /// Binds every command's key to [`RunCommand`] (globally; `context` limits them to a part of
    /// the window, as GPUI's key contexts do). A key that does not parse is skipped and named
    /// in the result.
    pub fn bind_keys(&self, context: Option<&str>, cx: &mut App) -> Vec<String> {
        let mut bindings = Vec::new();
        let mut bad = Vec::new();
        let context = match context.map(gpui_kit::KeyBindingContextPredicate::parse).transpose() {
            Ok(c) => c.map(Rc::new),
            Err(e) => return vec![format!("context {context:?}: {e}")],
        };
        for c in self.all() {
            let Some(key) = c.key.clone() else { continue };
            match KeyBinding::load(&key, Box::new(RunCommand { id: c.id.clone() }), context.clone(), false, None, cx) {
                Ok(b) => bindings.push(b),
                Err(e) => bad.push(format!("{}: {key}: {e:?}", c.id)),
            }
        }
        cx.bind_keys(bindings);
        bad
    }

    /// A ghost button titled as `menu`, opening its entries: each command with its icon, key
    /// and check mark; a command that cannot run now is greyed with its reason under it.
    pub fn menu_button(&self, id: impl Into<SharedString>, menu: &MenuSpec) -> gpui_kit::AnyElement {
        let registry = self.clone();
        let items = Rc::new(menu.items.clone());
        let mut button = Button::new(id.into()).ghost().small().label(menu.title.clone());
        if !menu.hint.is_empty() {
            button = button.tooltip(menu.hint.clone());
        }
        // Long menus (an entry per image or channel) scroll
        button.dropdown_menu(move |m, window, cx| registry.fill_menu(m.min_w(gpui_kit::px(240.)).scrollable(true).max_h(gpui_kit::px(560.)), &items, window, cx)).into_any_element()
    }

    /// Adds `items` to `menu` (as [`Registry::menu_button`] does).
    pub fn fill_menu(&self, mut menu: PopupMenu, items: &[MenuItem], window: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
        for item in items {
            menu = match item {
                MenuItem::Separator => menu.separator(),
                MenuItem::Label(text) => menu.label(text.clone()),
                MenuItem::Submenu { title, icon, items } => {
                    let (registry, items) = (self.clone(), items.clone());
                    menu.submenu_with_icon(icon.map(Icon::new), title.clone(), window, cx, move |m, window, cx| registry.fill_menu(m, &items, window, cx))
                }
                MenuItem::Command(id) => match self.get(id) {
                    Some(c) => menu.item(self.menu_item(&c, cx)),
                    // A menu naming a command the app did not register shows it, greyed
                    None => menu.item(PopupMenuItem::new(format!("{id} (not registered)")).disabled(true)),
                },
                MenuItem::Run { title, icon, command, args, checked } => match self.get(command) {
                    Some(c) => menu.item(self.run_item(&c, title, icon.unwrap_or(c.icon), args, *checked, cx)),
                    None => menu.item(PopupMenuItem::new(format!("{title} ({command} not registered)")).disabled(true)),
                },
                MenuItem::Provided(id) => {
                    let items = self.provided(id, cx);
                    self.fill_menu(menu, &items, window, cx)
                }
            };
        }
        menu
    }

    /// An entry running `c` with `args` under `title`.
    fn run_item(&self, c: &CommandSpec, title: &SharedString, icon: IconName, args: &Value, checked: bool, cx: &App) -> PopupMenuItem {
        match (c.enabled)(cx) {
            Ok(()) => {
                let (registry, id, args) = (self.clone(), c.id.clone(), args.clone());
                PopupMenuItem::new(title.clone()).icon(Icon::new(icon)).checked(checked).on_click(move |_, window, cx| {
                    if let Err(e) = registry.run_as(crate::activity::Actor::Person, "menu", &id, args.clone(), window, cx) {
                        eprintln!("{id}: {e}");
                    }
                })
            }
            Err(why) => {
                let title = title.clone();
                PopupMenuItem::element(move |_, _| {
                    v_flex().child(div().text_color(color::text_disabled()).child(title.clone())).child(div().text_xs().text_color(color::text_muted()).child(why.clone()))
                })
                .icon(Icon::new(icon))
                .disabled(true)
            }
        }
    }

    fn menu_item(&self, c: &CommandSpec, cx: &App) -> PopupMenuItem {
        let action = Box::new(RunCommand { id: c.id.clone() });
        match (c.enabled)(cx) {
            Ok(()) => {
                let (registry, id) = (self.clone(), c.id.clone());
                let checked = c.checked.as_ref().is_some_and(|f| f(cx));
                // The action shows the key; the handler runs the command (it wins over the action)
                PopupMenuItem::new(c.title.clone()).icon(Icon::new(c.icon)).checked(checked).action(action).on_click(move |_, window, cx| {
                    if let Err(e) = registry.run_as(crate::activity::Actor::Person, "menu", &id, Value::Null, window, cx) {
                        eprintln!("{id}: {e}");
                    }
                })
            }
            Err(why) => {
                let title = c.title.clone();
                PopupMenuItem::element(move |_, _| {
                    v_flex().child(div().text_color(color::text_disabled()).child(title.clone())).child(div().text_xs().text_color(color::text_muted()).child(why.clone()))
                })
                .icon(Icon::new(c.icon))
                .disabled(true)
            }
        }
    }
}

/// A key in GPUI's notation as people read it: `secondary-shift-s` is "Ctrl+Shift+S" (and
/// "⌘⇧S" on macOS), `f3` is "F3".
pub fn key_label(key: &str) -> String {
    // A sequence of keystrokes is separated by spaces
    key.split_whitespace().map(stroke_label).collect::<Vec<_>>().join(" ")
}

fn stroke_label(stroke: &str) -> String {
    let mac = cfg!(target_os = "macos");
    let parts: Vec<&str> = stroke.split('-').collect();
    // `ctrl--` (the minus key) splits into an empty last part
    let (mods, key) = match parts.as_slice() {
        [mods @ .., "", ""] => (mods, "-"),
        [mods @ .., key] => (mods, *key),
        [] => (&[][..], ""),
    };
    let mut out: Vec<String> = mods
        .iter()
        .map(|m| match (*m, mac) {
            ("secondary" | "cmd", true) => "⌘".into(),
            ("secondary" | "ctrl", false) => "Ctrl".into(),
            ("ctrl", true) => "⌃".into(),
            ("cmd", false) => "Super".into(),
            ("shift", true) => "⇧".into(),
            ("shift", false) => "Shift".into(),
            ("alt", true) => "⌥".into(),
            ("alt", false) => "Alt".into(),
            (other, _) => other.to_string(),
        })
        .collect();
    let key = match key {
        k if k.len() == 1 => k.to_uppercase(),
        k if k.starts_with('f') && k[1..].parse::<u8>().is_ok() => k.to_uppercase(),
        "escape" => "Esc".into(),
        k => {
            let mut c = k.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    };
    out.push(key);
    if mac { out.concat() } else { out.join("+") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_read_as_people_write_them() {
        if cfg!(target_os = "macos") {
            assert_eq!(key_label("secondary-shift-s"), "⌘⇧S");
        } else {
            assert_eq!(key_label("secondary-shift-s"), "Ctrl+Shift+S");
            assert_eq!(key_label("secondary-,"), "Ctrl+,");
            assert_eq!(key_label("ctrl--"), "Ctrl+-");
            assert_eq!(key_label("secondary-k secondary-s"), "Ctrl+K Ctrl+S");
        }
        assert_eq!(key_label("f3"), "F3");
        assert_eq!(key_label("escape"), "Esc");
    }

    #[test]
    fn menus_list_their_commands() {
        let m = MenuSpec::new("File", vec![MenuItem::command("file.open"), MenuItem::Separator, MenuItem::submenu("Recent", vec![MenuItem::command("file.recent.1")])]);
        assert_eq!(m.command_ids(), vec![SharedString::from("file.open"), SharedString::from("file.recent.1")]);
    }

    #[test]
    fn a_registry_replaces_a_command_with_the_same_id() {
        let r = Registry::new(vec![CommandSpec::new("a", "A", IconName::Plus, |_, _| {}), CommandSpec::new("b", "B", IconName::Plus, |_, _| {})]);
        r.add(CommandSpec::new("a", "A again", IconName::Plus, |_, _| {}));
        assert_eq!(r.len(), 2);
        assert_eq!(r.get("a").unwrap().title.as_ref(), "A again");
        assert!(r.remove("b") && !r.remove("b"));
    }

    #[derive(serde::Deserialize, JsonSchema)]
    #[allow(dead_code)]
    struct Threshold {
        /// The level, 0–1.
        level: f64,
    }

    #[test]
    fn typed_commands_publish_their_schema() {
        let c = CommandSpec::with_args::<Threshold>("t", "Threshold", IconName::Plus, |a, _, _| Ok(serde_json::json!(a.level)));
        let schema = c.args.unwrap();
        assert_eq!(schema["properties"]["level"]["type"], "number");
        assert_eq!(schema["required"][0], "level");
    }
}
