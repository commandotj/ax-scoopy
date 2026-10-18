//! The settings pages beyond General/Recordings/Screenshots -- Shortcuts,
//! CLI, Transcription and Experimental -- kept out of `settings_window.rs` so
//! the shell stays readable. Every renderer here is an `impl SettingsWindow`
//! extension.
//!
//! Transcribed from the routes under
//! `apps/desktop/src/routes/(window-chrome)/settings/`, with every store key
//! and HTTP endpoint spelled exactly the way the Tauri app spells it so the
//! two apps keep reading each other's state. Persistence goes through
//! [`crate::store`].

use std::{str::FromStr as _, time::Duration};

use gpui::{
    AppContext as _, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    MouseButton, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, rgb, svg,
};
use serde_json::{Map, Value};

use crate::{
    settings_window::{Page, SettingsWindow},
    store::{self, GENERAL_SETTINGS, Hotkey},
    theme::Theme,
    ui,
};

/// `font-mono` in the TSX resolves to the platform mono stack; the closest
/// single face per OS, since no mono font is bundled.
const MONO_FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "monospace"
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// `saveState` in transcription.tsx.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HintSave {
    Idle,
    Saving,
    Saved,
}

struct ListeningHotkey {
    action: &'static str,
    modifiers: gpui::Modifiers,
    error: Option<&'static str>,
}

struct SavedShortcut {
    action: &'static str,
    moved_from: Option<&'static str>,
}

/// Everything the pages here own. Lives on [`SettingsWindow`] as one field so
/// the shell's struct stays readable.
pub(crate) struct PagesState {
    // Shortcuts (hotkeys.tsx)
    hotkeys: Map<String, Value>,
    listening: Option<ListeningHotkey>,
    shortcut_saved: Option<SavedShortcut>,
    shortcut_saved_reset: Option<gpui::Task<()>>,
    shortcut_blocked: Option<&'static str>,

    // CLI (cli.tsx). `None` while the status fetch is in flight.
    cli_status: Option<Result<cli_install::CliInstallStatus, String>>,
    cli_installing: bool,
    cli_uninstalling: bool,
    /// Stand-in for cli.tsx's `toast.error` -- there is no toast layer here.
    cli_error: Option<String>,
    cli_copied: bool,
    cli_copied_reset: Option<gpui::Task<()>>,

    // Transcription (transcription.tsx)
    hints: Vec<String>,
    hint_input: Entity<ui::TextInputState>,
    hint_draft: String,
    hint_save: HintSave,
    hint_save_task: Option<gpui::Task<()>>,

    // refetch keeps showing the loaded list, the way the cached query does.

    // Experimental (experimental.tsx). The Developer section's hand-back takes the
    // whole settings window over, so its clock and the ticker that repaints the
    // sequence live here and the overlay renders from the window root.
    switch_back: Option<SwitchBack>,
    switch_back_ticker: Option<gpui::Task<()>>,

    _subscriptions: Vec<gpui::Subscription>,
}

impl PagesState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<SettingsWindow>) -> Self {
        let hint_input = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_placeholder("Add a term");
            input
        });
        let mut subscriptions = vec![cx.subscribe(&hint_input, |this, input, event, cx| {
            if *event == ui::TextInputEvent::Changed {
                this.pages.hint_draft = input.read(cx).text().to_string();
                cx.notify();
            }
        })];
        // Keys stop arriving once the window loses focus, so a recording row
        // would otherwise keep every global shortcut paused.
        subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.shortcuts_cancel(cx);
            }
        }));

        Self {
            hotkeys: store::hotkeys_raw(),
            listening: None,
            shortcut_saved: None,
            shortcut_saved_reset: None,
            shortcut_blocked: None,
            cli_status: None,
            cli_installing: false,
            cli_uninstalling: false,
            cli_error: None,
            cli_copied: false,
            cli_copied_reset: None,
            hints: store::transcription_hints(),
            hint_input,
            hint_draft: String::new(),
            hint_save: HintSave::Idle,
            hint_save_task: None,
            switch_back: None,
            switch_back_ticker: None,
            _subscriptions: subscriptions,
        }
    }
}

// ---------------------------------------------------------------------------
// Shell hooks
// ---------------------------------------------------------------------------

impl SettingsWindow {
    /// Per-page fetch/reset on navigation -- what each route's mount effects
    /// do. Called from [`SettingsWindow::page_shown`].
    pub(crate) fn pages_shown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.page {
            Page::Shortcuts => {
                self.pages.hotkeys = store::hotkeys_raw();
                self.pages.shortcut_saved = None;
                self.pages.shortcut_saved_reset = None;
                self.pages.shortcut_blocked = None;
            }
            Page::Cli => self.cli_refresh(window, cx),
            Page::Transcription => {
                self.pages.hints = store::transcription_hints();
                self.pages.hint_draft.clear();
                self.pages.hint_save = HintSave::Idle;
                self.pages.hint_save_task = None;
                let input = self.pages.hint_input.clone();
                input.update(cx, |input, cx| input.set_text("", cx));
            }
            _ => {}
        }
    }

    /// A page-owned single-line input on the settings surface.
    fn pages_input(
        &self,
        id: impl Into<gpui::ElementId>,
        input: &Entity<ui::TextInputState>,
    ) -> ui::TextInput {
        ui::TextInput::settings(&self.theme, id, input)
    }
}

// ---------------------------------------------------------------------------
// Shortcuts
// ---------------------------------------------------------------------------

const SCREENSHOT_SHORTCUTS: [&str; 3] = ["screenshotArea", "screenshotWindow", "screenshotDisplay"];

const RECORDING_SHORTCUTS: [&str; 7] = [
    "startStudioRecording",
    "startInstantRecording",
    "openRecordingPickerArea",
    "openRecordingPickerWindow",
    "openRecordingPickerDisplay",
    "openRecordingPicker",
    "cycleRecordingMode",
];

const WHILE_RECORDING_SHORTCUTS: [&str; 3] =
    ["stopRecording", "togglePauseRecording", "restartRecording"];

const SHORTCUT_SAVED_NOTICE: std::time::Duration = std::time::Duration::from_secs(6);

#[derive(Clone, Copy)]
struct OutcomeOption {
    icon: &'static str,
    label: &'static str,
    hint: &'static str,
}

const SCREENSHOT_OUTCOMES: [(store::PostScreenshotBehaviour, OutcomeOption); 4] = [
    (
        store::PostScreenshotBehaviour::OpenEditor,
        OutcomeOption {
            icon: "icons/edit.svg",
            label: "Open editor",
            hint: "Annotate first",
        },
    ),
    (
        store::PostScreenshotBehaviour::Copy,
        OutcomeOption {
            icon: "icons/copy.svg",
            label: "Copy",
            hint: "Paste anywhere",
        },
    ),
    (
        store::PostScreenshotBehaviour::CopyAndOpenEditor,
        OutcomeOption {
            icon: "icons/layers.svg",
            label: "Copy and edit",
            hint: "Both at once",
        },
    ),
    (
        store::PostScreenshotBehaviour::Save,
        OutcomeOption {
            icon: "icons/folder-down.svg",
            label: "Do nothing",
            hint: "Saved to library",
        },
    ),
];

const STUDIO_OUTCOMES: [(store::PostStudioBehaviour, OutcomeOption); 3] = [
    (
        store::PostStudioBehaviour::OpenEditor,
        OutcomeOption {
            icon: "icons/edit.svg",
            label: "Open editor",
            hint: "Edit right away",
        },
    ),
    (
        store::PostStudioBehaviour::ShowOverlay,
        OutcomeOption {
            icon: "icons/app-window.svg",
            label: "Show Scoopy",
            hint: "Back to the recorder",
        },
    ),
    (
        store::PostStudioBehaviour::DoNothing,
        OutcomeOption {
            icon: "icons/folder-down.svg",
            label: "Do nothing",
            hint: "Saved to library",
        },
    ),
];

/// `disableAutoOpenLinks`, phrased as what an Instant recording does.
const INSTANT_OUTCOMES: [(bool, OutcomeOption); 2] = [
    (
        false,
        OutcomeOption {
            icon: "icons/globe.svg",
            label: "Open link",
            hint: "In your browser",
        },
    ),
    (
        true,
        OutcomeOption {
            icon: "icons/link.svg",
            label: "Copy link only",
            hint: "Nothing opens",
        },
    ),
];

fn shortcut_label(action: &str) -> &'static str {
    match action {
        "screenshotArea" => "Screenshot an area",
        "screenshotWindow" => "Screenshot a window",
        "screenshotDisplay" => "Screenshot the display",
        "startStudioRecording" => "Start Studio recording",
        "startInstantRecording" => "Start Instant recording",
        "openRecordingPickerArea" => "Record an area",
        "openRecordingPickerWindow" => "Record a window",
        "openRecordingPickerDisplay" => "Record a display",
        "openRecordingPicker" => "Open recording picker",
        "cycleRecordingMode" => "Cycle recording mode",
        "stopRecording" => "Stop recording",
        "togglePauseRecording" => "Pause or resume",
        "restartRecording" => "Restart recording",
        _ => "Shortcut",
    }
}

fn screenshot_outcome(behaviour: store::PostScreenshotBehaviour) -> &'static str {
    match behaviour {
        store::PostScreenshotBehaviour::OpenEditor => "open it in the editor",
        store::PostScreenshotBehaviour::Copy => "copy it to your clipboard",
        store::PostScreenshotBehaviour::CopyAndOpenEditor => "copy it and open the editor",
        store::PostScreenshotBehaviour::Save => "save it to your library without opening anything",
    }
}

