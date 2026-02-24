//! Interactive TUI for battery-pack CLI.

use crate::{
    BatteryPackDetail, BatteryPackSummary, InstalledPack, fetch_battery_pack_detail,
    fetch_battery_pack_list, load_installed_packs,
};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::rc::Rc;
use std::time::Duration;

// ============================================================================
// Public entry points
// ============================================================================

/// Run the TUI starting from the list view
pub fn run_list(filter: Option<String>) -> Result<()> {
    let app = App::new_list(filter);
    app.run()
}

/// Run the TUI starting from the detail view
pub fn run_show(name: &str, path: Option<&str>) -> Result<()> {
    let app = App::new_show(name, path);
    app.run()
}

/// Run the TUI for interactive dependency management
pub fn run_add() -> Result<()> {
    let app = App::new_add();
    app.run()
}

// ============================================================================
// App state
// ============================================================================

struct App {
    screen: Screen,
    should_quit: bool,
    pending_action: Option<PendingAction>,
}

/// A single crate being added or updated.
struct CrateChange {
    name: String,
    version: String,
    features: Vec<String>,
}

impl From<&CrateEntry> for CrateChange {
    fn from(entry: &CrateEntry) -> Self {
        Self {
            name: entry.name.clone(),
            version: entry.version.clone(),
            features: entry.features.clone(),
        }
    }
}

/// A change to apply from the Add TUI.
enum AddChange {
    /// Add a new battery pack with selected crates.
    AddPack {
        name: String,
        crates: Vec<CrateChange>,
    },
    /// Update an existing pack's enabled crates.
    UpdatePack {
        name: String,
        /// Crates to add (weren't enabled before, now are).
        add_crates: Vec<CrateChange>,
        /// Crates to remove (were enabled before, now aren't).
        remove_crates: Vec<String>,
    },
}

enum Screen {
    Loading(LoadingState),
    List(ListScreen),
    Detail(DetailScreen),
    NewProjectForm(FormScreen),
    Add(AddScreen),
}

struct LoadingState {
    message: String,
    target: LoadingTarget,
}

enum LoadingTarget {
    List {
        filter: Option<String>,
    },
    Detail {
        name: String,
        path: Option<String>,
        came_from_list: bool,
    },
    Add,
    /// Fetch the browse list for the Add screen (initial load or search).
    BrowseList {
        add_screen: AddScreen,
        filter: Option<String>,
    },
    /// Expand a battery pack from the browse list.
    BrowseExpand {
        add_screen: AddScreen,
        bp_name: String,
        bp_short_name: String,
    },
}

struct ListScreen {
    items: Vec<BatteryPackSummary>,
    list_state: ListState,
    filter: Option<String>,
}

struct DetailScreen {
    detail: Rc<BatteryPackDetail>,
    /// Index into selectable_items()
    selected_index: usize,
    came_from_list: bool,
}

/// A selectable item in the detail view
#[derive(Clone, Debug)]
enum DetailItem {
    /// A crate dependency - opens crates.io
    Crate(String),
    /// An extended battery pack - opens crates.io
    Extends(String),
    /// A template - opens GitHub tree URL (stores local path and resolved repo path)
    Template {
        _path: String,
        repo_path: Option<String>,
    },
    /// An example - opens GitHub blob URL (stores name and resolved repo path)
    Example {
        _name: String,
        repo_path: Option<String>,
    },
    /// Open on crates.io action
    ActionOpenCratesIo,
    /// Add to project action
    ActionAddToProject,
    /// Create new project action
    ActionNewProject,
}

impl DetailScreen {
    /// Get the total count of selectable items without building the full vector.
    /// This is more efficient for navigation operations that only need the count.
    fn item_count(&self) -> usize {
        self.detail.crates.len()
            + self.detail.extends.len()
            + self.detail.templates.len()
            + self.detail.examples.len()
            + 3 // actions: OpenCratesIo, AddToProject, NewProject
    }

    /// Build a list of all selectable items in order
    fn selectable_items(&self) -> Vec<DetailItem> {
        let mut items = Vec::with_capacity(self.item_count());

        // Crates
        for crate_name in &self.detail.crates {
            items.push(DetailItem::Crate(crate_name.clone()));
        }

        // Extends (other battery packs)
        for extends in &self.detail.extends {
            items.push(DetailItem::Extends(extends.clone()));
        }

        // Templates
        for tmpl in &self.detail.templates {
            items.push(DetailItem::Template {
                _path: tmpl.path.clone(),
                repo_path: tmpl.repo_path.clone(),
            });
        }

        // Examples
        for example in &self.detail.examples {
            items.push(DetailItem::Example {
                _name: example.name.clone(),
                repo_path: example.repo_path.clone(),
            });
        }

        // Actions (always present)
        items.push(DetailItem::ActionOpenCratesIo);
        items.push(DetailItem::ActionAddToProject);
        items.push(DetailItem::ActionNewProject);

        items
    }

    fn selected_item(&self) -> Option<DetailItem> {
        self.selectable_items().get(self.selected_index).cloned()
    }

    fn select_next(&mut self) {
        let count = self.item_count();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    fn select_prev(&mut self) {
        let count = self.item_count();
        if count > 0 {
            self.selected_index = (self.selected_index + count - 1) % count;
        }
    }
}

struct FormScreen {
    battery_pack: String,
    /// If set, use this specific template (passed via --template)
    template: Option<String>,
    directory: String,
    project_name: String,
    focused_field: FormField,
    cursor_position: usize,
    /// The detail screen to return to on cancel (shared to avoid cloning)
    detail: Rc<BatteryPackDetail>,
    /// Selected index to restore when returning to detail
    selected_index: usize,
    came_from_list: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum FormField {
    Directory,
    ProjectName,
}

// ============================================================================
// Add screen (interactive dependency manager)
// ============================================================================

struct AddScreen {
    tab: AddTab,
    installed: InstalledState,
    browse: BrowseState,
    /// Changes to apply after the TUI exits.
    changes: Option<Vec<AddChange>>,
}

#[derive(Clone, Copy, PartialEq)]
enum AddTab {
    Installed,
    Browse,
}

struct InstalledState {
    packs: Vec<InstalledPackState>,
    /// Flat index across all selectable rows (crate entries) in all packs.
    selected_index: usize,
}

struct InstalledPackState {
    name: String,
    short_name: String,
    version: String,
    entries: Vec<CrateEntry>,
}

#[derive(Clone)]
struct CrateEntry {
    name: String,
    version: String,
    features: Vec<String>,
    group: String,
    enabled: bool,
    originally_enabled: bool,
}

impl CrateEntry {
    fn version_info(&self) -> String {
        if self.features.is_empty() {
            format!("({})", self.version)
        } else {
            format!("({}, features: {})", self.version, self.features.join(", "))
        }
    }
}

struct BrowseState {
    items: Vec<BatteryPackSummary>,
    list_state: ListState,
    search_input: String,
    searching: bool,
    /// When a pack is expanded, its crate picker state
    expanded: Option<ExpandedPack>,
}

struct ExpandedPack {
    pack: InstalledPackState,
    selected_index: usize,
}

impl InstalledState {
    /// Total number of selectable crate rows across all packs.
    fn total_entries(&self) -> usize {
        self.packs.iter().map(|p| p.entries.len()).sum()
    }

