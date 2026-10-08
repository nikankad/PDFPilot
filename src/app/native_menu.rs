//! AppKit owns the menu bar; commands return to egui's main-thread update.
use eframe::egui;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem, NSPasteboard, NSPasteboardTypeString,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString};
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    About,
    Open,
    New,
    Images,
    Save,
    SaveCopy,
    Print,
    Close,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    Thumbnails,
    Tools,
    Dark,
    Fullscreen,
    FitWidth,
    FitPage,
    Single,
    Continuous,
    Spread,
    Help,
    Sample,
    Document(usize),
}
impl Command {
    fn tag(self) -> isize {
        match self {
            Self::About => 1,
            Self::Open => 2,
            Self::New => 3,
            Self::Images => 4,
            Self::Save => 5,
            Self::SaveCopy => 6,
            Self::Print => 7,
            Self::Close => 8,
            Self::Quit => 9,
            Self::Undo => 10,
            Self::Redo => 11,
            Self::Cut => 12,
            Self::Copy => 13,
            Self::Paste => 14,
            Self::SelectAll => 15,
            Self::Find => 16,
            Self::Thumbnails => 17,
            Self::Tools => 18,
            Self::Dark => 19,
            Self::Fullscreen => 20,
            Self::FitWidth => 21,
            Self::FitPage => 22,
            Self::Single => 23,
            Self::Continuous => 24,
            Self::Spread => 25,
            Self::Help => 26,
            Self::Sample => 27,
            Self::Document(index) => 1000 + index as isize,
        }
    }
    fn from_tag(tag: isize) -> Option<Self> {
        Some(match tag {
            1 => Self::About,
            2 => Self::Open,
            3 => Self::New,
            4 => Self::Images,
            5 => Self::Save,
            6 => Self::SaveCopy,
            7 => Self::Print,
            8 => Self::Close,
            9 => Self::Quit,
            10 => Self::Undo,
            11 => Self::Redo,
            12 => Self::Cut,
            13 => Self::Copy,
            14 => Self::Paste,
            15 => Self::SelectAll,
            16 => Self::Find,
            17 => Self::Thumbnails,
            18 => Self::Tools,
            19 => Self::Dark,
            20 => Self::Fullscreen,
            21 => Self::FitWidth,
            22 => Self::FitPage,
            23 => Self::Single,
            24 => Self::Continuous,
            25 => Self::Spread,
            26 => Self::Help,
            27 => Self::Sample,
            n if n >= 1000 => Self::Document((n - 1000) as usize),
            _ => return None,
        })
    }
}
struct TargetState {
    sender: Sender<Command>,
    context: egui::Context,
}
define_class!(
    // SAFETY: NSObject has no subclassing requirements. MenuTarget has no Drop
    // implementation and is created and accessed exclusively on the main thread.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetState]
    struct MenuTarget;
    unsafe impl NSObjectProtocol for MenuTarget {}
    impl MenuTarget {
        #[unsafe(method(performPDFPilotCommand:))]
        fn perform(&self,item:&NSMenuItem) {
            if let Some(command)=Command::from_tag(item.tag()) {
                let _=self.ivars().sender.send(command);
                self.ivars().context.request_repaint();
            }
        }
    }
);