fn picker_outcome(mode: crate::main_window::Mode) -> &'static str {
    match mode {
        crate::main_window::Mode::Studio => "record it in Studio mode",
        crate::main_window::Mode::Instant => "record it in Instant mode",
        crate::main_window::Mode::Screenshot => "take a screenshot of it",
    }
}

/// What pressing the shortcut does, as a phrase that completes "Press ⌘⇧4
/// to ...". It follows the after-capture settings and the current mode, so
/// the row always describes what will actually happen.
fn shortcut_effect(
    action: &str,
    settings: &store::GeneralSettings,
    mode: crate::main_window::Mode,
) -> String {
    let screenshot = screenshot_outcome(settings.post_screenshot_behaviour);
    let studio_stop = match settings.post_studio_recording_behaviour {
        store::PostStudioBehaviour::OpenEditor => ", then open the editor when you stop",
        store::PostStudioBehaviour::ShowOverlay | store::PostStudioBehaviour::DoNothing => "",
    };
    let instant_stop = if settings.disable_auto_open_links {
        ", then copy its share link when you stop"
    } else {
        ", then copy and open its share link when you stop"
    };
    match action {
        "screenshotArea" => format!("select an area and {screenshot}"),
        "screenshotWindow" => format!("capture the window under your pointer and {screenshot}"),
        "screenshotDisplay" => format!("capture the display under your pointer and {screenshot}"),
        "startStudioRecording" => {
            format!("record your selected screen or window in Studio mode{studio_stop}")
        }
        "startInstantRecording" => {
            format!("record your selected screen or window in Instant mode{instant_stop}")
        }
        "openRecordingPickerArea" => format!("select an area, then {}", picker_outcome(mode)),
        "openRecordingPickerWindow" => format!("choose a window, then {}", picker_outcome(mode)),
        "openRecordingPickerDisplay" => format!("choose a display, then {}", picker_outcome(mode)),
        "openRecordingPicker" => "open Scoopy so you can choose what to record".into(),
        "cycleRecordingMode" => "switch between Studio, Instant and Screenshot mode".into(),
        "stopRecording" => "stop the current recording".into(),
        "togglePauseRecording" => "pause the current recording, or resume it".into(),
        "restartRecording" => "throw away the current take and start again".into(),
        _ => String::new(),
    }
}

fn description_follows_outcome(action: &str) -> bool {
    SCREENSHOT_SHORTCUTS.contains(&action)
        || matches!(action, "startStudioRecording" | "startInstantRecording")
}

fn capitalized_sentence(phrase: &str) -> String {
    let mut chars = phrase.chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// The gpui keystroke name for a key, as the W3C `KeyboardEvent.code` string
/// the Tauri app stores (`e.code` in hotkeys.tsx, `global_hotkey::Code` in
/// hotkeys.rs). `None` for keys neither side can bind.
fn hotkey_code_for_key(key: &str) -> Option<String> {
    if let Some(rest) = key.strip_prefix('f')
        && !rest.is_empty()
        && rest.len() <= 2
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return Some(format!("F{rest}"));
    }
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return match c {
            'a'..='z' => Some(format!("Key{}", c.to_ascii_uppercase())),
            'A'..='Z' => Some(format!("Key{c}")),
            '0'..='9' => Some(format!("Digit{c}")),
            ',' => Some("Comma".into()),
            '.' => Some("Period".into()),
            '/' => Some("Slash".into()),
            ';' => Some("Semicolon".into()),
            '\'' => Some("Quote".into()),
            '[' => Some("BracketLeft".into()),
            ']' => Some("BracketRight".into()),
            '\\' => Some("Backslash".into()),
            '-' => Some("Minus".into()),
            '=' => Some("Equal".into()),
            '`' => Some("Backquote".into()),
            _ => None,
        };
    }
    let code = match key {
        "space" => "Space",
        "enter" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        _ => return None,
    };
    Some(code.to_string())
}

fn is_function_key(code: &str) -> bool {
    code.strip_prefix('F')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

/// Bare letters, digits and Shift-only combos would fire while typing in any
/// app, so a shortcut needs ⌘, ⌃ or ⌥ unless it is a function key.
fn shortcut_needs_modifier(hotkey: &Hotkey) -> bool {
    !(hotkey.meta || hotkey.ctrl || hotkey.alt) && !is_function_key(&hotkey.code)
}

const NEEDS_MODIFIER: &str = if cfg!(target_os = "macos") {
    "Include ⌘, ⌃ or ⌥ so the shortcut doesn't get in the way of typing."
} else {
    "Include Ctrl, Alt or Win so the shortcut doesn't get in the way of typing."
};

const UNSUPPORTED_KEY: &str =
    "That key can't be used in a shortcut. Try a letter, number or F-key.";

const RECORDING_HINT: &str = "Press the keys you want to use. Esc cancels, Delete removes it.";

const BLOCKED_WHILE_RECORDING: &str =
    "Shortcuts can't be changed while this recording holds its stop key. Stop recording first.";

const UNAVAILABLE: &str =
    "Another app or the system already uses this shortcut. Click to choose a different one.";

/// `HotkeyText`'s key list: modifiers in ⌘⌃⌥⇧ order, then the main key.
/// [`ui::kbd_symbol`] does the per-OS glyph mapping.
fn hotkey_display_keys(hotkey: &Hotkey) -> Vec<String> {
    let mut keys = Vec::new();
    if hotkey.meta {
        keys.push("meta".to_string());
    }
    if hotkey.ctrl {
        keys.push("ctrl".to_string());
    }
    if hotkey.alt {
        keys.push("alt".to_string());
    }
    if hotkey.shift {
        keys.push("shift".to_string());
    }
    keys.push(hotkey.code.clone());
    keys
}

fn held_modifier_keys(modifiers: gpui::Modifiers) -> Vec<String> {
    let mut keys = Vec::new();
    if modifiers.platform {
        keys.push("meta".to_string());
    }
    if modifiers.control {
        keys.push("ctrl".to_string());
    }
    if modifiers.alt {
        keys.push("alt".to_string());
    }
    if modifiers.shift {
        keys.push("shift".to_string());
    }
    keys
}

fn hotkey_inline_text(hotkey: &Hotkey) -> String {
    let separator = if cfg!(target_os = "macos") { "" } else { "+" };
    hotkey_display_keys(hotkey)
        .iter()
        .map(|key| hotkey_key_text(key))
        .collect::<Vec<_>>()
        .join(separator)
}

fn hotkey_key_text(key: &str) -> SharedString {
    if let Some(digit) = key.strip_prefix("Digit") {
        return SharedString::from(digit.to_string());
    }
    let named = match key {
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Semicolon" => ";",
        "Quote" => "'",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Minus" => "-",
        "Equal" => "=",
        "Backquote" => "`",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "Tab" => "⇥",
        "Delete" => "⌦",
        _ => return ui::kbd_symbol(key),
    };
    SharedString::from(named)
}

fn shortcut_key_cap(theme: &Theme, key: &str) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .h(px(20.))
        .min_w(px(20.))
        .px(px(5.))
        .rounded(px(5.))
        .bg(theme.settings_card_bg())
        .border_1()
        .border_color(theme.settings_border())
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.settings_text())
        .child(hotkey_key_text(key))
}

impl SettingsWindow {
    fn shortcut_binding(&self, action: &str) -> Option<Hotkey> {
        self.pages
            .hotkeys
            .get(action)
            .and_then(store::hotkey_from_value)
    }

    fn shortcuts_start_listening(
        &mut self,
        action: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pages.shortcut_saved = None;
        self.pages.shortcut_saved_reset = None;
        if !crate::hotkeys::pause(cx) {
            self.pages.shortcut_blocked = Some(action);
            cx.notify();
            return;
        }
        self.pages.shortcut_blocked = None;
        self.pages.listening = Some(ListeningHotkey {
            action,
            modifiers: window.modifiers(),
            error: None,
        });
        // Keys must reach the root's on_key_down, not a focused field.
        self.focus_root(window, cx);
        cx.notify();
    }

    /// Leave recording mode without touching the binding. Safe to call when
    /// nothing is recording; every exit path (Escape, a click elsewhere,
    /// another page, the window losing focus or closing) lands here.
    pub(crate) fn shortcuts_cancel(&mut self, cx: &mut Context<Self>) {
        if self.pages.listening.take().is_none() {
            return;
        }
        crate::hotkeys::resume(cx);
        cx.notify();
    }

    pub(crate) fn shortcuts_modifiers_changed(
        &mut self,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        if let Some(listening) = self.pages.listening.as_mut() {
            listening.modifiers = modifiers;
            cx.notify();
        }
    }