    fn select_next(&mut self) {
        let total = self.total_entries();
        if total > 0 {
            self.selected_index = (self.selected_index + 1) % total;
        }
    }

    fn select_prev(&mut self) {
        let total = self.total_entries();
        if total > 0 {
            self.selected_index = (self.selected_index + total - 1) % total;
        }
    }

    /// Toggle the currently selected crate's enabled state.
    fn toggle_selected(&mut self) {
        let mut idx = 0;
        for pack in &mut self.packs {
            for entry in &mut pack.entries {
                if idx == self.selected_index {
                    entry.enabled = !entry.enabled;
                    return;
                }
                idx += 1;
            }
        }
    }

    /// Returns true if any crate's enabled state differs from its original.
    fn has_changes(&self) -> bool {
        self.packs
            .iter()
            .flat_map(|p| &p.entries)
            .any(|e| e.enabled != e.originally_enabled)
    }

    /// Returns true if any pack was added from Browse (all entries have originally_enabled=false).
    fn has_new_packs(&self) -> bool {
        self.packs.iter().any(|p| {
            p.entries.iter().all(|e| !e.originally_enabled) && p.entries.iter().any(|e| e.enabled)
        })
    }

    /// Collect all changes into AddChange items for processing.
    fn collect_changes(&self) -> Vec<AddChange> {
        let mut changes = Vec::new();

        for pack in &self.packs {
            let is_new = pack.entries.iter().all(|e| !e.originally_enabled);
            let has_enabled = pack.entries.iter().any(|e| e.enabled);

            if is_new && has_enabled {
                // New pack from Browse
                let crates = pack
                    .entries
                    .iter()
                    .filter(|e| e.enabled)
                    .map(CrateChange::from)
                    .collect();
                changes.push(AddChange::AddPack {
                    name: pack.name.clone(),
                    crates,
                });
            } else {
                // Existing pack — check for toggles
                let add_crates: Vec<_> = pack
                    .entries
                    .iter()
                    .filter(|e| e.enabled && !e.originally_enabled)
                    .map(CrateChange::from)
                    .collect();
                let remove_crates: Vec<_> = pack
                    .entries
                    .iter()
                    .filter(|e| !e.enabled && e.originally_enabled)
                    .map(|e| e.name.clone())
                    .collect();

                if !add_crates.is_empty() || !remove_crates.is_empty() {
                    changes.push(AddChange::UpdatePack {
                        name: pack.name.clone(),
                        add_crates,
                        remove_crates,
                    });
                }
            }
        }

        changes
    }
}

impl ExpandedPack {
    fn select_next(&mut self) {
        let len = self.pack.entries.len();
        if len > 0 {
            self.selected_index = (self.selected_index + 1) % len;
        }
    }

    fn select_prev(&mut self) {
        let len = self.pack.entries.len();
        if len > 0 {
            self.selected_index = (self.selected_index + len - 1) % len;
        }
    }

    fn toggle_selected(&mut self) {
        if let Some(entry) = self.pack.entries.get_mut(self.selected_index) {
            entry.enabled = !entry.enabled;
        }
    }
}

/// Build InstalledState from loaded packs.
fn build_installed_state(packs: Vec<InstalledPack>) -> InstalledState {
    let pack_states = packs
        .into_iter()
        .map(|pack| {
            let grouped = pack.spec.all_crates_with_grouping();
            let resolved = if pack.active_sets.iter().any(|s| s == "all") {
                pack.spec.resolve_all()
            } else {
                let str_sets: Vec<&str> = pack.active_sets.iter().map(|s| s.as_str()).collect();
                pack.spec.resolve_crates(&str_sets)
            };

            let entries = grouped
                .into_iter()
                .map(|(group, crate_name, dep, _is_default)| {
                    let is_enabled = resolved.contains_key(&crate_name);
                    CrateEntry {
                        name: crate_name,
                        version: dep.version.clone(),
                        features: dep.features.clone(),
                        group,
                        enabled: is_enabled,
                        originally_enabled: is_enabled,
                    }
                })
                .collect();

            InstalledPackState {
                name: pack.name,
                short_name: pack.short_name,
                version: pack.version,
                entries,
            }
        })
        .collect();

    InstalledState {
        packs: pack_states,
        selected_index: 0,
    }
}

/// Build an ExpandedPack from a battery pack fetched from crates.io.
fn build_expanded_pack(
    detail: &BatteryPackSummary,
    spec: bphelper_manifest::BatteryPackSpec,
) -> ExpandedPack {
    let grouped = spec.all_crates_with_grouping();
    let entries = grouped
        .into_iter()
        .map(|(group, crate_name, dep, is_default)| CrateEntry {
            name: crate_name,
            version: dep.version.clone(),
            features: dep.features.clone(),
            group,
            enabled: is_default,
            originally_enabled: false, // new pack, nothing was originally enabled
        })
        .collect();

    ExpandedPack {
        pack: InstalledPackState {
            name: detail.name.clone(),
            short_name: detail.short_name.clone(),
            version: detail.version.clone(),
            entries,
        },
        selected_index: 0,
    }
}

enum PendingAction {
    /// Open a URL in the browser (generic)
    OpenUrl {
        url: String,
    },
    /// Open crates.io for the battery pack
    OpenCratesIo {
        crate_name: String,
    },
    AddToProject {
        battery_pack: String,
    },
    NewProject {
        battery_pack: String,
        template: Option<String>,
        directory: String,
        name: String,
    },
}

// ============================================================================
// App implementation
// ============================================================================

impl App {
    fn new_list(filter: Option<String>) -> Self {
        Self {
            screen: Screen::Loading(LoadingState {
                message: "Loading battery packs...".to_string(),
                target: LoadingTarget::List { filter },
            }),
            should_quit: false,
            pending_action: None,
        }
    }