pub(super) struct State {
    pub document: bool,
    pub undo: bool,
    pub redo: bool,
    pub editing: bool,
    pub modal: bool,
    pub thumbnails: bool,
    pub tools: bool,
    pub dark: bool,
    pub fullscreen: bool,
    pub view: super::ViewMode,
    pub titles: Vec<String>,
    pub active: Option<usize>,
}
pub(super) struct NativeMenu {
    // AppKit menu-item targets are not retained, so keep the target alive.
    _target: Retained<MenuTarget>,
    _root: Retained<NSMenu>,
    receiver: Receiver<Command>,
    items: Vec<(Command, Retained<NSMenuItem>)>,
    documents: Retained<NSMenu>,
    document_items: Vec<Retained<NSMenuItem>>,
    titles: Vec<String>,
}
impl NativeMenu {
    pub fn install(context: &egui::Context) -> Self {
        let mtm = MainThreadMarker::new().expect("Native menus require the main thread");
        let (sender, receiver) = mpsc::channel();
        let allocated = MenuTarget::alloc(mtm).set_ivars(TargetState {
            sender,
            context: context.clone(),
        });
        // SAFETY: NSObject's init signature is correct for our NSObject subclass.
        let target: Retained<MenuTarget> = unsafe { msg_send![super(allocated), init] };
        let app = NSApplication::sharedApplication(mtm);
        let root = NSMenu::new(mtm);
        let mut items = vec![];
        let app_menu = submenu(&root, "PDFPilot", mtm);
        append(
            &app_menu,
            &target,
            &mut items,
            "About PDFPilot",
            Command::About,
            "",
            false,
            mtm,
        );
        separator(&app_menu, mtm);
        let services = submenu(&app_menu, "Services", mtm);
        services.setAutoenablesItems(true);
        // root retains the Services submenu for the lifetime of the app.
        app.setServicesMenu(Some(&services));
        system(&app_menu, "Hide PDFPilot", sel!(hide:), "h", false, mtm);
        let hide_others = system(
            &app_menu,
            "Hide Others",
            sel!(hideOtherApplications:),
            "h",
            false,
            mtm,
        );
        hide_others.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Option,
        );
        system(
            &app_menu,
            "Show All",
            sel!(unhideAllApplications:),
            "",
            false,
            mtm,
        );
        separator(&app_menu, mtm);
        append(
            &app_menu,
            &target,
            &mut items,
            "Quit PDFPilot",
            Command::Quit,
            "q",
            false,
            mtm,
        );
        let file = submenu(&root, "File", mtm);
        append(
            &file,
            &target,
            &mut items,
            "New PDF…",
            Command::New,
            "n",
            false,
            mtm,
        );
        append(
            &file,
            &target,
            &mut items,
            "Open…",
            Command::Open,
            "o",
            false,
            mtm,
        );
        append(
            &file,
            &target,
            &mut items,
            "PDF from Images…",
            Command::Images,
            "",
            false,
            mtm,
        );
        separator(&file, mtm);
        append(
            &file,
            &target,
            &mut items,
            "Close Document",
            Command::Close,
            "w",
            false,
            mtm,
        );
        append(
            &file,
            &target,
            &mut items,
            "Save",
            Command::Save,
            "s",
            false,
            mtm,
        );
        append(
            &file,
            &target,
            &mut items,
            "Save a Copy…",
            Command::SaveCopy,
            "s",
            true,
            mtm,
        );
        separator(&file, mtm);
        append(
            &file,
            &target,
            &mut items,
            "Print…",
            Command::Print,
            "p",
            false,
            mtm,
        );
        let edit = submenu(&root, "Edit", mtm);
        for (label, command, key, shift) in [
            ("Undo", Command::Undo, "z", false),
            ("Redo", Command::Redo, "z", true),
        ] {
            append(&edit, &target, &mut items, label, command, key, shift, mtm);
        }
        separator(&edit, mtm);
        for (label, command, key) in [
            ("Cut", Command::Cut, "x"),
            ("Copy", Command::Copy, "c"),
            ("Paste", Command::Paste, "v"),
            ("Select All", Command::SelectAll, "a"),
        ] {
            append(&edit, &target, &mut items, label, command, key, false, mtm);
        }
        separator(&edit, mtm);
        append(
            &edit,
            &target,
            &mut items,
            "Find…",
            Command::Find,
            "f",
            false,
            mtm,
        );
        let view = submenu(&root, "View", mtm);
        for (label, command) in [
            ("Page Thumbnails", Command::Thumbnails),
            ("Tools Pane", Command::Tools),
            ("Dark Mode", Command::Dark),
        ] {
            append(&view, &target, &mut items, label, command, "", false, mtm);
        }
        separator(&view, mtm);
        for (label, command) in [
            ("Fit Width", Command::FitWidth),
            ("Fit Page", Command::FitPage),
            ("Single Page", Command::Single),
            ("Continuous", Command::Continuous),
            ("Two Pages", Command::Spread),
        ] {
            append(&view, &target, &mut items, label, command, "", false, mtm);
        }
        separator(&view, mtm);
        append(
            &view,
            &target,
            &mut items,
            "Enter Full Screen",
            Command::Fullscreen,
            "",
            false,
            mtm,
        );
        let window = submenu(&root, "Window", mtm);
        window.setAutoenablesItems(true);
        system(
            &window,
            "Minimize",
            sel!(performMiniaturize:),
            "m",
            false,
            mtm,
        );
        system(&window, "Zoom", sel!(performZoom:), "", false, mtm);
        system(
            &window,
            "Bring All to Front",
            sel!(arrangeInFront:),
            "",
            false,
            mtm,
        );
        separator(&window, mtm);
        let documents = submenu(&window, "Documents", mtm);
        app.setWindowsMenu(Some(&window));
        let help = submenu(&root, "Help", mtm);
        append(
            &help,
            &target,
            &mut items,
            "PDFPilot Help",
            Command::Help,
            "",
            false,
            mtm,
        );
        append(
            &help,
            &target,
            &mut items,
            "Open Sample Document",
            Command::Sample,
            "",
            false,
            mtm,
        );
        app.setHelpMenu(Some(&help));
        app.setMainMenu(Some(&root));
        Self {
            _target: target,
            _root: root,
            receiver,
            items,
            documents,
            document_items: vec![],
            titles: vec![],
        }
    }
    pub fn drain(&self) -> Vec<Command> {
        self.receiver.try_iter().collect()
    }
    pub fn sync(&mut self, state: &State) {
        for (command, item) in &self.items {
            let enabled = match command {
                Command::Undo => state.editing || state.undo,
                Command::Redo => state.editing || state.redo,
                Command::Cut | Command::Copy | Command::Paste | Command::SelectAll => state.editing,
                Command::Save
                | Command::SaveCopy
                | Command::Print
                | Command::Close
                | Command::Find
                | Command::FitWidth
                | Command::FitPage
                | Command::Single
                | Command::Continuous
                | Command::Spread => state.document && !state.modal,
                Command::About
                | Command::New
                | Command::Open
                | Command::Images
                | Command::Help
                | Command::Sample => !state.modal,
                _ => true,
            };
            item.setEnabled(enabled);
            let selected = match command {
                Command::Thumbnails => state.thumbnails,
                Command::Tools => state.tools,
                Command::Dark => state.dark,
                Command::Single => state.document && state.view == super::ViewMode::Single,
                Command::Continuous => state.document && state.view == super::ViewMode::Continuous,
                Command::Spread => state.document && state.view == super::ViewMode::TwoPage,
                _ => false,
            };
            item.setState(if selected { 1 } else { 0 });
            if *command == Command::Fullscreen {
                item.setTitle(&NSString::from_str(if state.fullscreen {
                    "Exit Full Screen"
                } else {
                    "Enter Full Screen"
                }));
            }
        }
        if self.titles != state.titles {
            self.documents.removeAllItems();
            self.document_items.clear();
            let mtm = MainThreadMarker::new().expect("Main thread");
            for (index, title) in state.titles.iter().enumerate() {
                let mut added = vec![];
                append(
                    &self.documents,
                    &self._target,
                    &mut added,
                    title,
                    Command::Document(index),
                    "",
                    false,
                    mtm,
                );
                self.document_items.push(added.pop().unwrap().1);
            }
            if state.titles.is_empty() {
                let item = NSMenuItem::new(mtm);
                item.setTitle(&NSString::from_str("No Documents Open"));
                item.setEnabled(false);
                self.documents.addItem(&item);
            }
            self.titles = state.titles.clone();
        }
        for (index, item) in self.document_items.iter().enumerate() {
            item.setState(if state.active == Some(index) { 1 } else { 0 });
            item.setEnabled(!state.modal);
        }
    }
}
impl Drop for NativeMenu {
    fn drop(&mut self) {
        let mtm = MainThreadMarker::new().expect("Native menus drop on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        if app
            .mainMenu()
            .is_some_and(|menu| std::ptr::eq(&*menu, &*self._root))
        {
            app.setMainMenu(None);
            app.setServicesMenu(None);
            app.setHelpMenu(None);
            app.setWindowsMenu(None);
        }
    }
}

fn submenu(parent: &NSMenu, title: &str, mtm: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(title));
    menu.setAutoenablesItems(false);
    let item = NSMenuItem::new(mtm);
    item.setTitle(&NSString::from_str(title));
    item.setSubmenu(Some(&menu));
    parent.addItem(&item);
    menu
}
fn separator(menu: &NSMenu, mtm: MainThreadMarker) {
    menu.addItem(&NSMenuItem::separatorItem(mtm));
}
#[allow(clippy::too_many_arguments)]
fn append(
    menu: &NSMenu,
    target: &MenuTarget,
    items: &mut Vec<(Command, Retained<NSMenuItem>)>,
    title: &str,
    command: Command,
    key: &str,
    shift: bool,
    mtm: MainThreadMarker,
) {
    // SAFETY: the selector's method takes an NSMenuItem and returns void. The
    // NativeMenu retains target until the application has finished running.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            Some(sel!(performPDFPilotCommand:)),
            &NSString::from_str(key),
        )
    };
    unsafe {
        item.setTarget(Some(target));
    }
    item.setTag(command.tag());
    item.setKeyEquivalentModifierMask(
        NSEventModifierFlags::Command
            | if shift {
                NSEventModifierFlags::Shift
            } else {
                NSEventModifierFlags::empty()
            },
    );
    menu.addItem(&item);
    items.push((command, item));
}
fn system(
    menu: &NSMenu,
    title: &str,
    selector: Sel,
    key: &str,
    shift: bool,
    mtm: MainThreadMarker,
) -> Retained<NSMenuItem> {
    // SAFETY: these are standard AppKit actions resolved through its responder chain.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            Some(selector),
            &NSString::from_str(key),
        )
    };
    item.setKeyEquivalentModifierMask(
        NSEventModifierFlags::Command
            | if shift {
                NSEventModifierFlags::Shift
            } else {
                NSEventModifierFlags::empty()
            },
    );
    menu.addItem(&item);
    item
}
pub(super) fn pasted_text() -> Option<String> {
    // SAFETY: this constant is initialized by AppKit; the returned string is
    // immediately copied into a Rust String on the application main thread.
    unsafe {
        NSPasteboard::generalPasteboard()
            .stringForType(NSPasteboardTypeString)
            .map(|s| s.to_string())
    }
}