    /// The window keydown listener while a row is recording. Returns whether
    /// the key was consumed. Modifier-only presses never arrive as
    /// `KeyDownEvent`s in gpui; they come through
    /// [`Self::shortcuts_modifiers_changed`].
    pub(crate) fn shortcuts_capture_key(
        &mut self,
        keystroke: &gpui::Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.page != Page::Shortcuts {
            return false;
        }
        let Some(listening) = self.pages.listening.as_mut() else {
            return false;
        };
        let action = listening.action;
        let modifiers = keystroke.modifiers;
        let bare = !modifiers.platform && !modifiers.control && !modifiers.alt && !modifiers.shift;

        // Escape is the picker's cancel key, so it is never a binding.
        if keystroke.key == "escape" {
            self.shortcuts_cancel(cx);
            return true;
        }
        if bare && matches!(keystroke.key.as_str(), "backspace" | "delete") {
            self.pages.listening = None;
            self.pages.hotkeys.remove(action);
            self.shortcuts_commit(cx);
            return true;
        }

        // A code the Tauri app cannot parse must never reach the store: its
        // `HotkeysStore` deserializes the whole `hotkeys` map strictly
        // (`serde_json::from_value` in `HotkeysStore::get`), so one bad entry
        // would silently drop every binding over there.
        let Some(code) = hotkey_code_for_key(&keystroke.key)
            .filter(|code| global_hotkey::hotkey::Code::from_str(code).is_ok())
        else {
            listening.error = Some(UNSUPPORTED_KEY);
            cx.notify();
            return true;
        };
        let hotkey = Hotkey {
            code,
            meta: modifiers.platform,
            ctrl: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
        };
        if shortcut_needs_modifier(&hotkey) {
            listening.error = Some(NEEDS_MODIFIER);
            cx.notify();
            return true;
        }

        let mut moved_from = None;
        for other in SCREENSHOT_SHORTCUTS
            .iter()
            .chain(RECORDING_SHORTCUTS.iter())
            .chain(WHILE_RECORDING_SHORTCUTS.iter())
        {
            if *other != action && self.shortcut_binding(other).as_ref() == Some(&hotkey) {
                self.pages.hotkeys.remove(*other);
                moved_from.get_or_insert(*other);
            }
        }

        let value = serde_json::to_value(&hotkey).unwrap_or(Value::Null);
        self.pages.hotkeys.insert(action.to_string(), value);
        self.pages.listening = None;
        self.shortcuts_commit(cx);
        self.pages.shortcut_saved = Some(SavedShortcut { action, moved_from });
        self.pages.shortcut_saved_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SHORTCUT_SAVED_NOTICE).await;
            this.update(cx, |this, cx| {
                if this
                    .pages
                    .shortcut_saved
                    .as_ref()
                    .is_some_and(|saved| saved.action == action)
                {
                    this.pages.shortcut_saved = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        true
    }

    /// Persist the map and swap the OS registrations. Deferred because the
    /// registry swap reads the store this just wrote; resuming re-registers
    /// from the store, and so does a reload when nothing was paused.
    fn shortcuts_commit(&mut self, cx: &mut Context<Self>) {
        if !store::set_hotkeys_raw(&self.pages.hotkeys) {
            tracing::warn!("saving the hotkeys store failed");
        }
        cx.defer(|cx| {
            if !crate::hotkeys::resume(cx) {
                crate::hotkeys::reload(cx);
            }
            cx.refresh_windows();
        });
        cx.notify();
    }

    pub(crate) fn render_shortcuts(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let unavailable = crate::hotkeys::unavailable_actions(cx);
        let mode = crate::main_window::Mode::from_store();
        let rows_for = |actions: &[&'static str], cx: &mut Context<Self>| {
            actions
                .iter()
                .map(|action| {
                    self.render_shortcut_row(action, mode, unavailable.contains(action), cx)
                })
                .collect::<Vec<_>>()
        };

        let mut screenshots = rows_for(&SCREENSHOT_SHORTCUTS, cx);
        screenshots.push(
            self.render_outcome_panel(
                "outcome-screenshot",
                "After a screenshot",
                "For every screenshot, from a shortcut or from Scoopy",
                &SCREENSHOT_OUTCOMES
                    .iter()
                    .map(|(value, option)| {
                        (*option, *value == self.settings.post_screenshot_behaviour)
                    })
                    .collect::<Vec<_>>(),
                |this, index, cx| {
                    if let Some((value, _)) = SCREENSHOT_OUTCOMES.get(index) {
                        this.settings.post_screenshot_behaviour = *value;
                        this.write_enum("postScreenshotBehaviour", *value, cx);
                    }
                },
                cx,
            ),
        );

        let mut recording = rows_for(&RECORDING_SHORTCUTS, cx);
        recording.push(
            self.render_outcome_panel(
                "outcome-studio",
                "After a Studio recording",
                "When you stop",
                &STUDIO_OUTCOMES
                    .iter()
                    .map(|(value, option)| {
                        (
                            *option,
                            *value == self.settings.post_studio_recording_behaviour,
                        )
                    })
                    .collect::<Vec<_>>(),
                |this, index, cx| {
                    if let Some((value, _)) = STUDIO_OUTCOMES.get(index) {
                        this.settings.post_studio_recording_behaviour = *value;
                        this.write_enum("postStudioRecordingBehaviour", *value, cx);
                    }
                },
                cx,
            ),
        );
        recording.push(
            self.render_outcome_panel(
                "outcome-instant",
                "After an Instant recording",
                "The share link is always copied",
                &INSTANT_OUTCOMES
                    .iter()
                    .map(|(disabled, option)| {
                        (*option, *disabled == self.settings.disable_auto_open_links)
                    })
                    .collect::<Vec<_>>(),
                |this, index, cx| {
                    if let Some((disabled, _)) = INSTANT_OUTCOMES.get(index) {
                        this.settings.disable_auto_open_links = *disabled;
                        this.write_bool("disableAutoOpenLinks", *disabled, cx);
                    }
                },
                cx,
            ),
        );

        let while_recording = rows_for(&WHILE_RECORDING_SHORTCUTS, cx);

        let listening_any = self.pages.listening.is_some();
        let card = |id: &'static str, rows: Vec<gpui::AnyElement>, cx: &mut Context<Self>| {
            self.rows(rows)
                .id(id)
                .when(listening_any, |this| {
                    this.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| this.shortcuts_cancel(cx)),
                    )
                })
                .into_any_element()
        };