    fn new_add() -> Self {
        Self {
            screen: Screen::Loading(LoadingState {
                message: "Loading installed battery packs...".to_string(),
                target: LoadingTarget::Add,
            }),
            should_quit: false,
            pending_action: None,
        }
    }

    fn new_show(name: &str, path: Option<&str>) -> Self {
        Self {
            screen: Screen::Loading(LoadingState {
                message: format!("Loading {}...", name),
                target: LoadingTarget::Detail {
                    name: name.to_string(),
                    path: path.map(|s| s.to_string()),
                    came_from_list: false,
                },
            }),
            should_quit: false,
            pending_action: None,
        }
    }

    fn run(mut self) -> Result<()> {
        let mut terminal = ratatui::init();

        // Process initial loading state
        self.process_loading()?;

        loop {
            terminal.draw(|frame| self.render(frame))?;

            // Execute pending actions (exit TUI, run command, re-enter)
            if let Some(action) = self.pending_action.take() {
                ratatui::restore();
                self.execute_action(&action)?;
                terminal = ratatui::init();
                continue;
            }

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    // Windows compatibility: only handle Press events
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code);
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        ratatui::restore();

        // Apply any pending add changes after TUI exits
        if let Screen::Add(add_screen) = &mut self.screen {
            if let Some(changes) = add_screen.changes.take() {
                apply_add_changes(&changes)?;
            }
        }

        Ok(())
    }

    fn process_loading(&mut self) -> Result<()> {
        // Take ownership of the screen so we can move data out of LoadingTarget variants.
        let screen = std::mem::replace(
            &mut self.screen,
            Screen::Loading(LoadingState {
                message: String::new(),
                target: LoadingTarget::Add, // placeholder, will be overwritten
            }),
        );

        let Screen::Loading(state) = screen else {
            self.screen = screen;
            return Ok(());
        };

        match state.target {
            LoadingTarget::List { filter } => {
                let items = fetch_battery_pack_list(filter.as_deref())?;
                let mut list_state = ListState::default();
                if !items.is_empty() {
                    list_state.select(Some(0));
                }
                self.screen = Screen::List(ListScreen {
                    items,
                    list_state,
                    filter,
                });
            }
            LoadingTarget::Detail {
                name,
                path,
                came_from_list,
            } => {
                let detail = fetch_battery_pack_detail(&name, path.as_deref())?;
                let initial_index = detail.crates.len()
                    + detail.extends.len()
                    + detail.templates.len()
                    + detail.examples.len();
                self.screen = Screen::Detail(DetailScreen {
                    detail: Rc::new(detail),
                    selected_index: initial_index,
                    came_from_list,
                });
            }
            LoadingTarget::Add => {
                let packs = load_installed_packs()?;
                let installed = build_installed_state(packs);
                self.screen = Screen::Add(AddScreen {
                    tab: AddTab::Installed,
                    installed,
                    browse: BrowseState {
                        items: Vec::new(),
                        list_state: ListState::default(),
                        search_input: String::new(),
                        searching: false,
                        expanded: None,
                    },
                    changes: None,
                });
            }
            LoadingTarget::BrowseList {
                mut add_screen,
                filter,
            } => {
                let items = fetch_battery_pack_list(filter.as_deref())?;
                let has_items = !items.is_empty();
                add_screen.browse.items = items;
                add_screen.browse.list_state = ListState::default();
                if has_items {
                    add_screen.browse.list_state.select(Some(0));
                }
                add_screen.tab = AddTab::Browse;
                self.screen = Screen::Add(add_screen);
            }
            LoadingTarget::BrowseExpand {
                mut add_screen,
                bp_name,
                bp_short_name,
            } => {
                let (_version, spec) = crate::fetch_bp_spec_from_registry(&bp_name)?;
                let summary = BatteryPackSummary {
                    name: bp_name,
                    short_name: bp_short_name,
                    version: spec.version.clone(),
                    description: String::new(),
                };
                add_screen.browse.expanded = Some(build_expanded_pack(&summary, spec));
                self.screen = Screen::Add(add_screen);
            }
        }
        Ok(())
    }