impl super::PdfApp {
    pub(super) fn poll_native_menu(&mut self, ctx: &egui::Context) {
        let commands = self
            .native_menu
            .as_ref()
            .map(NativeMenu::drain)
            .unwrap_or_default();
        for command in commands {
            use super::{Action, Modal, ViewMode};
            let editing = ctx.wants_keyboard_input();
            let action = match command {
                Command::Open => Some(Action::Open),
                Command::New => Some(Action::New),
                Command::Images => Some(Action::Images),
                Command::Save => Some(Action::Save(false)),
                Command::SaveCopy => Some(Action::Save(true)),
                Command::Print => Some(Action::Print),
                Command::Close => self.active.map(Action::Close),
                Command::Quit => Some(Action::Quit),
                Command::Sample => Some(Action::Sample),
                Command::Undo | Command::Redo if editing => {
                    inject_key(ctx, egui::Key::Z, command == Command::Redo);
                    None
                }
                Command::Undo => Some(Action::Undo),
                Command::Redo => Some(Action::Redo),
                Command::Cut => {
                    ctx.input_mut(|i| i.events.push(egui::Event::Cut));
                    None
                }
                Command::Copy => {
                    ctx.input_mut(|i| i.events.push(egui::Event::Copy));
                    None
                }
                Command::Paste => {
                    if let Some(text) = pasted_text() {
                        ctx.input_mut(|i| i.events.push(egui::Event::Paste(text)));
                    }
                    None
                }
                Command::SelectAll => {
                    inject_key(ctx, egui::Key::A, false);
                    None
                }
                Command::Find => {
                    self.search_open = true;
                    ctx.memory_mut(|m| m.request_focus(egui::Id::new("find_input")));
                    None
                }
                Command::Thumbnails => {
                    self.thumbnails_open = !self.thumbnails_open;
                    None
                }
                Command::Tools => {
                    self.tools_open = !self.tools_open;
                    None
                }
                Command::Dark => {
                    self.dark_mode = !self.dark_mode;
                    crate::theme::apply(ctx, self.dark_mode);
                    None
                }
                Command::Fullscreen => {
                    self.fullscreen = !self.fullscreen;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                    None
                }
                Command::FitWidth => Some(Action::Fit),
                Command::FitPage => Some(Action::FitPage),
                Command::Single | Command::Continuous | Command::Spread => {
                    if let Some(s) = self.current_mut() {
                        s.view = match command {
                            Command::Single => ViewMode::Single,
                            Command::Continuous => ViewMode::Continuous,
                            _ => ViewMode::TwoPage,
                        };
                        self.jump_page = true;
                    }
                    None
                }
                Command::About | Command::Help => {
                    self.modal = Some(Modal::Help);
                    None
                }
                Command::Document(index) => {
                    if index < self.sessions.len() {
                        Some(Action::Tab(index))
                    } else {
                        None
                    }
                }
            };
            if let Some(action) = action {
                self.execute(action, ctx);
            }
        }
    }
    pub(super) fn sync_native_menu(&mut self, ctx: &egui::Context) {
        let state = State {
            document: self.active.is_some(),
            undo: self.current().is_some_and(|s| s.pdf.can_undo()),
            redo: self.current().is_some_and(|s| s.pdf.can_redo()),
            editing: ctx.wants_keyboard_input(),
            modal: self.modal.is_some(),
            thumbnails: self.thumbnails_open,
            tools: self.tools_open,
            dark: self.dark_mode,
            fullscreen: self.fullscreen,
            view: self
                .current()
                .map(|s| s.view)
                .unwrap_or(super::ViewMode::Single),
            titles: self
                .sessions
                .iter()
                .map(|s| format!("{}{}", s.pdf.title, if s.pdf.dirty { " *" } else { "" }))
                .collect(),
            active: self.active,
        };
        if let Some(menu) = &mut self.native_menu {
            menu.sync(&state);
        }
    }
}
fn inject_key(ctx: &egui::Context, key: egui::Key, shift: bool) {
    ctx.input_mut(|i| {
        i.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                mac_cmd: true,
                command: true,
                shift,
                ..Default::default()
            },
        })
    });
}