        vec![
            self.section(
                "Screenshots",
                Some("Shortcuts work from any app. Click one to change it."),
                None,
                vec![card("shortcuts-screenshots", screenshots, cx)],
            )
            .into_any_element(),
            self.section(
                "Recording",
                Some("Start a recording without opening Scoopy first."),
                None,
                vec![card("shortcuts-recording", recording, cx)],
            )
            .into_any_element(),
            self.section(
                "While recording",
                None,
                None,
                vec![card("shortcuts-while-recording", while_recording, cx)],
            )
            .into_any_element(),
        ]
    }

    fn render_outcome_panel(
        &self,
        id: &'static str,
        title: &'static str,
        hint: &'static str,
        options: &[(OutcomeOption, bool)],
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let ink = theme.settings_text();
        // The card tokens are translucent, so on this tinted panel they read
        // as gray; the chosen option needs a solid, lighter surface.
        let raised = if theme.is_dark() {
            gpui::hsla(0., 0., 1., 0.1)
        } else {
            gpui::white()
        };
        let on_select = std::rc::Rc::new(on_select);

        let cards =
            options
                .iter()
                .enumerate()
                .map(|(index, (option, selected))| {
                    let selected = *selected;
                    let on_select = on_select.clone();
                    div()
                        .id((id, index))
                        .relative()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .flex_1()
                        .min_w_0()
                        .p(px(10.))
                        .rounded(px(10.))
                        .border_1()
                        .when(selected, |this| this.border_color(ink).bg(raised))
                        .when(!selected, |this| {
                            this.border_color(theme.settings_border())
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.settings_hover()))
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(28.))
                                .rounded(px(7.))
                                .bg(theme.settings_fill())
                                .child(svg().path(option.icon).size(px(16.)).text_color(
                                    if selected {
                                        ink
                                    } else {
                                        theme.settings_muted()
                                    },
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(ink)
                                        .child(option.label),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .line_height(px(14.))
                                        .text_color(theme.settings_muted())
                                        .child(option.hint),
                                ),
                        )
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.shortcuts_cancel(cx);
                            on_select(this, index, cx);
                            cx.notify();
                        }))
                });

        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .bg(theme.settings_fill())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.settings_muted())
                            .child(hint),
                    ),
            )
            .child(div().flex().flex_row().gap(px(8.)).children(cards))
            .into_any_element()
    }

    fn render_shortcut_row(
        &self,
        action: &'static str,
        mode: crate::main_window::Mode,
        unavailable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let binding = self.shortcut_binding(action);
        let listening = self
            .pages
            .listening
            .as_ref()
            .filter(|listening| listening.action == action);
        let saved = self
            .pages
            .shortcut_saved
            .as_ref()
            .filter(|saved| saved.action == action);
        let effect = shortcut_effect(action, &self.settings, mode);
        let red: Hsla = theme.red_11.into();
        let amber: Hsla = theme.amber_11.into();

        let blocked =
            self.pages.shortcut_blocked == Some(action) && crate::hotkeys::editing_blocked(cx);
        let (detail, detail_color, detail_icon): (String, Hsla, Option<(&str, Hsla)>) = if blocked {
            (BLOCKED_WHILE_RECORDING.into(), red, None)
        } else if let Some(listening) = listening {
            match listening.error {
                Some(error) => (error.into(), red, None),
                None => (RECORDING_HINT.into(), theme.settings_muted(), None),
            }
        } else if unavailable && binding.is_some() {
            (UNAVAILABLE.into(), amber, None)
        } else if let (Some(saved), Some(binding)) = (saved, binding.as_ref()) {
            let mut text = format!(
                "Saved. Press {} in any app to {effect}.",
                hotkey_inline_text(binding)
            );
            if let Some(from) = saved.moved_from {
                text.push_str(&format!(" Removed from {}.", shortcut_label(from)));
            }
            (
                text,
                theme.settings_text(),
                Some(("icons/circle-check.svg", theme.settings_text())),
            )
        } else {
            (capitalized_sentence(&effect), theme.settings_muted(), None)
        };

        let group: SharedString = format!("shortcut-row-{action}").into();

        let field = self.render_shortcut_field(action, binding.as_ref(), listening, cx);
        let clear = div()
            .id(SharedString::from(format!("shortcut-clear-{action}")))
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.))
            .rounded(px(6.))
            .cursor_pointer()
            .invisible()
            .when(binding.is_some() && listening.is_none(), |this| {
                this.group_hover(group.clone(), |style| style.visible())
            })
            .hover(|style| style.bg(theme.settings_hover()))
            .child(
                svg()
                    .path("icons/x.svg")
                    .size(px(12.))
                    .text_color(theme.settings_muted()),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.shortcuts_cancel(cx);
                this.pages.hotkeys.remove(action);
                if this
                    .pages
                    .shortcut_saved
                    .as_ref()
                    .is_some_and(|saved| saved.action == action)
                {
                    this.pages.shortcut_saved = None;
                }
                this.shortcuts_commit(cx);
            }));

        div()
            .id(SharedString::from(format!("shortcut-{action}")))
            .group(group)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .min_h(px(56.))
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(div().text_size(px(13.)).child(shortcut_label(action)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(5.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(detail_color)
                            // Picking an "After ..." option rewrites this text,
                            // and a one-line/two-line swap moved the panel out
                            // from under the pointer. Two lines are reserved
                            // for every description that can change that way.
                            .when(description_follows_outcome(action), |this| {
                                this.min_h(px(32.))
                            })
                            .when_some(detail_icon, |this, (path, color)| {
                                this.child(
                                    svg()
                                        .path(path)
                                        .flex_shrink_0()
                                        .mt(px(2.))
                                        .size(px(12.))
                                        .text_color(color),
                                )
                            })
                            .child(div().min_w_0().child(detail)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(px(4.))
                    .flex_shrink_0()
                    .child(clear)
                    .child(field),
            )
            .into_any_element()
    }

    fn render_shortcut_field(
        &self,
        action: &'static str,
        binding: Option<&Hotkey>,
        listening: Option<&ListeningHotkey>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;

        let base = div()
            .id(SharedString::from(format!("shortcut-field-{action}")))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(3.))
            .h(px(30.))
            .min_w(px(132.))
            .px(px(5.))
            .rounded(px(8.))
            .border_1()
            .cursor_pointer();

        let field = match (listening, binding) {
            (Some(listening), _) => {
                let held = held_modifier_keys(listening.modifiers);
                let mut field = base
                    .border_color(theme.settings_text())
                    .bg(theme.settings_card_bg())
                    .children(held.iter().map(|key| shortcut_key_cap(&theme, key)));
                if held.is_empty() {
                    field = field.child(
                        div()
                            .px(px(4.))
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child("Type shortcut…"),
                    );
                } else {
                    field = field.child(
                        div()
                            .px(px(2.))
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child("…"),
                    );
                }
                field.into_any_element()
            }
            (None, Some(hotkey)) => base
                .border_color(theme.settings_border())
                .bg(theme.settings_fill())
                .hover(|style| style.bg(theme.settings_hover()))
                .children(
                    hotkey_display_keys(hotkey)
                        .iter()
                        .map(|key| shortcut_key_cap(&theme, key)),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.shortcuts_start_listening(action, window, cx);
                }))
                .into_any_element(),
            (None, None) => base
                .border_color(theme.settings_border())
                .border_dashed()
                .hover(|style| style.bg(theme.settings_hover()))
                .child(
                    div()
                        .px(px(4.))
                        .text_size(px(12.))
                        .text_color(theme.settings_muted())
                        .child("Record shortcut"),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.shortcuts_start_listening(action, window, cx);
                }))
                .into_any_element(),
        };
        div().child(field).into_any_element()
    }
}

// ---------------------------------------------------------------------------
// CLI (cli.tsx)
// ---------------------------------------------------------------------------

impl SettingsWindow {
    fn cli_refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pages.cli_status = None;
        cx.spawn_in(window, async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async { cli_install::status() })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.pages.cli_status = Some(status);
                cx.notify();
                window.refresh();
            })
            .ok();
        })
        .detach();
    }

    fn cli_run(&mut self, uninstall: bool, window: &mut Window, cx: &mut Context<Self>) {
        if uninstall {
            self.pages.cli_uninstalling = true;
        } else {
            self.pages.cli_installing = true;
        }
        self.pages.cli_error = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if uninstall {
                        cli_install::uninstall()
                    } else {
                        cli_install::install()
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.pages.cli_installing = false;
                this.pages.cli_uninstalling = false;
                match result {
                    Ok(status) => this.pages.cli_status = Some(Ok(status)),
                    Err(error) => {
                        // `toast.error(..)` then `refetch()` in cli.tsx.
                        this.pages.cli_error = Some(error);
                        this.cli_refresh(window, cx);
                    }
                }
                cx.notify();
                window.refresh();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn render_cli(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = self.theme;

        let body: gpui::AnyElement = match &self.pages.cli_status {
            // `h-20 rounded-lg bg-gray-3 animate-pulse` (static here -- no
            // keyframe ticker for a placeholder).
            None => div()
                .h(px(80.))
                .rounded(px(8.))
                .bg(theme.settings_fill())
                .into_any_element(),
            Some(Err(error)) => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(Hsla::from(theme.red_9))
                        .child(format!("Couldn't load CLI status: {error}")),
                )
                .child(div().child(self.button(
                    "cli-retry",
                    (ui::ButtonVariant::Gray, None),
                    "Retry",
                    false,
                    cx,
                    |this, window, cx| this.cli_refresh(window, cx),
                )))
                .into_any_element(),
            Some(Ok(status)) => self
                .render_cli_status(status.clone(), cx)
                .into_any_element(),
        };

        vec![
            self.section(
                "Command Line",
                Some(
                    "Install the Scoopy command for terminals, agents, scripts, and local automation.",
                ),
                None,
                vec![self.card(true).child(body).into_any_element()],
            )
            .into_any_element(),
        ]
    }

    fn render_cli_status(
        &self,
        status: cli_install::CliInstallStatus,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let installing = self.pages.cli_installing;
        let uninstalling = self.pages.cli_uninstalling;

        let install_label = if installing {
            if status.installed {
                "Repairing..."
            } else {
                "Installing..."
            }
        } else if status.installed {
            "Repair"
        } else {
            "Install CLI"
        };

        // `<code class="font-mono text-gray-12">`.
        let code_span = |text: String| {
            div()
                .font_family(MONO_FONT)
                .text_color(theme.settings_text())
                .child(text)
        };

        let header = div()
            .flex()
            .flex_row()
            .items_start()
            .justify_between()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .min_w_0()
                    .child(div().text_size(px(13.)).child(if status.installed {
                        "Installed"
                    } else {
                        "Not installed"
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(theme.settings_muted())
                            .child("The desktop app installs a local\u{a0}")
                            .child(code_span("cap".into()).text_size(px(12.)))
                            .child("\u{a0}command that points back to the bundled CLI."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_shrink_0()
                    .gap(px(8.))
                    .when(status.installed, |this| {
                        this.child(self.button(
                            "cli-remove",
                            (ui::ButtonVariant::Gray, None),
                            if uninstalling {
                                "Removing..."
                            } else {
                                "Remove"
                            },
                            uninstalling,
                            cx,
                            |this, window, cx| this.cli_run(true, window, cx),
                        ))
                    })
                    .child(self.button(
                        "cli-install",
                        (ui::ButtonVariant::Dark, None),
                        install_label,
                        installing,
                        cx,
                        |this, window, cx| this.cli_run(false, window, cx),
                    )),
            );

        // `<PathRow>`: `w-16` label, mono value chip.
        let path_row = |label: &'static str, value: String| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .min_w_0()
                .child(
                    div()
                        .w(px(64.))
                        .flex_shrink_0()
                        .text_size(px(12.))
                        .text_color(theme.settings_muted())
                        .child(label),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(px(6.))
                        .bg(theme.settings_fill())
                        .font_family(MONO_FONT)
                        .text_size(px(11.))
                        .text_color(theme.settings_text())
                        .child(value),
                )
        };

        let shell_command = status.shell_command.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(path_row("Command", status.shim_path.clone()))
                    .child(path_row("Target", status.target_path.clone())),
            )
            .when_some(status.conflict.clone(), |this, conflict| {
                // `rounded-lg border border-red-300/40 bg-red-500/10 px-3 py-2
                //  text-xs text-red-11`.
                this.child(
                    div()
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Theme::with_alpha(theme.red_9, 0.4))
                        .bg(Theme::with_alpha(theme.red_9, 0.1))
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(Hsla::from(theme.red_9))
                        .child(conflict),
                )
            })
            .when_some(self.pages.cli_error.clone(), |this, error| {
                // cli.tsx surfaces install/remove failures as toasts; inline
                // red text is the stand-in.
                this.child(
                    div()
                        .text_size(px(12.))
                        .text_color(Hsla::from(theme.red_9))
                        .child(error),
                )
            })
            .when(status.installed && !status.on_path, |this| {
                let help: gpui::AnyElement = if status.path_configured {
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(theme.settings_muted())
                        .child(
                            "Added cap to your PATH. Restart your terminal to use it, or run \
                             this now:",
                        )
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .text_color(theme.settings_muted())
                        .child(format!(
                            "Add {} to your PATH to use cap from a new terminal.",
                            status.path_entry
                        ))
                        .into_any_element()
                };
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(12.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.settings_border())
                        .bg(theme.settings_fill())
                        .child(help)
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(6.))
                                        .bg(theme.settings_card_bg())
                                        .font_family(MONO_FONT)
                                        .text_size(px(12.))
                                        .text_color(theme.settings_text())
                                        .child(status.shell_command.clone()),
                                )
                                .child(self.button(
                                    "cli-copy-path",
                                    (ui::ButtonVariant::Gray, None),
                                    if self.pages.cli_copied {
                                        "Copied"
                                    } else {
                                        "Copy"
                                    },
                                    false,
                                    cx,
                                    move |this, window, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            shell_command.clone(),
                                        ));
                                        this.pages.cli_copied = true;
                                        this.pages.cli_copied_reset =
                                            Some(cx.spawn_in(window, async move |this, cx| {
                                                cx.background_executor()
                                                    .timer(Duration::from_millis(1500))
                                                    .await;
                                                this.update(cx, |this, cx| {
                                                    this.pages.cli_copied = false;
                                                    cx.notify();
                                                })
                                                .ok();
                                            }));
                                        cx.notify();
                                    },
                                )),
                        ),
                )
            })
    }
}