    fn execute_action(&self, action: &PendingAction) -> Result<()> {
        match action {
            PendingAction::OpenUrl { url } => {
                if let Err(e) = open::that(url) {
                    println!("Failed to open browser: {}", e);
                    println!("URL: {}", url);
                    println!("\nPress Enter to return to TUI...");
                    let _ = std::io::stdin().read_line(&mut String::new());
                }
                // No "press enter" for successful open - just return immediately
            }
            PendingAction::OpenCratesIo { crate_name } => {
                let url = format!("https://crates.io/crates/{}", crate_name);
                if let Err(e) = open::that(&url) {
                    println!("Failed to open browser: {}", e);
                    println!("URL: {}", url);
                    println!("\nPress Enter to return to TUI...");
                    let _ = std::io::stdin().read_line(&mut String::new());
                }
                // No "press enter" for successful open - just return immediately
            }
            PendingAction::AddToProject { battery_pack } => {
                let status = std::process::Command::new("cargo")
                    .args(["bp", "add", battery_pack])
                    .status()?;

                if status.success() {
                    println!("\nSuccessfully added {}!", battery_pack);
                }
                println!("\nPress Enter to return to TUI...");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            PendingAction::NewProject {
                battery_pack,
                template,
                directory,
                name,
            } => {
                let mut cmd = std::process::Command::new("cargo");
                cmd.args(["bp", "new", battery_pack, "-n", name]);
                if let Some(tmpl) = template {
                    cmd.args(["-t", tmpl]);
                }
                let status = cmd.current_dir(directory).status()?;

                if status.success() {
                    println!("\nSuccessfully created project '{}'!", name);
                }
                println!("\nPress Enter to return to TUI...");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyCode) {
        // Extract needed data to avoid borrow conflicts
        enum Action {
            None,
            Quit,
            ListSelect(usize),
            ListUp,
            ListDown,
            DetailNext,
            DetailPrev,
            OpenCratesIoUrl(String),
            OpenTemplate {
                repository: Option<String>,
                repo_path: Option<String>,
            },
            OpenExample {
                repository: Option<String>,
                repo_path: Option<String>,
            },
            DetailOpenCratesIo(String),
            DetailAdd(String),
            DetailNewProject(Rc<BatteryPackDetail>, Option<String>, usize, bool),
            DetailBack(bool),
            FormToggleField,
            FormSubmit(
                String,
                Option<String>,
                String,
                String,
                Rc<BatteryPackDetail>,
                usize,
                bool,
            ),
            FormCancel(Rc<BatteryPackDetail>, usize, bool),
            FormChar(char),
            FormBackspace,
            FormDelete,
            FormLeft,
            FormRight,
            FormHome,
            FormEnd,
        }

        let action = match &self.screen {
            Screen::Loading(_) => Action::None,
            Screen::List(state) => match key {
                KeyCode::Up | KeyCode::Char('k') => Action::ListUp,
                KeyCode::Down | KeyCode::Char('j') => Action::ListDown,
                KeyCode::Enter => {
                    if let Some(selected) = state.list_state.selected() {
                        Action::ListSelect(selected)
                    } else {
                        Action::None
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
                _ => Action::None,
            },
            Screen::Detail(state) => match key {
                KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => Action::DetailNext,
                KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => Action::DetailPrev,
                KeyCode::Enter => {
                    if let Some(item) = state.selected_item() {
                        match item {
                            DetailItem::Crate(crate_name) => Action::OpenCratesIoUrl(crate_name),
                            DetailItem::Extends(bp_name) => {
                                // Extends are battery packs, resolve to full name
                                let full_name = if bp_name.ends_with("-battery-pack") {
                                    bp_name
                                } else {
                                    format!("{}-battery-pack", bp_name)
                                };
                                Action::OpenCratesIoUrl(full_name)
                            }
                            DetailItem::Template {
                                _path: _,
                                repo_path,
                            } => Action::OpenTemplate {
                                repository: state.detail.repository.clone(),
                                repo_path,
                            },
                            DetailItem::Example {
                                _name: _,
                                repo_path,
                            } => Action::OpenExample {
                                repository: state.detail.repository.clone(),
                                repo_path,
                            },
                            DetailItem::ActionOpenCratesIo => {
                                Action::DetailOpenCratesIo(state.detail.name.clone())
                            }
                            DetailItem::ActionAddToProject => {
                                Action::DetailAdd(state.detail.short_name.clone())
                            }
                            DetailItem::ActionNewProject => {
                                Action::DetailNewProject(
                                    Rc::clone(&state.detail),
                                    None, // no specific template
                                    state.selected_index,
                                    state.came_from_list,
                                )
                            }
                        }
                    } else {
                        Action::None
                    }
                }
                KeyCode::Char('n') => {
                    // 'n' creates new project with currently selected template (if a template is selected)
                    if let Some(DetailItem::Template {
                        _path,
                        repo_path: _,
                    }) = state.selected_item()
                    {
                        // Find the template name from the path
                        let template_name = state
                            .detail
                            .templates
                            .iter()
                            .find(|t| t.path == _path)
                            .map(|t| t.name.clone());
                        Action::DetailNewProject(
                            Rc::clone(&state.detail),
                            template_name,
                            state.selected_index,
                            state.came_from_list,
                        )
                    } else {
                        Action::None
                    }
                }
                KeyCode::Esc => Action::DetailBack(state.came_from_list),
                KeyCode::Char('q') => Action::Quit,
                _ => Action::None,
            },
            Screen::NewProjectForm(state) => match key {
                KeyCode::Tab => Action::FormToggleField,
                KeyCode::Enter => {
                    if !state.project_name.is_empty() {
                        Action::FormSubmit(
                            state.battery_pack.clone(),
                            state.template.clone(),
                            state.directory.clone(),
                            state.project_name.clone(),
                            Rc::clone(&state.detail),
                            state.selected_index,
                            state.came_from_list,
                        )
                    } else {
                        Action::None
                    }
                }
                KeyCode::Esc => Action::FormCancel(
                    Rc::clone(&state.detail),
                    state.selected_index,
                    state.came_from_list,
                ),
                KeyCode::Char(c) => Action::FormChar(c),
                KeyCode::Backspace => Action::FormBackspace,
                KeyCode::Delete => Action::FormDelete,
                KeyCode::Left => Action::FormLeft,
                KeyCode::Right => Action::FormRight,
                KeyCode::Home => Action::FormHome,
                KeyCode::End => Action::FormEnd,
                _ => Action::None,
            },
            Screen::Add(_) => {
                // Add screen handles its own state directly to avoid
                // threading complex state through the Action enum.
                self.handle_add_key(key);
                return;
            }
        };

        // Now apply the action with full mutable access
        match action {
            Action::None => {}
            Action::Quit => self.should_quit = true,
            Action::ListUp => {
                if let Screen::List(state) = &mut self.screen {
                    if let Some(selected) = state.list_state.selected() {
                        if selected > 0 {
                            state.list_state.select(Some(selected - 1));
                        }
                    }
                }
            }
            Action::ListDown => {
                if let Screen::List(state) = &mut self.screen {
                    if let Some(selected) = state.list_state.selected() {
                        if selected < state.items.len().saturating_sub(1) {
                            state.list_state.select(Some(selected + 1));
                        }
                    }
                }
            }
            Action::ListSelect(selected) => {
                if let Screen::List(state) = &self.screen {
                    if let Some(bp) = state.items.get(selected) {
                        self.screen = Screen::Loading(LoadingState {
                            message: format!("Loading {}...", bp.short_name),
                            target: LoadingTarget::Detail {
                                name: bp.name.clone(),
                                path: None,
                                came_from_list: true,
                            },
                        });
                        let _ = self.process_loading();
                    }
                }
            }
            Action::DetailNext => {
                if let Screen::Detail(state) = &mut self.screen {
                    state.select_next();
                }
            }
            Action::DetailPrev => {
                if let Screen::Detail(state) = &mut self.screen {
                    state.select_prev();
                }
            }
            Action::OpenCratesIoUrl(crate_name) => {
                let url = format!("https://crates.io/crates/{}", crate_name);
                self.pending_action = Some(PendingAction::OpenUrl { url });
            }
            Action::OpenTemplate {
                repository,
                repo_path,
            } => {
                let url = match repo_path {
                    Some(path) => build_github_url(repository.as_deref(), &path),
                    None => repository.unwrap_or_else(|| "https://crates.io".to_string()),
                };
                self.pending_action = Some(PendingAction::OpenUrl { url });
            }
            Action::OpenExample {
                repository,
                repo_path,
            } => {
                let url = match repo_path {
                    Some(path) => build_github_blob_url(repository.as_deref(), &path),
                    None => repository.unwrap_or_else(|| "https://crates.io".to_string()),
                };
                self.pending_action = Some(PendingAction::OpenUrl { url });
            }
            Action::DetailOpenCratesIo(crate_name) => {
                self.pending_action = Some(PendingAction::OpenCratesIo { crate_name });
            }
            Action::DetailAdd(battery_pack) => {
                self.pending_action = Some(PendingAction::AddToProject { battery_pack });
            }
            Action::DetailNewProject(detail, template, selected_index, came_from_list) => {
                let cwd = std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| ".".to_string());
                self.screen = Screen::NewProjectForm(FormScreen {
                    battery_pack: detail.short_name.clone(),
                    template,
                    directory: cwd,
                    project_name: String::new(),
                    focused_field: FormField::ProjectName,
                    cursor_position: 0,
                    detail,
                    selected_index,
                    came_from_list,
                });
            }
            Action::DetailBack(came_from_list) => {
                if came_from_list {
                    self.screen = Screen::Loading(LoadingState {
                        message: "Loading battery packs...".to_string(),
                        target: LoadingTarget::List { filter: None },
                    });
                    let _ = self.process_loading();
                } else {
                    self.should_quit = true;
                }
            }
            Action::FormToggleField => {
                if let Screen::NewProjectForm(state) = &mut self.screen {
                    state.focused_field = match state.focused_field {
                        FormField::Directory => FormField::ProjectName,
                        FormField::ProjectName => FormField::Directory,
                    };
                    state.cursor_position = match state.focused_field {
                        FormField::Directory => state.directory.len(),
                        FormField::ProjectName => state.project_name.len(),
                    };
                }
            }
            Action::FormSubmit(
                battery_pack,
                template,
                directory,
                name,
                detail,
                selected_index,
                came_from_list,
            ) => {
                self.pending_action = Some(PendingAction::NewProject {
                    battery_pack,
                    template,
                    directory,
                    name,
                });
                self.screen = Screen::Detail(DetailScreen {
                    detail,
                    selected_index,
                    came_from_list,
                });
            }
            Action::FormCancel(detail, selected_index, came_from_list) => {
                self.screen = Screen::Detail(DetailScreen {
                    detail,
                    selected_index,
                    came_from_list,
                });
            }
            Action::FormChar(c) => {
                if let Screen::NewProjectForm(state) = &mut self.screen {
                    let field = match state.focused_field {
                        FormField::Directory => &mut state.directory,
                        FormField::ProjectName => &mut state.project_name,
                    };
                    field.insert(state.cursor_position, c);
                    state.cursor_position += 1;
                }
            }
            Action::FormBackspace => {
                if let Screen::NewProjectForm(state) = &mut self.screen {
                    if state.cursor_position > 0 {
                        let field = match state.focused_field {
                            FormField::Directory => &mut state.directory,
                            FormField::ProjectName => &mut state.project_name,
                        };
                        field.remove(state.cursor_position - 1);
                        state.cursor_position -= 1;
                    }
                }
            }
            Action::FormDelete => {
                if let Screen::NewProjectForm(state) = &mut self.screen {
                    let field = match state.focused_field {
                        FormField::Directory => &mut state.directory,
                        FormField::ProjectName => &mut state.project_name,
                    };
                    if state.cursor_position < field.len() {
                        field.remove(state.cursor_position);
                    }
                }
            }
            Action::FormLeft => {
                if let Screen::NewProjectForm(state) = &mut self.screen {
                    state.cursor_position = state.cursor_position.saturating_sub(1);
                }
            }
            Action::FormRight => {
                if let Screen::NewProjectForm(state) = &mut self.screen {
                    let field_len = match state.focused_field {
                        FormField::Directory => state.directory.len(),
                        FormField::ProjectName => state.project_name.len(),
                    };
                    if state.cursor_position < field_len {
                        state.cursor_position += 1;
                    }
                }
            }
            Action::FormHome => {
                if let Screen::NewProjectForm(state) = &mut self.screen {
                    state.cursor_position = 0;
                }
            }
            Action::FormEnd => {
                if let Screen::NewProjectForm(state) = &mut self.screen {
                    state.cursor_position = match state.focused_field {
                        FormField::Directory => state.directory.len(),
                        FormField::ProjectName => state.project_name.len(),
                    };
                }
            }
        }
    }

    // ========================================================================
    // Add screen key handling
    // ========================================================================

    /// Take the AddScreen out of self.screen, replacing it with a Loading screen.
    fn take_add_screen_for_loading(
        &mut self,
        message: &str,
        target_fn: impl FnOnce(AddScreen) -> LoadingTarget,
    ) {
        let screen = std::mem::replace(
            &mut self.screen,
            Screen::Loading(LoadingState {
                message: String::new(),
                target: LoadingTarget::Add,
            }),
        );
        let Screen::Add(add_screen) = screen else {
            self.screen = screen;
            return;
        };
        self.screen = Screen::Loading(LoadingState {
            message: message.to_string(),
            target: target_fn(add_screen),
        });
    }

    fn handle_add_key(&mut self, key: KeyCode) {
        let Screen::Add(state) = &mut self.screen else {
            return;
        };

        match state.tab {
            AddTab::Installed => match key {
                KeyCode::Up | KeyCode::Char('k') => state.installed.select_prev(),
                KeyCode::Down | KeyCode::Char('j') => state.installed.select_next(),
                KeyCode::Char(' ') => state.installed.toggle_selected(),
                KeyCode::Tab => {
                    if state.browse.items.is_empty() {
                        // First visit — load browse list via loading screen
                        self.take_add_screen_for_loading("Loading battery packs...", |s| {
                            LoadingTarget::BrowseList {
                                add_screen: s,
                                filter: None,
                            }
                        });
                    } else {
                        state.tab = AddTab::Browse;
                    }
                }
                KeyCode::Enter => {
                    if state.installed.has_changes() || state.installed.has_new_packs() {
                        state.changes = Some(state.installed.collect_changes());
                        self.should_quit = true;
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                _ => {}
            },
            AddTab::Browse => {
                if state.browse.searching {
                    match key {
                        KeyCode::Enter => {
                            let filter = if state.browse.search_input.is_empty() {
                                None
                            } else {
                                Some(state.browse.search_input.clone())
                            };
                            // Search via loading screen
                            self.take_add_screen_for_loading("Searching...", |mut s| {
                                s.browse.searching = false;
                                LoadingTarget::BrowseList {
                                    add_screen: s,
                                    filter,
                                }
                            });
                        }
                        KeyCode::Esc => {
                            state.browse.searching = false;
                        }
                        KeyCode::Char(c) => {
                            state.browse.search_input.push(c);
                        }
                        KeyCode::Backspace => {
                            state.browse.search_input.pop();
                        }
                        _ => {}
                    }
                } else if let Some(ref mut expanded) = state.browse.expanded {
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => expanded.select_prev(),
                        KeyCode::Down | KeyCode::Char('j') => expanded.select_next(),
                        KeyCode::Char(' ') => expanded.toggle_selected(),
                        KeyCode::Enter => {
                            let expanded = state.browse.expanded.take().unwrap();
                            let has_selected = expanded.pack.entries.iter().any(|e| e.enabled);
                            if has_selected {
                                state.installed.packs.push(expanded.pack);
                            }
                            state.tab = AddTab::Installed;
                        }
                        KeyCode::Esc => {
                            state.browse.expanded = None;
                        }
                        _ => {}
                    }
                } else {
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => {
                            if let Some(selected) = state.browse.list_state.selected() {
                                if selected > 0 {
                                    state.browse.list_state.select(Some(selected - 1));
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if let Some(selected) = state.browse.list_state.selected() {
                                if selected < state.browse.items.len().saturating_sub(1) {
                                    state.browse.list_state.select(Some(selected + 1));
                                }
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(selected) = state.browse.list_state.selected() {
                                if let Some(bp) = state.browse.items.get(selected) {
                                    let already_installed =
                                        state.installed.packs.iter().any(|p| p.name == bp.name);
                                    if already_installed {
                                        state.tab = AddTab::Installed;
                                    } else {
                                        let bp_name = bp.name.clone();
                                        let bp_short_name = bp.short_name.clone();
                                        self.take_add_screen_for_loading(
                                            "Loading battery pack...",
                                            |s| LoadingTarget::BrowseExpand {
                                                add_screen: s,
                                                bp_name,
                                                bp_short_name,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        KeyCode::Char('/') => {
                            state.browse.searching = true;
                        }
                        KeyCode::Tab => {
                            state.tab = AddTab::Installed;
                        }
                        KeyCode::Char('q') => self.should_quit = true,
                        KeyCode::Esc => {
                            state.tab = AddTab::Installed;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // ========================================================================
    // Rendering
    // ========================================================================

    fn render(&mut self, frame: &mut Frame) {
        match &mut self.screen {
            Screen::Loading(state) => render_loading(frame, state),
            Screen::List(state) => render_list(frame, state),
            Screen::Detail(state) => render_detail(frame, state),
            Screen::NewProjectForm(state) => render_form(frame, state),
            Screen::Add(state) => render_add(frame, state),
        }
    }
}

// ============================================================================
// Screen renderers
// ============================================================================

fn render_loading(frame: &mut Frame, state: &LoadingState) {
    let area = frame.area();
    let text = Paragraph::new(state.message.as_str())
        .style(Style::default().fg(Color::Cyan))
        .centered();

    let vertical = Layout::vertical([Constraint::Length(1)]).flex(Flex::Center);
    let [center] = vertical.areas(area);
    frame.render_widget(text, center);
}

fn render_list(frame: &mut Frame, state: &mut ListScreen) {
    let area = frame.area();

    let [header, main, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Header
    let title = match &state.filter {
        Some(f) => format!("Battery Packs (filter: {})", f),
        None => "Battery Packs".to_string(),
    };
    frame.render_widget(
        Paragraph::new(title)
            .style(Style::default().bold())
            .centered(),
        header,
    );

    // List
    let items: Vec<ListItem> = state
        .items
        .iter()
        .map(|bp| {
            let desc = bp.description.lines().next().unwrap_or("");
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<20}", bp.short_name),
                    Style::default().fg(Color::Green).bold(),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:<10}", bp.version),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::raw(desc),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, main, &mut state.list_state);

    // Footer
    frame.render_widget(
        Paragraph::new("↑↓/jk Navigate | Enter Select | q Quit")
            .style(Style::default().fg(Color::DarkGray))
            .centered(),
        footer,
    );
}

/// Helper function to render a selectable section with consistent styling
fn render_selectable_section<'a, T>(
    lines: &mut Vec<Line<'a>>,
    item_index: &mut usize,
    selected_index: usize,
    label: &'a str,
    items: &[T],
    normal_color: Option<Color>,
    format_item: impl Fn(&T) -> String,
) {
    if items.is_empty() {
        return;
    }

    lines.push(Line::styled(label, Style::default().bold()));
    for item in items {
        let selected = selected_index == *item_index;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::Cyan).bold()
        } else {
            match normal_color {
                Some(color) => Style::default().fg(color),
                None => Style::default(),
            }
        };
        let prefix = if selected { "> " } else { "  " };
        lines.push(Line::styled(
            format!("{}{}", prefix, format_item(item)),
            style,
        ));
        *item_index += 1;
    }
    lines.push(Line::from(""));
}

fn render_detail(frame: &mut Frame, state: &DetailScreen) {
    let area = frame.area();
    let detail = &state.detail;

    let [header, main, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Header
    let header_text = Line::from(vec![
        Span::styled(&detail.name, Style::default().fg(Color::Green).bold()),
        Span::raw(" "),
        Span::styled(&detail.version, Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(header_text).centered(), header);

    // Build selectable items to track indices
    let selectable_items = state.selectable_items();

    // Info section
    let mut lines: Vec<Line> = Vec::new();
    let mut item_index: usize = 0;

    if !detail.description.is_empty() {
        lines.push(Line::from(detail.description.clone()));
        lines.push(Line::from(""));
    }

    if !detail.owners.is_empty() {
        lines.push(Line::styled("Authors:", Style::default().bold()));
        for owner in &detail.owners {
            let text = match &owner.name {
                Some(name) => format!("  {} ({})", name, owner.login),
                None => format!("  {}", owner.login),
            };
            lines.push(Line::from(text));
        }
        lines.push(Line::from(""));
    }

    render_selectable_section(
        &mut lines,
        &mut item_index,
        state.selected_index,
        "Crates:",
        &detail.crates,
        None,
        |crate_name| crate_name.clone(),
    );

    render_selectable_section(
        &mut lines,
        &mut item_index,
        state.selected_index,
        "Extends:",
        &detail.extends,
        Some(Color::Yellow),
        |bp| bp.clone(),
    );

    render_selectable_section(
        &mut lines,
        &mut item_index,
        state.selected_index,
        "Templates:",
        &detail.templates,
        Some(Color::Cyan),
        |tmpl| match &tmpl.description {
            Some(desc) => format!("{} - {}", tmpl.name, desc),
            None => tmpl.name.clone(),
        },
    );

    render_selectable_section(
        &mut lines,
        &mut item_index,
        state.selected_index,
        "Examples:",
        &detail.examples,
        Some(Color::Magenta),
        |example| match &example.description {
            Some(desc) => format!("{} - {}", example.name, desc),
            None => example.name.clone(),
        },
    );

    // Actions section (always present)
    let action_labels = [
        "Open on crates.io",
        "Add to project",
        "Create new project from template",
    ];
    render_selectable_section(
        &mut lines,
        &mut item_index,
        state.selected_index,
        "Actions:",
        &action_labels,
        None,
        |label| (*label).to_string(),
    );

    // Sanity check
    debug_assert_eq!(
        item_index,
        selectable_items.len(),
        "Mismatch between rendered items and selectable_items()"
    );

    let info = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(info, main);

    // Footer - show 'n' hint when template is selected
    let back_hint = if state.came_from_list {
        "Esc Back"
    } else {
        "Esc/q Quit"
    };
    let template_selected = matches!(state.selected_item(), Some(DetailItem::Template { .. }));
    let footer_text = if template_selected {
        format!(
            "↑↓/jk Navigate | Enter Open | n New project | {}",
            back_hint
        )
    } else {
        format!("↑↓/jk Navigate | Enter Open/Select | {}", back_hint)
    };
    frame.render_widget(
        Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .centered(),
        footer,
    );
}

fn render_form(frame: &mut Frame, state: &FormScreen) {
    // First render detail view dimmed underneath
    let dimmed_detail = DetailScreen {
        detail: Rc::clone(&state.detail),
        selected_index: state.selected_index,
        came_from_list: state.came_from_list,
    };
    render_detail(frame, &dimmed_detail);

    // Calculate popup area
    let popup_area = centered_rect(60, 40, frame.area());

    // Clear the popup area
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" New Project ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let [_, dir_label, dir_input, _, name_label, name_input, _, hint] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    // Directory field
    frame.render_widget(
        Paragraph::new("Directory:").style(Style::default().bold()),
        dir_label,
    );

    let dir_style = if state.focused_field == FormField::Directory {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    frame.render_widget(
        Paragraph::new(state.directory.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(dir_style),
        ),
        dir_input,
    );

    // Project name field
    frame.render_widget(
        Paragraph::new("Project Name:").style(Style::default().bold()),
        name_label,
    );

    let name_style = if state.focused_field == FormField::ProjectName {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    frame.render_widget(
        Paragraph::new(state.project_name.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(name_style),
        ),
        name_input,
    );

    // Hint
    frame.render_widget(
        Paragraph::new("Tab Switch | Enter Create | Esc Cancel")
            .style(Style::default().fg(Color::DarkGray))
            .centered(),
        hint,
    );

    // Show cursor in active field
    let (cursor_area, cursor_x) = match state.focused_field {
        FormField::Directory => (dir_input, state.cursor_position.min(state.directory.len())),
        FormField::ProjectName => (
            name_input,
            state.cursor_position.min(state.project_name.len()),
        ),
    };
    // +1 for border
    frame.set_cursor_position(Position::new(
        cursor_area.x + 1 + cursor_x as u16,
        cursor_area.y + 1,
    ));
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [v_area] = vertical.areas(area);
    let [h_area] = horizontal.areas(v_area);
    h_area
}

/// Build a GitHub tree URL for a directory path.
/// If repository is set and looks like a GitHub URL, construct a tree URL.
/// Otherwise fall back to a crates.io search or just open the repo root.
fn build_github_url(repository: Option<&str>, path: &str) -> String {
    build_github_ref_url(repository, "tree", path)
}

/// Build a GitHub blob URL for a file path.
fn build_github_blob_url(repository: Option<&str>, path: &str) -> String {
    build_github_ref_url(repository, "blob", path)
}

/// Build a GitHub URL with the specified ref type (tree or blob).
fn build_github_ref_url(repository: Option<&str>, ref_type: &str, path: &str) -> String {
    match repository {
        Some(repo) => {
            // Try to parse GitHub URL: https://github.com/owner/repo
            if let Some(gh_path) = repo
                .strip_prefix("https://github.com/")
                .or_else(|| repo.strip_prefix("http://github.com/"))
            {
                // Remove trailing .git if present
                let gh_path = gh_path.strip_suffix(".git").unwrap_or(gh_path);
                // Remove trailing slash
                let gh_path = gh_path.trim_end_matches('/');
                // Construct URL with main branch
                format!("https://github.com/{}/{}/main/{}", gh_path, ref_type, path)
            } else {
                // Not a GitHub URL, just open the repository URL
                repo.to_string()
            }
        }
        None => {
            // No repository, can't construct URL
            // Fall back to nothing useful - this shouldn't happen in practice
            "https://crates.io".to_string()
        }
    }
}

// ============================================================================
// Add screen renderer
// ============================================================================

fn render_add(frame: &mut Frame, state: &mut AddScreen) {
    let area = frame.area();

    let [header, main, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Tab bar
    let installed_style = if state.tab == AddTab::Installed {
        Style::default().fg(Color::White).bold()
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let browse_style = if state.tab == AddTab::Browse {
        Style::default().fg(Color::White).bold()
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let tab_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("Installed", installed_style),
        Span::raw("  "),
        Span::styled("Browse", browse_style),
    ]);
    frame.render_widget(Paragraph::new(tab_line), header);

    match state.tab {
        AddTab::Installed => render_add_installed(frame, &state.installed, main),
        AddTab::Browse => render_add_browse(frame, &mut state.browse, main),
    }

    // Footer
    let footer_text = match state.tab {
        AddTab::Installed => {
            let has_changes = state.installed.has_changes() || state.installed.has_new_packs();
            if has_changes {
                "↑↓ Navigate │ Space Toggle │ Enter Apply │ Tab Browse │ q Quit"
            } else {
                "↑↓ Navigate │ Space Toggle │ Tab Browse │ q Quit"
            }
        }
        AddTab::Browse => {
            if state.browse.searching {
                "Type to search │ Enter Search │ Esc Cancel"
            } else if state.browse.expanded.is_some() {
                "↑↓ Navigate │ Space Toggle │ Enter Confirm │ Esc Back"
            } else {
                "↑↓ Navigate │ Enter Select │ / Search │ Tab Installed │ q Quit"
            }
        }
    };
    frame.render_widget(
        Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .centered(),
        footer,
    );
}

/// Render a pack header and its crate entries as checkbox lines.
///
/// `selected_index` is the global selection index; `entry_offset` is the starting
/// index of this pack's entries within that global space. Returns the number of
/// entries rendered (so the caller can advance `entry_offset`).
fn render_pack_entries<'a>(
    lines: &mut Vec<Line<'a>>,
    pack: &'a InstalledPackState,
    selected_index: usize,
    entry_offset: usize,
    show_changes: bool,
) -> usize {
    // Pack header
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(&pack.short_name, Style::default().fg(Color::Green).bold()),
        Span::raw(" "),
        Span::styled(&pack.version, Style::default().fg(Color::DarkGray)),
    ]));

    let mut current_group = String::new();
    for (i, entry) in pack.entries.iter().enumerate() {
        if entry.group != current_group && entry.group != "default" {
            lines.push(Line::styled(
                format!("    {}:", entry.group),
                Style::default().fg(Color::Cyan).bold(),
            ));
            current_group = entry.group.clone();
        } else if current_group.is_empty() {
            current_group = entry.group.clone();
        }

        let is_selected = (entry_offset + i) == selected_index;
        let checkbox = if entry.enabled { "[x]" } else { "[ ]" };

        let changed = show_changes && entry.enabled != entry.originally_enabled;
        let style = if is_selected {
            Style::default().fg(Color::Black).bg(Color::Cyan).bold()
        } else if changed {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        lines.push(Line::styled(
            format!("    {} {} {}", checkbox, entry.name, entry.version_info()),
            style,
        ));
    }
    lines.push(Line::from(""));

    pack.entries.len()
}

fn render_add_installed(frame: &mut Frame, state: &InstalledState, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    if state.packs.is_empty() {
        lines.push(Line::styled(
            "  No battery packs installed.",
            Style::default().fg(Color::DarkGray),
        ));
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  Press Tab to browse available battery packs.",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let mut entry_offset = 0;
    for pack in &state.packs {
        entry_offset +=
            render_pack_entries(&mut lines, pack, state.selected_index, entry_offset, true);
    }

    let content = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(content, area);
}

fn render_add_browse(frame: &mut Frame, state: &mut BrowseState, area: Rect) {
    if let Some(ref expanded) = state.expanded {
        // Show expanded pack crate picker
        render_expanded_pack(frame, expanded, area);
        return;
    }

    let [search_area, list_area] = Layout::vertical([
        Constraint::Length(if state.searching { 3 } else { 0 }),
        Constraint::Fill(1),
    ])
    .areas(area);

    // Search input (only shown when searching)
    if state.searching {
        let input = Paragraph::new(state.search_input.as_str()).block(
            Block::default()
                .title(" Search ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
        frame.render_widget(input, search_area);
        // Show cursor
        frame.set_cursor_position(Position::new(
            search_area.x + 1 + state.search_input.len() as u16,
            search_area.y + 1,
        ));
    }

    // Battery pack list
    let items: Vec<ListItem> = state
        .items
        .iter()
        .map(|bp| {
            let desc = bp.description.lines().next().unwrap_or("");
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<20}", bp.short_name),
                    Style::default().fg(Color::Green).bold(),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:<10}", bp.version),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::raw(desc),
            ]);
            ListItem::new(line)
        })
        .collect();

    if items.is_empty() && !state.searching {
        let hint = Paragraph::new("  Press / to search for battery packs on crates.io")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(hint, list_area);
    } else {
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, list_area, &mut state.list_state);
    }
}

fn render_expanded_pack(frame: &mut Frame, expanded: &ExpandedPack, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    render_pack_entries(
        &mut lines,
        &expanded.pack,
        expanded.selected_index,
        0,
        false,
    );

    let content = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(content, area);
}

// ============================================================================
// Add screen helpers
// ============================================================================

/// Apply collected changes from the Add TUI.
fn apply_add_changes(changes: &[AddChange]) -> Result<()> {
    use console::style;

    for change in changes {
        match change {
            AddChange::AddPack { name, crates } => {
                println!(
                    "{}",
                    style(format!("Adding {} ({} crate(s))...", name, crates.len())).bold()
                );
                // Shell out to cargo bp add with --all, then we could refine.
                // For now, use the existing add_battery_pack flow via CLI.
                let status = std::process::Command::new("cargo")
                    .args(["bp", "add", name, "--all"])
                    .status()?;
                if !status.success() {
                    println!("{}", style(format!("  Failed to add {}", name)).red());
                }
            }
            AddChange::UpdatePack {
                name,
                add_crates,
                remove_crates,
            } => {
                if !add_crates.is_empty() {
                    println!(
                        "{}",
                        style(format!(
                            "Adding {} crate(s) to {}...",
                            add_crates.len(),
                            name
                        ))
                        .bold()
                    );
                    for c in add_crates {
                        println!("  + {}", c.name);
                    }
                }
                if !remove_crates.is_empty() {
                    println!(
                        "{}",
                        style(format!(
                            "Removing {} crate(s) from {}...",
                            remove_crates.len(),
                            name
                        ))
                        .bold()
                    );
                    for crate_name in remove_crates {
                        println!("  - {}", crate_name);
                    }
                }
                // TODO: Implement fine-grained Cargo.toml manipulation
                // For now, run cargo bp sync to reconcile
                let _ = std::process::Command::new("cargo")
                    .args(["bp", "sync"])
                    .status();
            }
        }
    }

    Ok(())
}