// ---------------------------------------------------------------------------
// Transcription (transcription.tsx)
// ---------------------------------------------------------------------------

impl SettingsWindow {
    /// `addHint()`.
    fn transcription_add_hint(&mut self, cx: &mut Context<Self>) {
        let value = self.pages.hint_draft.replace('\0', "");
        let value = value.trim().to_string();
        if value.is_empty() {
            return;
        }

        let mut next = self.pages.hints.clone();
        next.push(value);
        let next = store::normalize_transcription_hints(next);
        let input = self.pages.hint_input.clone();
        input.update(cx, |input, cx| input.set_text("", cx));
        self.pages.hint_draft.clear();
        if next.len() == self.pages.hints.len() {
            cx.notify();
            return;
        }
        self.pages.hints = next.clone();
        self.transcription_persist(next, cx);
    }

    /// `persist()`: 250ms debounce, then the store write, then the 1.2s
    /// "Saved" hold. Assigning over the task drops the pending write, which
    /// is `clearTimeout`.
    fn transcription_persist(&mut self, hints: Vec<String>, cx: &mut Context<Self>) {
        let normalized = store::normalize_transcription_hints(hints);
        self.pages.hint_save = HintSave::Saving;
        self.pages.hint_save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let ok = this
                .update(cx, |this, cx| {
                    let ok = store::set_store_setting(
                        GENERAL_SETTINGS,
                        "transcriptionHints",
                        Value::Array(normalized.iter().cloned().map(Value::String).collect()),
                    );
                    this.pages.hint_save = if ok { HintSave::Saved } else { HintSave::Idle };
                    cx.notify();
                    ok
                })
                .unwrap_or(false);
            if !ok {
                return;
            }
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            this.update(cx, |this, cx| {
                if this.pages.hint_save == HintSave::Saved {
                    this.pages.hint_save = HintSave::Idle;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn transcription_remove_hint(&mut self, hint: String, cx: &mut Context<Self>) {
        self.pages.hints.retain(|existing| *existing != hint);
        let next = self.pages.hints.clone();
        self.transcription_persist(next, cx);
    }

    pub(crate) fn render_transcription(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = self.theme;
        let hints = self.pages.hints.clone();
        let save_label = match self.pages.hint_save {
            HintSave::Saving => "Saving...",
            HintSave::Saved => "Saved",
            HintSave::Idle => "",
        };

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .min_w_0()
                    .child(div().text_size(px(13.)).child("Remembered terms"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(theme.settings_muted())
                            .child(
                                "Add one term at a time to reduce typos and formatting mistakes.",
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .when(!hints.is_empty(), |this| {
                        this.child(self.button(
                            "hints-clear",
                            (ui::ButtonVariant::Gray, None),
                            "Clear",
                            false,
                            cx,
                            |this, _window, cx| {
                                this.pages.hints.clear();
                                this.transcription_persist(Vec::new(), cx);
                            },
                        ))
                    })
                    .child(
                        // `min-w-15 text-right text-xs text-gray-11`.
                        div()
                            .min_w(px(60.))
                            .text_size(px(12.))
                            .text_color(theme.settings_muted())
                            .child(save_label),
                    ),
            );

        let add_disabled = self.pages.hint_draft.trim().is_empty();
        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.pages_input("hint-input", &self.pages.hint_input)),
            )
            .child(self.button(
                "hints-add",
                (ui::ButtonVariant::Primary, Some("icons/plus.svg")),
                "Add",
                add_disabled,
                cx,
                |this, _window, cx| this.transcription_add_hint(cx),
            ));

        let card = self
            .card(true)
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(header)
            .child(input_row)
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(theme.settings_muted())
                    .child("These hints are applied when you generate captions in the editor."),
            );

        let mut sections = vec![
            self.section(
                "Transcription",
                Some(
                    "Add names, spellings, domains, and capitalization preferences that caption \
                     generation should keep in mind.",
                ),
                None,
                vec![card.into_any_element()],
            )
            .into_any_element(),
        ];

        if !hints.is_empty() {
            let count = div()
                .text_size(px(12.))
                .text_color(theme.settings_muted())
                .child(format!(
                    "{} {}",
                    hints.len(),
                    if hints.len() == 1 { "item" } else { "items" }
                ))
                .into_any_element();
            let chips = div().flex().flex_row().flex_wrap().gap(px(8.)).children(
                hints.into_iter().enumerate().map(|(index, hint)| {
                    let remove = hint.clone();
                    // `px-2.5 py-1 rounded-full text-xs bg-gray-3 border
                    //  border-gray-4 hover:bg-gray-4`.
                    div()
                        .id(("hint-chip", index))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .px(px(10.))
                        .py(px(4.))
                        .rounded_full()
                        .bg(theme.settings_fill())
                        .border_1()
                        .border_color(theme.settings_border())
                        .text_size(px(12.))
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.settings_selection()))
                        .child(hint)
                        .child(
                            svg()
                                .path("icons/x.svg")
                                .size(px(12.))
                                .text_color(theme.settings_muted()),
                        )
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.transcription_remove_hint(remove.clone(), cx);
                        }))
                }),
            );

            sections.push(
                self.section(
                    "Active hints",
                    None,
                    Some(count),
                    vec![self.card(true).child(chips).into_any_element()],
                )
                .into_any_element(),
            );
        }

        sections
    }
}

// ---------------------------------------------------------------------------
// Experimental (experimental.tsx)
// ---------------------------------------------------------------------------

pub(crate) enum SwitchBack {
    Running(std::time::Instant),
    WaitingForClassic,
    Failed(String),
}

/// A cold dev build can genuinely take this long; past it, assume the build
/// failed and hand the user back their app with a pointer at the terminal.
const CLASSIC_WAIT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

const SWITCH_SENTENCES: &[&str] = &[
    "Switching back to the classic Scoopy app.",
    "Your recordings and settings stay exactly where they are.",
];
const SWITCH_SENTENCE_MS: u64 = 2000;
const SWITCH_FADE_MS: f32 = 400.;
const SWITCH_COUNTDOWN_FROM: u32 = 5;
const SWITCH_TAKEOVER_MS: u64 = SWITCH_COUNTDOWN_FROM as u64 * 1000;

/// The overlay's whole appearance at one instant: which sentence, how opaque,
/// and the number.
fn takeover_frame(elapsed_ms: f32) -> (usize, f32, u32) {
    let last = SWITCH_SENTENCES.len() - 1;
    let index = ((elapsed_ms / SWITCH_SENTENCE_MS as f32) as usize).min(last);
    let within = elapsed_ms - index as f32 * SWITCH_SENTENCE_MS as f32;
    let fade_in = (within / SWITCH_FADE_MS).clamp(0., 1.);
    // The last line holds rather than fading out; blanking the column while the
    // countdown finishes would read as a stall.
    let alpha = if index == last {
        fade_in
    } else {
        fade_in.min(((SWITCH_SENTENCE_MS as f32 - within) / SWITCH_FADE_MS).clamp(0., 1.))
    };
    // Five on the first frame down to one in the last second; the switch fires
    // rather than ever showing zero.
    let left = SWITCH_TAKEOVER_MS as f32 - elapsed_ms;
    let remaining = (left / 1000.)
        .ceil()
        .clamp(1., SWITCH_COUNTDOWN_FROM as f32) as u32;
    (index, alpha, remaining)
}

/// Cap ships as this app alone, so the classic Tauri app only exists inside
/// the `tauri dev` harness: a debug cargo build can hand the session back to
/// it through the dev-session supervisor (`store::request_classic_reopen`),
/// and nothing else has a classic app to switch to.
fn dev_supervisor_available_for(exe: Option<&std::path::Path>, debug_build: bool) -> bool {
    debug_build
        && exe.is_some_and(|exe| {
            exe.components()
                .any(|component| component.as_os_str() == "target")
        })
}

fn dev_supervisor_available() -> bool {
    dev_supervisor_available_for(
        std::env::current_exe().ok().as_deref(),
        cfg!(debug_assertions),
    )
}

impl SettingsWindow {
    pub(crate) fn render_experimental(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        // No "Native camera preview" toggle here: in this app the native
        // camera path is the only implementation, so the Tauri page's
        // experimental switch has nothing to switch. The store key
        // (`enableNativeCameraPreview`) stays readable in `store.rs` and is
        // left untouched in the shared store -- the Tauri app still uses it.
        let mut sections = vec![
            self.section(
                "Reliability",
                None,
                None,
                vec![
                    self.rows(vec![
                        self.setting_row(
                            "Out-of-process muxer",
                            Some(
                                "Run the fragmented-MP4 muxer in an isolated subprocess so muxer \
                             crashes can't take down your recording. Requires the bundled \
                             cap-muxer binary.",
                            ),
                            self.toggle(
                                "out-of-process-muxer",
                                self.settings.out_of_process_muxer,
                                cx,
                                |this, cx| {
                                    this.settings.out_of_process_muxer =
                                        !this.settings.out_of_process_muxer;
                                    let value = this.settings.out_of_process_muxer;
                                    this.write_bool("outOfProcessMuxer", value, cx);
                                },
                            )
                            .into_any_element(),
                        ),
                    ])
                    .into_any_element(),
                ],
            )
            .into_any_element(),
        ];
        if dev_supervisor_available() {
            sections.push(
                self.section(
                    "Developer",
                    Some("Only shown in development builds."),
                    None,
                    vec![self.rows(vec![self.classic_app_row(cx)]).into_any_element()],
                )
                .into_any_element(),
            );
        }
        sections
    }

    /// The dev harness's way back to the classic app: being in this app is what
    /// the toggle means, and turning it off reopens the classic app under
    /// `tauri dev`.
    fn classic_app_row(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let checked = !matches!(self.pages.switch_back, Some(SwitchBack::Running(_)));
        self.setting_row(
            "Use native app",
            Some(
                "Turn off to close this app and reopen the classic Tauri app in the dev harness. \
                 Recordings and settings are shared.",
            ),
            self.toggle("enable-gpui-app", checked, cx, |this, cx| {
                match this.pages.switch_back {
                    Some(SwitchBack::Running(_)) => this.cancel_switch_back(cx),
                    // Committed: the classic app is already being brought up.
                    Some(SwitchBack::WaitingForClassic) => {}
                    _ => this.start_switch_back(cx),
                }
            })
            .into_any_element(),
        )
    }

    /// The full-window takeover, painted from `SettingsWindow::render` so it
    /// covers the sidebar and the menus too.
    pub(crate) fn render_switch_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let switch = self.pages.switch_back.as_ref()?;
        let white = gpui::white();
        let mut column = div()
            .absolute()
            .inset_0()
            .when(cfg!(target_os = "windows"), |overlay| overlay.top(px(36.)))
            // Nothing behind the takeover is clickable while it runs.
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(28.))
            .bg(gpui::hsla(0., 0., 0., 0.92));

        match switch {
            SwitchBack::Running(started) => {
                let (index, alpha, remaining) =
                    takeover_frame(started.elapsed().as_secs_f32() * 1000.);
                column = column
                    .child(
                        div()
                            .text_size(px(72.))
                            .line_height(px(76.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(white)
                            .child(SharedString::from(remaining.to_string())),
                    )
                    .child(
                        div()
                            .h(px(40.))
                            .max_w(px(380.))
                            .flex()
                            .items_center()
                            .text_size(px(15.))
                            .text_center()
                            .text_color(gpui::hsla(0., 0., 1., alpha))
                            .child(SWITCH_SENTENCES[index]),
                    );
            }
            SwitchBack::WaitingForClassic => {
                column = column
                    .child(
                        div()
                            .max_w(px(380.))
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_center()
                            .text_color(white)
                            .child("Opening the classic Scoopy app"),
                    )
                    .child(
                        div()
                            .max_w(px(380.))
                            .text_size(px(14.))
                            .text_center()
                            .text_color(gpui::hsla(0., 0., 1., 0.7))
                            .child("This window will close when the classic Scoopy app is ready."),
                    );
            }
            SwitchBack::Failed(error) => {
                column = column.child(
                    div()
                        .max_w(px(380.))
                        .text_size(px(14.))
                        .text_center()
                        .text_color(rgb(0xf87171))
                        .child(SharedString::from(error.clone())),
                );
            }
        }

        // Committed states have nothing to cancel.
        if matches!(switch, SwitchBack::WaitingForClassic) {
            return Some(column.into_any_element());
        }

        Some(
            column
                .child(
                    div()
                        .id("switch-back-cancel")
                        .flex()
                        .items_center()
                        .px(px(16.))
                        .h(px(32.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(gpui::hsla(0., 0., 1., 0.25))
                        .text_size(px(13.))
                        .text_color(white)
                        .cursor_pointer()
                        .hover(|style| style.bg(gpui::hsla(0., 0., 1., 0.12)))
                        .on_click(cx.listener(|this, _, _window, cx| this.cancel_switch_back(cx)))
                        .child(if matches!(switch, SwitchBack::Failed(_)) {
                            "Close"
                        } else {
                            "Cancel"
                        }),
                )
                .into_any_element(),
        )
    }

    fn start_switch_back(&mut self, cx: &mut Context<Self>) {
        self.pages.switch_back = Some(SwitchBack::Running(std::time::Instant::now()));
        // gpui only renders on invalidation, so the fades and the countdown
        // need a pulse to run at all -- the `toggle_placeholders` shape.
        // Assigning over the previous task drops it, which is how Cancel and a
        // restart stop the one already in flight.
        self.pages.switch_back_ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let running = this
                    .update(cx, |this, cx| {
                        let started = match &this.pages.switch_back {
                            Some(SwitchBack::Running(started)) => *started,
                            _ => return false,
                        };
                        if started.elapsed() >= Duration::from_millis(SWITCH_TAKEOVER_MS) {
                            this.finish_switch_back(cx);
                            return false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !running {
                    return;
                }
            }
        }));
        cx.notify();
    }

    fn cancel_switch_back(&mut self, cx: &mut Context<Self>) {
        self.pages.switch_back = None;
        self.pages.switch_back_ticker = None;
        cx.notify();
    }

    /// Hand the session (`enableGpuiApp`) to the classic dev app, ask the dev
    /// supervisor to start it, then quit once it is visible.
    fn finish_switch_back(&mut self, cx: &mut Context<Self>) {
        if !dev_supervisor_available() {
            self.pages.switch_back = Some(SwitchBack::Failed(
                "Couldn't find the classic Scoopy app to switch back to.".to_string(),
            ));
            cx.notify();
            return;
        }
        if let Err(error) = crate::app_windows::flush_pending_editor_saves(cx) {
            self.pages.switch_back = Some(SwitchBack::Failed(error));
            cx.notify();
            return;
        }

        if !store::set_store_setting(GENERAL_SETTINGS, "enableGpuiApp", Value::Bool(false)) {
            self.pages.switch_back = Some(SwitchBack::Failed(
                "Couldn't save your app preference. Scoopy is still open.".to_string(),
            ));
            cx.notify();
            return;
        }
        let pending = store::mark_classic_pending();
        self.pages.switch_back = Some(SwitchBack::WaitingForClassic);
        self.pages.switch_back_ticker = None;
        cx.notify();

        // The handoff must finish even if its settings window closes.
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { pending.and_then(|()| store::request_classic_reopen()) })
                .await;
            let mut failure = result
                .err()
                .map(|error| format!("Couldn't open the classic Scoopy app: {error}"));
            let started = std::time::Instant::now();
            while failure.is_none() {
                match store::classic_pending_path().try_exists() {
                    Ok(false) => {
                        tracing::info!("classic app is visible; quitting GPUI");
                        cx.update(crate::menus::quit);
                        return;
                    }
                    Ok(true) => {}
                    Err(error) => {
                        failure = Some(format!(
                            "Couldn't check whether the classic Scoopy app opened: {error}"
                        ));
                        break;
                    }
                }
                if started.elapsed() >= CLASSIC_WAIT_TIMEOUT {
                    failure = Some(
                        "The classic app hasn't opened. Check the dev terminal for build errors, then try again."
                            .to_string(),
                    );
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
            }
            let message =
                failure.unwrap_or_else(|| "Couldn't open the classic Scoopy app.".to_string());
            tracing::error!("{message}");
            store::clear_classic_pending();
            store::set_store_setting(GENERAL_SETTINGS, "enableGpuiApp", Value::Bool(true));
            this.update(cx, |this, cx| {
                this.pages.switch_back = Some(SwitchBack::Failed(message));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

// ---------------------------------------------------------------------------
// CLI install (crates/cli-install/src/lib.rs, ported)
// ---------------------------------------------------------------------------

/// A direct port of `crates/cli-install` -- this standalone workspace cannot
/// depend on the crate, and the whole point is that `status`/`install`/
/// `uninstall` behave byte-for-byte like the Tauri commands
/// (`src-tauri/src/cli.rs`) so the two apps manage the same shim.
mod cli_install {
    #[cfg(unix)]
    use std::ffi::OsStr;
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };

    const CAP_DIR_NAME: &str = ".cap";
    const BIN_DIR_NAME: &str = "bin";
    const CLI_BINARY_STEM: &str = "cap-cli";

    #[cfg(windows)]
    const SHIM_NAME: &str = "cap.cmd";
    #[cfg(not(windows))]
    const SHIM_NAME: &str = "cap";

    #[cfg(windows)]
    const CLI_BINARY_NAME: &str = "cap-cli.exe";
    #[cfg(not(windows))]
    const CLI_BINARY_NAME: &str = "cap-cli";

    /// `CliInstallStatus`, minus the serde/specta derives the commands need
    /// and minus `install_dir`, which neither cli.tsx nor this page displays
    /// (`path_entry` carries the same string).
    #[derive(Clone, Debug)]
    pub(crate) struct CliInstallStatus {
        pub shim_path: String,
        pub target_path: String,
        pub installed: bool,
        pub on_path: bool,
        pub conflict: Option<String>,
        pub path_entry: String,
        pub shell_command: String,
        pub path_configured: bool,
    }

    fn home_dir() -> Result<PathBuf, String> {
        dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())
    }

    fn install_dir() -> Result<PathBuf, String> {
        let home = home_dir()?;
        let cap_bin = home.join(CAP_DIR_NAME).join(BIN_DIR_NAME);
        let local_bin = home.join(".local/bin");

        // Prefer whichever candidate already holds a Cap-managed shim, so
        // status and install agree regardless of the launching PATH.
        if let Ok(target) = target_path() {
            for candidate in [&cap_bin, &local_bin] {
                let shim = candidate.join(SHIM_NAME);
                if shim_points_to(&shim, &target).unwrap_or(false) || shim_is_cap_managed(&shim) {
                    return Ok(candidate.clone());
                }
            }
        }

        if path_is_present(&cap_bin.join(SHIM_NAME)) || cfg!(windows) {
            return Ok(cap_bin);
        }

        if path_contains_install_dir(&local_bin) {
            return Ok(local_bin);
        }

        Ok(cap_bin)
    }

    fn shim_path() -> Result<PathBuf, String> {
        Ok(install_dir()?.join(SHIM_NAME))
    }

    fn target_path() -> Result<PathBuf, String> {
        let exe =
            env::current_exe().map_err(|e| format!("Could not locate Scoopy executable: {e}"))?;
        let exe = resolve_path_for_target_lookup(exe);
        let dir = exe
            .parent()
            .ok_or_else(|| "Could not locate Scoopy executable directory".to_string())?;

        for candidate in cli_binary_candidates(dir) {
            if candidate.exists() {
                return Ok(resolve_path_for_target_lookup(candidate));
            }
        }

        Ok(dir.join(CLI_BINARY_NAME))
    }

    #[cfg(windows)]
    fn resolve_path_for_target_lookup(path: PathBuf) -> PathBuf {
        path
    }

    #[cfg(not(windows))]
    fn resolve_path_for_target_lookup(path: PathBuf) -> PathBuf {
        fs::canonicalize(&path).unwrap_or(path)
    }

    fn cli_binary_candidates(dir: &Path) -> Vec<PathBuf> {
        let mut names = vec![CLI_BINARY_NAME.to_string()];
        if let Some(target_triple) = current_target_triple() {
            names.push(target_specific_cli_binary_name(target_triple));
        }

        let dirs = [
            dir.to_path_buf(),
            dir.join("../MacOS"),
            dir.join("../Resources"),
        ];
        let mut candidates = Vec::new();
        for dir in dirs {
            for name in &names {
                candidates.push(dir.join(name));
            }
        }
        candidates
    }

    fn target_specific_cli_binary_name(target_triple: &str) -> String {
        format!(
            "{CLI_BINARY_STEM}-{target_triple}{}",
            exe_suffix_for_target(target_triple)
        )
    }

    fn exe_suffix_for_target(target_triple: &str) -> &'static str {
        if target_triple.contains("windows") {
            ".exe"
        } else {
            ""
        }
    }

    fn current_target_triple() -> Option<&'static str> {
        if cfg!(all(
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        )) {
            Some("x86_64-pc-windows-msvc")
        } else if cfg!(all(
            target_os = "windows",
            target_arch = "aarch64",
            target_env = "msvc"
        )) {
            Some("aarch64-pc-windows-msvc")
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some("aarch64-apple-darwin")
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            Some("x86_64-apple-darwin")
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some("x86_64-unknown-linux-gnu")
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Some("aarch64-unknown-linux-gnu")
        } else {
            None
        }
    }

    #[cfg(unix)]
    fn cli_binary_file_name_is_cap_managed(name: &OsStr) -> bool {
        if name == CLI_BINARY_NAME {
            return true;
        }

        let Some(name) = name.to_str() else {
            return false;
        };

        current_target_triple().is_some_and(|target_triple| {
            name.eq_ignore_ascii_case(&target_specific_cli_binary_name(target_triple))
        })
    }

    fn display_path(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    fn path_is_present(path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok()
    }

    fn path_contains_install_dir(install_dir: &Path) -> bool {
        let Some(path) = env::var_os("PATH") else {
            return false;
        };

        env::split_paths(&path).any(|entry| entry == install_dir)
    }

    #[cfg(unix)]
    fn shim_points_to(shim_path: &Path, target_path: &Path) -> Result<bool, String> {
        match fs::read_link(shim_path) {
            Ok(link) => Ok(link == target_path || same_file(&link, target_path)),
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput
                ) =>
            {
                Ok(false)
            }
            Err(err) => Err(format!("Could not read CLI shim: {err}")),
        }
    }

    #[cfg(unix)]
    fn same_file(a: &Path, b: &Path) -> bool {
        match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }

    #[cfg(windows)]
    fn shim_points_to(shim_path: &Path, target_path: &Path) -> Result<bool, String> {
        let contents = match fs::read(shim_path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(format!("Could not read CLI shim: {err}")),
        };

        let target = windows_command_path(target_path);
        Ok(windows_shim_target(&contents).is_some_and(|shim_target| {
            windows_shim_target_matches(shim_target, &target, |name| env::var(name).ok())
        }))
    }

    #[cfg(unix)]
    fn shim_is_cap_managed(shim_path: &Path) -> bool {
        match fs::read_link(shim_path) {
            Ok(link) => link
                .file_name()
                .is_some_and(cli_binary_file_name_is_cap_managed),
            Err(_) => false,
        }
    }

    #[cfg(windows)]
    fn shim_is_cap_managed(shim_path: &Path) -> bool {
        fs::read(shim_path).is_ok_and(|contents| windows_shim_target(&contents).is_some())
    }

    #[cfg(windows)]
    fn windows_shim_target(contents: &[u8]) -> Option<&[u8]> {
        let contents = contents.strip_prefix(b"\xef\xbb\xbf").unwrap_or(contents);
        let mut lines = contents.split(|byte| *byte == b'\n');
        if !trim_ascii_whitespace(lines.next()?).eq_ignore_ascii_case(b"@echo off") {
            return None;
        }

        let command = trim_ascii_whitespace(lines.next()?);
        if lines.any(|line| !trim_ascii_whitespace(line).is_empty()) {
            return None;
        }

        let target = command.strip_prefix(b"\"")?.strip_suffix(b"\" %*")?;
        if windows_cli_binary_file_name_is_cap_managed(windows_path_file_name(target)) {
            Some(target)
        } else {
            None
        }
    }

    #[cfg(windows)]
    fn windows_path_file_name(path: &[u8]) -> &[u8] {
        path.rsplit(|byte| *byte == b'\\' || *byte == b'/')
            .next()
            .unwrap_or(path)
    }

    #[cfg(windows)]
    fn windows_cli_binary_file_name_is_cap_managed(name: &[u8]) -> bool {
        name.eq_ignore_ascii_case(b"cap-cli.exe")
            || name.eq_ignore_ascii_case(b"cap-cli-x86_64-pc-windows-msvc.exe")
            || name.eq_ignore_ascii_case(b"cap-cli-aarch64-pc-windows-msvc.exe")
    }

    #[cfg(windows)]
    fn windows_command_path(path: &Path) -> String {
        let path = display_path(path);

        if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
            return format!("\\\\{rest}");
        }

        if let Some(rest) = path.strip_prefix("\\\\?\\") {
            return rest.to_string();
        }

        path
    }

    #[cfg(windows)]
    fn windows_shim_target_matches<F>(shim_target: &[u8], target: &str, env_value: F) -> bool
    where
        F: FnMut(&str) -> Option<String>,
    {
        if shim_target.eq_ignore_ascii_case(target.as_bytes()) {
            return true;
        }

        let Ok(shim_target) = std::str::from_utf8(shim_target) else {
            return false;
        };

        windows_expand_env_prefix(shim_target, env_value)
            .is_some_and(|expanded| expanded.eq_ignore_ascii_case(target))
    }

    #[cfg(windows)]
    fn windows_expand_env_prefix<F>(target: &str, mut env_value: F) -> Option<String>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let rest = target.strip_prefix('%')?;
        let (name, suffix) = rest.split_once('%')?;
        if name.is_empty() {
            return None;
        }

        env_value(name).map(|value| format!("{value}{suffix}"))
    }

    #[cfg(windows)]
    fn windows_env_prefixed_path<F>(target: &str, mut env_value: F) -> Option<String>
    where
        F: FnMut(&str) -> Option<String>,
    {
        for name in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
            let Some(value) = env_value(name) else {
                continue;
            };
            let Some(suffix) = strip_ascii_prefix(target, &value) else {
                continue;
            };
            if suffix.is_empty() || suffix.starts_with('\\') || suffix.starts_with('/') {
                return Some(format!("%{name}%{suffix}"));
            }
        }

        None
    }

    #[cfg(windows)]
    fn strip_ascii_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
        let head = value.get(..prefix.len())?;
        let tail = value.get(prefix.len()..)?;
        head.eq_ignore_ascii_case(prefix).then_some(tail)
    }

    #[cfg(windows)]
    fn trim_ascii_whitespace(mut value: &[u8]) -> &[u8] {
        while value.first().is_some_and(|byte| byte.is_ascii_whitespace()) {
            value = &value[1..];
        }

        while value.last().is_some_and(|byte| byte.is_ascii_whitespace()) {
            value = &value[..value.len() - 1];
        }

        value
    }

    fn shell_command(install_dir: &Path) -> String {
        let install_dir = display_path(install_dir);

        if cfg!(windows) {
            let install_dir = install_dir.replace('\'', "''");
            format!(
                r#"powershell -NoProfile -Command "[Environment]::SetEnvironmentVariable('Path', [Environment]::GetEnvironmentVariable('Path', 'User') + ';{install_dir}', 'User')""#
            )
        } else {
            format!(r#"export PATH="{install_dir}:$PATH""#)
        }
    }

    pub(crate) fn status() -> Result<CliInstallStatus, String> {
        let install_dir = install_dir()?;
        let shim_path = shim_path()?;
        let target_path = target_path()?;
        let target_exists = target_path.exists();
        let shim_exists = path_is_present(&shim_path);
        let installed = target_exists && shim_points_to(&shim_path, &target_path)?;
        let conflict = if shim_exists && !installed && !shim_is_cap_managed(&shim_path) {
            Some(format!(
                "{} already exists and is not managed by Scoopy",
                display_path(&shim_path)
            ))
        } else if !target_exists {
            Some(format!(
                "Bundled CLI binary not found at {}",
                display_path(&target_path)
            ))
        } else {
            None
        };

        let on_path = path_contains_install_dir(&install_dir);

        Ok(CliInstallStatus {
            shim_path: display_path(&shim_path),
            target_path: display_path(&target_path),
            installed,
            on_path,
            conflict,
            path_entry: display_path(&install_dir),
            shell_command: shell_command(&install_dir),
            path_configured: path_persisted(&install_dir, on_path),
        })
    }

    #[cfg(unix)]
    fn write_shim(shim_path: &Path, target_path: &Path) -> Result<(), String> {
        std::os::unix::fs::symlink(target_path, shim_path)
            .map_err(|e| format!("Could not create CLI symlink: {e}"))
    }

    #[cfg(windows)]
    fn write_shim(shim_path: &Path, target_path: &Path) -> Result<(), String> {
        let target = windows_command_path(target_path);
        let target =
            windows_env_prefixed_path(&target, |name| env::var(name).ok()).unwrap_or(target);
        let contents = format!(
            r#"@echo off
"{target}" %*
"#
        );
        fs::write(shim_path, contents).map_err(|e| format!("Could not write CLI shim: {e}"))
    }

    pub(crate) fn install() -> Result<CliInstallStatus, String> {
        let install_dir = install_dir()?;
        let shim_path = shim_path()?;
        let target_path = target_path()?;

        if !target_path.exists() {
            return Err(format!(
                "Bundled CLI binary not found at {}",
                display_path(&target_path)
            ));
        }

        fs::create_dir_all(&install_dir)
            .map_err(|e| format!("Could not create CLI directory: {e}"))?;

        if path_is_present(&shim_path) {
            // Repoint our own shim and any other Cap-managed one; only refuse
            // to clobber a genuinely foreign file.
            if !shim_points_to(&shim_path, &target_path)? && !shim_is_cap_managed(&shim_path) {
                return Err(format!(
                    "{} already exists and is not managed by Scoopy",
                    display_path(&shim_path)
                ));
            }

            match fs::remove_file(&shim_path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Could not replace CLI shim: {e}")),
            }
        }

        write_shim(&shim_path, &target_path)?;

        let mut status = status()?;
        if !status.on_path && ensure_path_persisted(&install_dir) {
            status.path_configured = true;
        }
        Ok(status)
    }

    #[cfg(unix)]
    fn ensure_path_persisted(install_dir: &Path) -> bool {
        if env::var_os("CAP_NO_MODIFY_PATH").is_some() {
            return false;
        }
        let Some(home) = dirs::home_dir() else {
            return false;
        };
        let needle = display_path(install_dir);
        if profile_mentions_dir(&home, &needle) {
            return true;
        }
        let shell = env::var("SHELL").unwrap_or_default();
        let shell_name = Path::new(&shell)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        append_path_export(&shell_profile(&home, shell_name), &needle)
    }

    #[cfg(windows)]
    fn ensure_path_persisted(install_dir: &Path) -> bool {
        if env::var_os("CAP_NO_MODIFY_PATH").is_some() {
            return false;
        }
        let dir = display_path(install_dir).replace('\'', "''");
        let script = format!(
            "$d = '{dir}'; \
             $u = [Environment]::GetEnvironmentVariable('Path', 'User'); \
             $e = if ($u) {{ $u -split ';' }} else {{ @() }}; \
             if ($e -notcontains $d) {{ \
             $n = if ($u) {{ \"$d;$u\" }} else {{ $d }}; \
             [Environment]::SetEnvironmentVariable('Path', $n, 'User') }}"
        );
        let mut command = powershell_command();
        command
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    fn path_persisted(install_dir: &Path, on_path: bool) -> bool {
        on_path
            || dirs::home_dir()
                .is_some_and(|home| profile_mentions_dir(&home, &display_path(install_dir)))
    }

    #[cfg(windows)]
    fn path_persisted(install_dir: &Path, on_path: bool) -> bool {
        on_path || windows_user_path_contains(install_dir)
    }

    #[cfg(windows)]
    fn windows_user_path_contains(install_dir: &Path) -> bool {
        let mut command = powershell_command();
        let Ok(output) = command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Environment]::GetEnvironmentVariable('Path', 'User')",
            ])
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        let needle = display_path(install_dir);
        let needle = needle.trim();
        String::from_utf8_lossy(&output.stdout)
            .split(';')
            .any(|entry| entry.trim().eq_ignore_ascii_case(needle))
    }

    #[cfg(windows)]
    fn powershell_command() -> std::process::Command {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let mut command = std::process::Command::new("powershell");
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }

    #[cfg(unix)]
    fn shell_profile(home: &Path, shell_name: &str) -> PathBuf {
        match shell_name {
            "zsh" => home.join(".zshrc"),
            "bash" => home.join(".bashrc"),
            "" if home.join(".zshrc").exists() => home.join(".zshrc"),
            _ => home.join(".profile"),
        }
    }

    #[cfg(unix)]
    fn profile_mentions_dir(home: &Path, needle: &str) -> bool {
        [".zshrc", ".bashrc", ".bash_profile", ".profile"]
            .iter()
            .any(|file| fs::read_to_string(home.join(file)).is_ok_and(|c| c.contains(needle)))
    }

    #[cfg(unix)]
    fn append_path_export(profile: &Path, install_dir: &str) -> bool {
        use std::io::Write;
        let line = format!("export PATH=\"{install_dir}:$PATH\"");
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(profile)
            .and_then(|mut file| writeln!(file, "\n# Added by Scoopy\n{line}"))
            .is_ok()
    }

    pub(crate) fn uninstall() -> Result<CliInstallStatus, String> {
        let shim_path = shim_path()?;
        let target_path = target_path()?;

        if shim_points_to(&shim_path, &target_path)? {
            fs::remove_file(&shim_path).map_err(|e| format!("Could not remove CLI shim: {e}"))?;
        }

        status()
    }

    #[cfg(all(test, unix))]
    mod tests {
        use super::*;

        #[test]
        fn shell_profile_selection() {
            let home = Path::new("/home/u");
            assert_eq!(shell_profile(home, "zsh"), home.join(".zshrc"));
            assert_eq!(shell_profile(home, "bash"), home.join(".bashrc"));
            assert_eq!(shell_profile(home, "fish"), home.join(".profile"));
        }

        #[test]
        fn cap_managed_shim_detection() {
            let dir =
                std::env::temp_dir().join(format!("cap-gpui-cli-test-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            let shim = dir.join(SHIM_NAME);

            // A symlink to a cap-cli binary is Cap-managed even when it points
            // at a different install (the target need not exist).
            std::os::unix::fs::symlink("/elsewhere/Cap.app/Contents/MacOS/cap-cli", &shim).unwrap();
            assert!(shim_is_cap_managed(&shim));

            // A symlink to anything else is not.
            fs::remove_file(&shim).unwrap();
            std::os::unix::fs::symlink("/bin/ls", &shim).unwrap();
            assert!(!shim_is_cap_managed(&shim));

            // Nor is a regular file, so install refuses to clobber it.
            fs::remove_file(&shim).unwrap();
            fs::write(&shim, b"#!/bin/sh\n").unwrap();
            assert!(!shim_is_cap_managed(&shim));

            let _ = fs::remove_dir_all(&dir);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
