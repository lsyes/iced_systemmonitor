// SPDX-License-Identifier: GPL-3.0-only

//! The application state, update logic and views.

use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use iced::{
    Alignment, Element, Length, Point, Size, Subscription, Task, Theme, Widget,
    alignment::Horizontal,
    keyboard,
    widget::{
        Space, button, column, container, opaque, pick_list, progress_bar, row, rule, scrollable,
        stack, text,
    },
};
use itertools::Itertools;
use regex::{Regex, RegexBuilder};
use sysinfo::Pid;

use crate::{
    config::{AppTheme, Config},
    fl,
    graph::{Graph, GraphKind, ProcGraphKind},
    icons,
    info::{GpuId, GpuState, GraphItem, ProcessCategory, ProcessItem},
    theme,
    widget::{
        card::card,
        context_menu,
        dialog,
        menu_bar::{self, MenuItem},
        nav_bar,
        table::{self, ItemInterface, SortDirection},
        tag, text_input,
    },
};

pub const CARD_DATA_HEIGHT: f32 = 62.0;
pub const SMALL_GRAPH_HEIGHT: f32 = 207.0;
pub const LARGE_GRAPH_HEIGHT: f32 = 300.0;
pub const MIN_GRAPH_WIDTH: f32 = 640.0;
pub const MIN_PROCESSES_WIDTH: f32 = 720.0;

/// The width of the navigation bar.
const NAV_WIDTH: f32 = 200.0;

/// The width of a single per-core utilization entry.
const CORE_WIDTH: f32 = 300.0;
/// The width reserved for the name in a per-core entry.
const CORE_NAME_WIDTH: f32 = CORE_WIDTH - 110.0;
/// The width reserved for the value in a per-core entry.
const CORE_VALUE_WIDTH: f32 = 70.0;

/// A pending confirmation dialog.
#[derive(Clone, Debug)]
pub enum DialogKind {
    AppQuit {
        name: String,
        processes: Vec<ProcessItem>,
        force: bool,
    },
    ProcessQuit {
        name: String,
        pid: Pid,
        force: bool,
    },
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    AppTheme(AppTheme),
    ContextMenu(context_menu::Event),
    CloseContext,
    CpuGraph(ProcGraphKind),
    DialogCancel,
    DialogConfirm,
    Escape,
    GpuGraph(GpuId, ProcGraphKind),
    GpuSelect(String),
    Graph(GraphItem),
    LaunchUrl(String),
    MenuClose,
    MenuToggle,
    NavPage(NavPage),
    ProcessSort(ProcessCategory),
    RowContext(SelectedItem, Point),
    Search(text_input::Event),
    SeeAllProcesses(bool, ProcessCategory, SortDirection),
    Select(Option<SelectedItem>),
    Size(Size),
    Snapshot(GraphItem, Vec<ProcessItem>, Vec<ProcessItem>),
    ToggleContextPage(ContextPage),
    WindowHandle(Option<(usize, usize)>),
    WindowUnfocused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPage {
    About,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavPage {
    Dashboard,
    Applications,
    Processes,
    Cpu,
    Memory,
    Gpu,
    Disk,
    Network,
}

impl NavPage {
    pub fn all() -> &'static [Self] {
        &[
            Self::Dashboard,
            Self::Applications,
            Self::Processes,
            Self::Cpu,
            Self::Memory,
            Self::Gpu,
            Self::Disk,
            Self::Network,
        ]
    }

    pub fn title(&self) -> String {
        match self {
            Self::Dashboard => fl!("dashboard"),
            Self::Applications => fl!("applications"),
            Self::Processes => fl!("processes"),
            Self::Cpu => fl!("cpu"),
            Self::Memory => fl!("memory"),
            Self::Gpu => fl!("gpu"),
            Self::Disk => fl!("disk"),
            Self::Network => fl!("network"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectedItem {
    App(String),
    Process(Pid),
}

/// An entry of the context menu of a process row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessAction {
    ForceQuit,
    Quit,
    CopyName,
    CopyPid,
}

impl ProcessAction {
    /// Every action, in menu order.
    const ALL: [Self; 4] = [Self::ForceQuit, Self::Quit, Self::CopyName, Self::CopyPid];

    /// The label shown for the action.
    fn label(self) -> String {
        match self {
            Self::ForceQuit => fl!("force-quit"),
            Self::Quit => fl!("quit"),
            Self::CopyName => fl!("copy-name"),
            Self::CopyPid => fl!("copy-pid"),
        }
    }
}

/// The [`App`] stores application-specific state.
pub struct App {
    app_search: (String, Option<Regex>),
    apps: Vec<ProcessItem>,
    config: Config,
    context_menu: context_menu::State,
    context_page: Option<ContextPage>,
    cpu_graph: ProcGraphKind,
    dialog_opt: Option<DialogKind>,
    gpu_graphs: HashMap<GpuId, ProcGraphKind>,
    gpu_id_opt: Option<GpuId>,
    gpu_names: Vec<String>,
    graph_history: VecDeque<GraphItem>,
    graph_snapshot: Option<GraphItem>,
    menu_open: bool,
    nav_page: NavPage,
    process_search: (String, Option<Regex>),
    process_sort: (ProcessCategory, SortDirection),
    processes: Vec<ProcessItem>,
    search: text_input::State,
    selected: Option<SelectedItem>,
    size: Option<Size>,
}

impl App {
    /// Creates the application, and optionally emits a task on initialize.
    pub fn new() -> (Self, Task<Message>) {
        let mut context_menu = context_menu::State::new();
        context_menu.set_items(ProcessAction::ALL.iter().map(|action| action.label()));

        let app = Self {
            app_search: (String::new(), None),
            apps: Vec::new(),
            config: Config::load(),
            context_menu,
            context_page: None,
            cpu_graph: ProcGraphKind::default(),
            dialog_opt: None,
            gpu_graphs: HashMap::new(),
            gpu_id_opt: None,
            gpu_names: Vec::new(),
            graph_history: VecDeque::new(),
            graph_snapshot: None,
            menu_open: false,
            nav_page: NavPage::Dashboard,
            process_search: (String::new(), None),
            process_sort: (ProcessCategory::default(), SortDirection::default()),
            processes: Vec::new(),
            search: text_input::State::new(),
            selected: None,
            size: None,
        };

        (app, capture_window_handle())
    }

    /// Re-sorts the process lists for the current page.
    fn update_snapshot(&mut self) {
        let sort = self.process_sort;

        let list = if self.nav_page == NavPage::Applications {
            &mut self.apps
        } else {
            &mut self.processes
        };

        list.sort_by(|a, b| {
            if sort.1.is_descending() {
                b.compare(a, sort.0)
            } else {
                a.compare(b, sort.0)
            }
        });
    }

    /// The categories shown for the current list page.
    fn list_categories(&self) -> Vec<ProcessCategory> {
        match self.nav_page {
            NavPage::Applications => ProcessCategory::for_applications(self.process_sort.0),
            _ => ProcessCategory::for_processes(self.process_sort.0),
        }
    }

    /// The items shown for the current list page, filtered by the search term.
    fn list_items(&self) -> Vec<&ProcessItem> {
        let search = if self.nav_page == NavPage::Applications {
            &self.app_search
        } else {
            &self.process_search
        };

        let items = if self.nav_page == NavPage::Applications {
            &self.apps
        } else {
            &self.processes
        };

        items
            .iter()
            .filter(|item| search.1.as_ref().is_none_or(|regex| item.matches(regex)))
            .collect()
    }

    /// The size available to the page content.
    fn content_size(&self) -> Size {
        let size = self.size.unwrap_or(Size::new(1280.0, 800.0));

        Size::new((size.width - NAV_WIDTH).max(320.0), size.height)
    }

    /// Handles application events.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AppTheme(app_theme) => {
                self.config.app_theme = app_theme;
                self.config.save();
            }
            Message::CloseContext => {
                self.context_page = None;
            }
            Message::CpuGraph(cpu_graph) => {
                self.cpu_graph = cpu_graph;
            }
            Message::ContextMenu(event) => {
                if matches!(event, context_menu::Event::Open(_)) {
                    self.search.close_menu();
                }

                let theme = self.current_theme();

                if let Some(index) = self.context_menu.update(event, &theme) {
                    return self.apply_process_action(index);
                }
            }
            Message::RowContext(target, position) => {
                // The menu acts on the selected row, so a right click first
                // selects the row it belongs to.
                self.selected = Some(target);

                return self.update(Message::ContextMenu(context_menu::Event::Open(position)));
            }
            Message::Search(event) => {
                if matches!(&event, text_input::Event::Menu(context_menu::Event::Open(_))) {
                    self.context_menu.close();
                }

                let theme = self.current_theme();
                let (changed, task) = self.search.update(event, &theme, Message::Search);

                if changed {
                    self.sync_search_from_input();
                }

                return task;
            }
            Message::WindowHandle(handle) => {
                self.context_menu.set_window_handle(handle);
                self.search.set_window_handle(handle);
            }
            Message::WindowUnfocused => {
                // The compositor can leave a native popup on screen when the
                // window loses focus, so the menus are closed here as well.
                self.context_menu.close();
                self.search.close_menu();
            }
            Message::DialogCancel => {
                self.dialog_opt = None;
            }
            Message::DialogConfirm => {
                if let Some(dialog_kind) = self.dialog_opt.take() {
                    match dialog_kind {
                        DialogKind::AppQuit {
                            processes, force, ..
                        } => {
                            for process in processes {
                                let Some(pid) = process.pid else { continue };
                                kill(pid, force);
                            }
                        }
                        DialogKind::ProcessQuit { pid, force, .. } => {
                            kill(pid, force);
                        }
                    }
                }
            }
            Message::Escape => {
                if self.dialog_opt.take().is_some() {
                    return Task::none();
                }
                if self.menu_open {
                    self.menu_open = false;
                    return Task::none();
                }
                if self.context_menu.is_open() {
                    self.context_menu.close();
                    return Task::none();
                }
                if self.search.is_menu_open() {
                    self.search.close_menu();
                    return Task::none();
                }
                if self.context_page.take().is_some() {
                    return Task::none();
                }
                if self.selected.take().is_some() {
                    return Task::none();
                }

                let search_empty = if self.nav_page == NavPage::Applications {
                    self.app_search.0.is_empty()
                } else {
                    self.process_search.0.is_empty()
                };

                if !search_empty {
                    self.search.set_text("");
                    self.sync_search_from_input();
                    return Task::none();
                }
            }
            Message::GpuGraph(gpu_id, gpu_graph) => {
                self.gpu_graphs.insert(gpu_id, gpu_graph);
            }
            Message::GpuSelect(name) => {
                self.gpu_id_opt = None;
                if let Some(gpu_i) = self.gpu_names.iter().position(|x| x == &name)
                    && let Some(graph_item) = &self.graph_snapshot
                    && let Some(gpu) = graph_item.gpus.get(gpu_i)
                {
                    self.gpu_id_opt = Some(gpu.id);
                }
            }
            Message::Graph(graph_item) => {
                self.graph_history.push_back(graph_item);
                let now = Instant::now();
                self.graph_history
                    .retain(|x| now.saturating_duration_since(x.time) < Duration::from_secs(60));
            }
            Message::LaunchUrl(url) => {
                if let Err(err) = open::that_detached(&url) {
                    log::warn!("failed to open {:?}: {}", url, err);
                }
            }
            Message::MenuClose => {
                self.menu_open = false;
            }
            Message::MenuToggle => {
                self.menu_open = !self.menu_open;
            }
            Message::NavPage(nav_page) => {
                self.nav_page = nav_page;
                self.selected = None;

                let value = self.active_search_value().clone();
                self.search.set_text(&value);

                self.update_snapshot();
            }
            Message::ProcessSort(category) => {
                if self.process_sort.0 == category {
                    self.process_sort.1 = self.process_sort.1.flipped();
                } else {
                    self.process_sort = (category, SortDirection::default());
                }
                self.update_snapshot();
            }
            Message::SeeAllProcesses(show_apps, category, direction) => {
                self.process_sort = (category, direction);
                self.nav_page = if show_apps {
                    NavPage::Applications
                } else {
                    NavPage::Processes
                };
                self.selected = None;
                self.update_snapshot();
            }
            Message::Select(selected) => {
                self.selected = selected;
            }
            Message::Size(size) => {
                self.size = Some(size);
            }
            Message::Snapshot(graph_item, apps, processes) => {
                self.graph_snapshot = Some(graph_item);
                self.apps = apps;
                self.processes = processes;
                self.update_snapshot();
            }
            Message::ToggleContextPage(context_page) => {
                self.menu_open = false;
                if self.context_page == Some(context_page) {
                    self.context_page = None;
                } else {
                    self.context_page = Some(context_page);
                }
            }
        }

        Task::none()
    }

    /// The title of the window.
    pub fn title(&self) -> String {
        fl!("app-name")
    }

    /// The theme of the application. `None` follows the system theme.
    pub fn theme(&self) -> Option<Theme> {
        self.config.app_theme.theme()
    }

    /// The subscriptions of the application.
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::window::resize_events().map(|(_id, size)| Message::Size(size)),
            iced::window::events().filter_map(|(_id, event)| match event {
                iced::window::Event::Unfocused => Some(Message::WindowUnfocused),
                _ => None,
            }),
            Subscription::run(crate::info::worker),
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                } => Some(Message::Escape),
                _ => None,
            }),
        ];

        subscriptions.push(self.context_menu.subscription().map(Message::ContextMenu));
        subscriptions.push(self.search.subscription().map(Message::Search));

        Subscription::batch(subscriptions)
    }

    /// The root view of the application.
    pub fn view(&self) -> Element<'_, Message> {
        let nav = nav_bar::nav_bar(
            NavPage::all().iter().map(|page| (*page, page.title())),
            self.nav_page,
            Message::NavPage,
        );

        let page = match (self.nav_page, &self.graph_snapshot) {
            (NavPage::Dashboard, Some(graph_item)) => self.view_dashboard(graph_item),
            (NavPage::Applications | NavPage::Processes, _) => self.view_processes(),
            (NavPage::Cpu, Some(graph_item)) => self.view_cpu(graph_item),
            (NavPage::Memory, Some(graph_item)) => self.view_memory(graph_item),
            (NavPage::Gpu, Some(graph_item)) => self.view_gpu(graph_item),
            (NavPage::Disk, Some(graph_item)) => self.view_disk(graph_item),
            (NavPage::Network, Some(graph_item)) => self.view_network(graph_item),
            _ => container(text(fl!("loading")).size(16))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        };

        let content = column![
            self.header(),
            row![nav, page].width(Length::Fill).height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        let mut layers: Vec<Element<'_, Message>> = vec![content.boxed()];

        if let Some(context_page) = self.context_page {
            layers.push(self.view_context_page(context_page));
        }

        if self.menu_open {
            layers.push(menu_bar::menu_dismiss(Message::MenuClose));
            layers.push(self.view_menu_panel());
        }

        layers.extend(self.context_menu.layers(Message::ContextMenu));
        layers.extend(self.search.layers(Message::Search));

        let base: Element<'_, Message> = stack(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed();

        if let Some(dialog) = self.view_dialog() {
            dialog::modal(base, dialog, Message::DialogCancel)
        } else {
            base
        }
    }

    /// The header bar with the in-application menu.
    fn header(&self) -> Element<'_, Message> {
        let menu_button = menu_bar::menu_button("\u{2261}", self.menu_open, Message::MenuToggle);

        container(
            row![
                menu_button,
                text(fl!("app-name")).size(16),
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SPACE_S),
        )
        .padding([theme::SPACE_XXS, theme::SPACE_S])
        .width(Length::Fill)
        .style(|iced_theme: &iced::Theme| {
            let palette = iced_theme.palette();

            container::Style {
                background: Some(palette.background.base.color.into()),
                ..container::Style::default()
            }
        })
        .boxed()
    }

    /// The theme used to render the native context menus.
    fn current_theme(&self) -> Theme {
        self.config.app_theme.theme().unwrap_or(Theme::Light)
    }

    /// The value of the search input on the current page.
    fn active_search_value(&self) -> &String {
        if self.nav_page == NavPage::Applications {
            &self.app_search.0
        } else {
            &self.process_search.0
        }
    }

    /// The name and the PIDs of the selected item, if it still exists.
    fn selected_details(&self) -> Option<(String, String)> {
        match self.selected.as_ref()? {
            SelectedItem::App(app_id) => {
                let item = self
                    .apps
                    .iter()
                    .find(|x| x.app.as_ref().is_some_and(|app| &app.id == app_id))?;

                let pids: Vec<String> = self
                    .processes
                    .iter()
                    .filter(|x| x.app.as_ref().is_some_and(|app| &app.id == app_id))
                    .filter_map(|x| x.pid)
                    .map(|pid| pid.to_string())
                    .collect();

                Some((item.name.clone(), pids.join(", ")))
            }
            SelectedItem::Process(pid) => {
                let item = self.processes.iter().find(|x| x.pid == Some(*pid))?;

                Some((item.name.clone(), pid.to_string()))
            }
        }
    }

    /// Applies an action of the context menu of a process row.
    fn apply_process_action(&mut self, index: usize) -> Task<Message> {
        let Some(action) = ProcessAction::ALL.get(index).copied() else {
            return Task::none();
        };

        match action {
            ProcessAction::CopyName | ProcessAction::CopyPid => {
                let Some((name, pids)) = self.selected_details() else {
                    return Task::none();
                };

                let text = if action == ProcessAction::CopyName {
                    name
                } else {
                    pids
                };

                iced::clipboard::write(text).discard()
            }
            ProcessAction::Quit | ProcessAction::ForceQuit => {
                let force = action == ProcessAction::ForceQuit;
                let Some(selected) = self.selected.clone() else {
                    return Task::none();
                };
                let Some((name, _)) = self.selected_details() else {
                    return Task::none();
                };

                self.dialog_opt = Some(match selected {
                    SelectedItem::App(app_id) => {
                        let processes: Vec<ProcessItem> = self
                            .processes
                            .iter()
                            .filter(|x| {
                                x.app
                                    .as_ref()
                                    .is_some_and(|app| app.id == app_id)
                            })
                            .cloned()
                            .collect();

                        DialogKind::AppQuit {
                            name,
                            processes,
                            force,
                        }
                    }
                    SelectedItem::Process(pid) => DialogKind::ProcessQuit { name, pid, force },
                });

                Task::none()
            }
        }
    }

    /// Mirrors the text input into the active page's search filter.
    fn sync_search_from_input(&mut self) {
        let text = self.search.text().replace('\n', " ");
        let regex = search_regex(&text);

        if self.nav_page == NavPage::Applications {
            self.app_search = (text, regex);
        } else {
            self.process_search = (text, regex);
        }

        self.update_snapshot();
    }

    /// The application menu panel, positioned below the menu button.
    fn view_menu_panel(&self) -> Element<'_, Message> {
        let panel = menu_bar::menu_panel(
            [
                MenuItem::new(
                    fl!("menu-settings"),
                    Message::ToggleContextPage(ContextPage::Settings),
                ),
                MenuItem::new(
                    fl!("menu-about"),
                    Message::ToggleContextPage(ContextPage::About),
                ),
            ],
            260.0,
        );

        container(panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(iced::alignment::Vertical::Top)
            .padding(iced::Padding {
                top: 44.0,
                left: theme::SPACE_XS,
                right: 0.0,
                bottom: 0.0,
            })
            .boxed()
    }

    /// The settings or about panel, shown over the content.
    fn view_context_page(&self, context_page: ContextPage) -> Element<'_, Message> {
        let content = match context_page {
            ContextPage::Settings => self.view_settings(),
            ContextPage::About => self.view_about(),
        };

        let panel = container(
            column![
                row![
                    text(match context_page {
                        ContextPage::Settings => fl!("settings"),
                        ContextPage::About => fl!("menu-about"),
                    })
                    .size(20),
                    Space::new().width(Length::Fill),
                    button(text("\u{2715}")).style(flat_button).on_press(Message::CloseContext),
                ]
                .align_y(Alignment::Center),
                scrollable(content).height(Length::Fill),
            ]
            .spacing(theme::SPACE_M)
            .height(Length::Fill),
        )
        .padding(theme::SPACE_M)
        .width(Length::Fixed(360.0))
        .height(Length::Fill)
        .style(|iced_theme: &iced::Theme| {
            let palette = iced_theme.palette();

            container::Style {
                background: Some(palette.background.base.color.into()),
                ..container::Style::default()
            }
        });

        stack![
            opaque(
                iced::widget::mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                    .on_press(Message::CloseContext)
            ),
            row![Space::new().width(Length::Fill), opaque(panel)]
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let themes = [
            (AppTheme::Light, fl!("light")),
            (AppTheme::Dark, fl!("dark")),
            (AppTheme::System, fl!("match-desktop")),
        ];

        let mut children: Vec<Element<'_, Message>> = Vec::with_capacity(themes.len());

        for (app_theme, label) in themes {
            let selected = self.config.app_theme == app_theme;

            children.push(tag::tag(
                text(label).boxed(),
                selected,
                false,
                Message::AppTheme(app_theme),
            ));
        }

        column![
            text(fl!("appearance")).size(16),
            row(children).spacing(theme::SPACE_XXS),
        ]
        .spacing(theme::SPACE_XS)
        .width(Length::Fill)
        .boxed()
    }

    fn view_about(&self) -> Element<'_, Message> {
        column![
            text(fl!("app-name")).size(24),
            text(format!("{} {}", fl!("version"), env!("CARGO_PKG_VERSION"))),
            text("GPL-3.0-only"),
            text(fl!("comment")),
            rule::horizontal(1),
            button(text(fl!("repository")))
                .style(flat_button)
                .on_press(Message::LaunchUrl(
                    "https://github.com/iced-rs/iced".to_string()
                )),
        ]
        .spacing(theme::SPACE_XS)
        .width(Length::Fill)
        .boxed()
    }

    fn view_dialog(&self) -> Option<Element<'_, Message>> {
        let dialog_kind = self.dialog_opt.as_ref()?;

        Some(match dialog_kind {
            DialogKind::AppQuit {
                name,
                processes,
                force,
            } => {
                let categories = [
                    ProcessCategory::App,
                    ProcessCategory::Name,
                    ProcessCategory::PID,
                ];

                let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(processes.len() * 2);

                for process in processes.iter() {
                    rows.push(table::divider());
                    rows.push(table::row_item(
                        process,
                        &categories,
                        false,
                        None::<Message>,
                    ));
                }

                let max_height = self
                    .size
                    .map_or(480.0, |size| (size.height - 300.0).min(480.0));

                let table_content = scrollable(
                    column![
                        table::header(
                            &categories,
                            ProcessCategory::Name,
                            SortDirection::default(),
                            false,
                            |_| Message::Escape,
                        ),
                        column(rows),
                    ]
                    .width(Length::Fill),
                )
                .height(Length::Fixed(max_height));

                dialog::confirm(
                    if *force {
                        fl!("force-quit-app-title", name = name)
                    } else {
                        fl!("quit-app-title", name = name)
                    },
                    if *force {
                        fl!("force-quit-app-body")
                    } else {
                        fl!("quit-app-body")
                    },
                    Some(table_content.boxed()),
                    (
                        if *force {
                            fl!("force-quit")
                        } else {
                            fl!("quit")
                        },
                        Message::DialogConfirm,
                    ),
                    (fl!("cancel"), Message::DialogCancel),
                )
            }
            DialogKind::ProcessQuit { name, force, .. } => dialog::confirm(
                if *force {
                    fl!("force-quit-title")
                } else {
                    fl!("quit-title")
                },
                if *force {
                    fl!("force-quit-body", name = name)
                } else {
                    fl!("quit-body", name = name)
                },
                None,
                (
                    if *force {
                        fl!("force-quit")
                    } else {
                        fl!("quit")
                    },
                    Message::DialogConfirm,
                ),
                (fl!("cancel"), Message::DialogCancel),
            ),
        })
    }

    /// A header with an optional back button.
    fn page_header(&self, title: String, with_back: bool) -> Element<'_, Message> {
        let mut children: Vec<Element<'_, Message>> = Vec::with_capacity(2);

        if with_back {
            children.push(
                button(text(format!("\u{2039} {}", fl!("dashboard"))))
                    .style(flat_button)
                    .on_press(Message::NavPage(NavPage::Dashboard))
                    .boxed(),
            );
        }

        children.push(text(title).size(24).boxed());

        column(children)
            .spacing(theme::SPACE_XS)
            .width(Length::Fill)
            .boxed()
    }

    /// Renders one process row, which opens the context menu on right-click.
    fn view_row<'a>(
        &'a self,
        item: &'a ProcessItem,
        categories: &[ProcessCategory],
        selected: Option<&SelectedItem>,
    ) -> Element<'a, Message> {
        let target = item.as_selected();

        let row = table::row_item(
            item,
            categories,
            target.as_ref() == selected,
            Some(Message::Select(target.clone())),
        );

        match target {
            Some(target) => self.context_menu.wrap(row, move |position| {
                Message::RowContext(target.clone(), position)
            }),
            None => row,
        }
    }

    fn view_processes(&self) -> Element<'_, Message> {
        let is_apps = self.nav_page == NavPage::Applications;
        let categories = self.list_categories();

        let input = self.search.view(fl!("search-processes"), Message::Search);

        let selected = self.selected.clone();
        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        for item in self.list_items() {
            rows.push(table::divider());
            rows.push(self.view_row(item, &categories, selected.as_ref()));
        }

        column![
            self.page_header(
                if is_apps {
                    fl!("applications")
                } else {
                    fl!("processes")
                },
                false
            ),
            container(input).width(Length::Fill).center_x(Length::Fill),
            container(
                card(column![
                    table::header(
                        &categories,
                        self.process_sort.0,
                        self.process_sort.1,
                        true,
                        Message::ProcessSort,
                    ),
                    scrollable(column(rows).width(Length::Fill)).height(Length::Fill),
                ]
                .spacing(theme::SPACE_XXS)
                .width(Length::Fill)
                .boxed()),
            )
            .height(Length::Fill),
        ]
        .spacing(theme::SPACE_S)
        .padding(theme::PAGE_PADDING)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    fn view_dashboard<'a>(&'a self, graph_item: &'a GraphItem) -> Element<'a, Message> {
        let size = self.content_size();
        let items = self.dashboard_cards(graph_item);

        let card_height = theme::SPACE_S + SMALL_GRAPH_HEIGHT + theme::SPACE_S;
        let min_width = 440.0;
        let content_width = (size.width - theme::SPACE_XL * 2.0).max(min_width);

        let large_width = content_width - (MIN_PROCESSES_WIDTH + theme::SPACE_S) * 2.0;
        let large_cards = (large_width / min_width).floor()
            * (size.height / (card_height + theme::SPACE_S)).floor();

        let (graphs_width, large) = if large_cards >= items.len() as f32 {
            (large_width, true)
        } else {
            (content_width, false)
        };

        let mut cols = 1;
        while cols < 4 && graphs_width / ((cols + 1) as f32) > min_width {
            cols += 1;
        }

        let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(items.len() / cols + 1);
        let mut grid: Vec<Element<'a, Message>> = Vec::with_capacity(cols);

        for item in items {
            grid.push(item);

            if grid.len() == cols {
                rows.push(
                    row(std::mem::take(&mut grid))
                        .spacing(theme::SPACE_S)
                        .width(Length::Fill)
                        .boxed(),
                );
            }
        }

        if !grid.is_empty() {
            while grid.len() < cols {
                grid.push(Space::new().width(Length::Fill).boxed());
            }

            rows.push(
                row(grid)
                    .spacing(theme::SPACE_S)
                    .width(Length::Fill)
                    .boxed(),
            );
        }

        let lists = column![
            card(self.top_processes_by(
                true,
                self.process_sort.0,
                self.process_sort.1,
                true,
                5
            )),
            card(self.top_processes_by(
                false,
                self.process_sort.0,
                self.process_sort.1,
                true,
                5
            )),
        ]
        .spacing(theme::SPACE_S)
        .width(Length::Fill);

        let content: Element<'a, Message> = if large {
            row![
                lists.width(Length::Fixed(MIN_PROCESSES_WIDTH)),
                column(rows).spacing(theme::SPACE_S).width(Length::Fill),
            ]
            .spacing(theme::SPACE_S)
            .width(Length::Fill)
            .boxed()
        } else {
            column![
                column(rows).spacing(theme::SPACE_S).width(Length::Fill),
                lists,
            ]
            .spacing(theme::SPACE_S)
            .width(Length::Fill)
            .boxed()
        };

        iced::widget::mouse_area(
            scrollable(
                container(content)
                    .padding(theme::PAGE_PADDING)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .on_press(Message::Select(None))
        .boxed()
    }

    fn dashboard_cards<'a>(&'a self, graph_item: &'a GraphItem) -> Vec<Element<'a, Message>> {
        let mut items: Vec<Element<'a, Message>> =
            Vec::with_capacity(4 + graph_item.gpus.len() * 2);

        let cpu_graph = self.cpu_graph;

        let mut cpu_data: Vec<Element<'a, Message>> = Vec::with_capacity(2);
        cpu_data.push(
            row![
                tag::tag(
                    text(format!("{:.1}%", graph_item.total_cpu_usage())).boxed(),
                    cpu_graph == ProcGraphKind::Utilization,
                    false,
                    Message::CpuGraph(ProcGraphKind::Utilization),
                ),
                tag::tag(
                    text(format_frequency(graph_item.max_cpu_frequency())).boxed(),
                    cpu_graph == ProcGraphKind::Frequency,
                    false,
                    Message::CpuGraph(ProcGraphKind::Frequency),
                ),
            ]
            .spacing(theme::SPACE_XXXS)
            .width(Length::Fill)
            .boxed(),
        );

        if let Some(temp) = graph_item.max_cpu_temp() {
            cpu_data.push(
                row![tag::tag(
                    text(format!("{:.1}°C", temp)).boxed(),
                    cpu_graph == ProcGraphKind::Temperature,
                    false,
                    Message::CpuGraph(ProcGraphKind::Temperature),
                )]
                .spacing(theme::SPACE_XXXS)
                .width(Length::Fill)
                .boxed(),
            );
        }

        items.push(self.graph_card(
            GraphKind::Cpu(cpu_graph),
            fl!("cpu"),
            graph_item
                .cpus
                .first()
                .map(|x| x.brand.clone())
                .unwrap_or_default(),
            column(cpu_data).spacing(theme::SPACE_XXXS).boxed(),
            Some(ProcessCategory::CPU),
            Message::NavPage(NavPage::Cpu),
        ));

        items.push(self.graph_card(
            GraphKind::Memory,
            fl!("memory"),
            humansize::format_size(graph_item.memory.total, humansize::BINARY),
            column![
                text(format!(
                    "{:.1}%",
                    100.0 * (graph_item.memory.used as f32) / (graph_item.memory.total as f32),
                )),
                text(humansize::format_size(
                    graph_item.memory.used,
                    humansize::BINARY
                )),
            ]
            .spacing(theme::SPACE_XXXS)
            .boxed(),
            Some(ProcessCategory::Memory),
            Message::NavPage(NavPage::Memory),
        ));

        let disk_io = graph_item.total_disk_io();
        items.push(self.graph_card(
            GraphKind::DiskTotal,
            fl!("disk"),
            String::new(),
            column![
                text(format!(
                    "{}/s read",
                    humansize::format_size(disk_io.0 as u64, humansize::DECIMAL),
                )),
                text(format!(
                    "{}/s write",
                    humansize::format_size(disk_io.1 as u64, humansize::DECIMAL),
                )),
            ]
            .spacing(theme::SPACE_XXXS)
            .boxed(),
            Some(ProcessCategory::DiskTotal),
            Message::NavPage(NavPage::Disk),
        ));

        let network_io = graph_item.total_network_io();
        items.push(self.graph_card(
            GraphKind::NetworkTotal,
            fl!("network"),
            String::new(),
            column![
                text(format!(
                    "{}/s rx",
                    humansize::format_size(network_io.0 as u64, humansize::DECIMAL),
                )),
                text(format!(
                    "{}/s tx",
                    humansize::format_size(network_io.1 as u64, humansize::DECIMAL),
                )),
            ]
            .spacing(theme::SPACE_XXXS)
            .boxed(),
            None,
            Message::NavPage(NavPage::Network),
        ));

        for (gpu_i, gpu) in graph_item.gpus.iter().enumerate() {
            let gpu_graph = self.gpu_graphs.get(&gpu.id).copied().unwrap_or_default();

            if let Some(usage) = gpu.usage {
                let (data, process_category): (Element<'a, Message>, _) = match gpu.state {
                    GpuState::Active | GpuState::Idle(_) => {
                        let mut data: Vec<Element<'a, Message>> = Vec::with_capacity(2);

                        let mut first: Vec<Element<'a, Message>> = vec![tag::tag(
                            text(format!("{:.1}%", usage)).boxed(),
                            gpu_graph == ProcGraphKind::Utilization,
                            false,
                            Message::GpuGraph(gpu.id, ProcGraphKind::Utilization),
                        )];

                        if let Some(frequency) = gpu.frequency {
                            first.push(tag::tag(
                                text(format_frequency(frequency)).boxed(),
                                gpu_graph == ProcGraphKind::Frequency,
                                false,
                                Message::GpuGraph(gpu.id, ProcGraphKind::Frequency),
                            ));
                        }

                        data.push(row(first).spacing(theme::SPACE_XXXS).boxed());

                        let mut second: Vec<Element<'a, Message>> = Vec::with_capacity(2);

                        if let Some(power) = gpu.power {
                            second.push(tag::tag(
                                text(format!("{:.1} W", power)).boxed(),
                                gpu_graph == ProcGraphKind::Power,
                                false,
                                Message::GpuGraph(gpu.id, ProcGraphKind::Power),
                            ));
                        }

                        if let Some(temp) = gpu.temp {
                            second.push(tag::tag(
                                text(format!("{:.1}°C", temp)).boxed(),
                                gpu_graph == ProcGraphKind::Temperature,
                                false,
                                Message::GpuGraph(gpu.id, ProcGraphKind::Temperature),
                            ));
                        }

                        if !second.is_empty() {
                            data.push(row(second).spacing(theme::SPACE_XXXS).boxed());
                        }

                        (
                            column(data).spacing(theme::SPACE_XXXS).boxed(),
                            Some(ProcessCategory::GpuUsage(gpu.id, Some(gpu_i))),
                        )
                    }
                    GpuState::Suspended => (text(fl!("gpu-suspended-title")).boxed(), None),
                };

                items.push(self.graph_card(
                    GraphKind::Gpu(gpu.id, gpu_graph),
                    fl!("gpu-index", index = gpu_i),
                    gpu.name.clone(),
                    data,
                    process_category,
                    Message::GpuSelect(gpu.name.clone()),
                ));
            }

            if let (Some(vram_used), Some(vram_total)) = (gpu.vram_used, gpu.vram_total) {
                let (data, process_category): (Element<'a, Message>, _) = match gpu.state {
                    GpuState::Active | GpuState::Idle(_) => (
                        column![
                            text(format!(
                                "{:.1}%",
                                100.0 * (vram_used as f32) / (vram_total as f32),
                            )),
                            text(format!(
                                "{} / {}",
                                humansize::format_size(vram_used, humansize::BINARY),
                                humansize::format_size(vram_total, humansize::BINARY),
                            )),
                        ]
                        .spacing(theme::SPACE_XXXS)
                        .boxed(),
                        Some(ProcessCategory::GpuVram(gpu.id, Some(gpu_i))),
                    ),
                    GpuState::Suspended => (text(fl!("gpu-suspended-title")).boxed(), None),
                };

                items.push(self.graph_card(
                    GraphKind::GpuVram(gpu.id),
                    fl!("gpu-vram-index", index = gpu_i),
                    gpu.name.clone(),
                    data,
                    process_category,
                    Message::GpuSelect(gpu.name.clone()),
                ));
            }
        }

        items
    }

    fn graph_card<'a>(
        &'a self,
        graph_kind: GraphKind<'a>,
        name: String,
        caption: String,
        data: Element<'a, Message>,
        process_category: Option<ProcessCategory>,
        message: Message,
    ) -> Element<'a, Message> {
        let mut details: Vec<Element<'a, Message>> = Vec::with_capacity(5);

        details.push(
            column![
                text(name).size(16),
                text(caption).wrapping(iced::widget::text::Wrapping::None)
                    .ellipsis(iced::widget::text::Ellipsis::End),
            ]
            .boxed(),
        );
        details.push(
            container(data)
                .height(Length::Fixed(CARD_DATA_HEIGHT))
                .boxed(),
        );

        let mut handled = false;

        if let Some(sort_category) = process_category
            && let Some(item) = self
                .processes
                .iter()
                .min_by(|a, b| a.compare(b, sort_category))
        {
            handled = true;

            let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(3);

            if let Some(icon) = table::ItemInterface::get_icon(item, ProcessCategory::App) {
                children.push(icons::view(&icon, 20));
            }

            children.push(
                container(text(item.name.as_str()).wrapping(iced::widget::text::Wrapping::None)
                    .ellipsis(iced::widget::text::Ellipsis::End))
                    .align_x(Horizontal::Left)
                    .align_y(Alignment::Center)
                    .width(Length::Fill)
                    .boxed(),
            );
            children.push(
                container(text(item.text(sort_category)))
                    .align_x(Horizontal::Right)
                    .align_y(Alignment::Center)
                    .width(Length::Shrink)
                    .boxed(),
            );

            details.push(rule::horizontal(1).boxed());
            details.push(row(children).align_y(Alignment::Center).boxed());
            details.push(rule::horizontal(1).boxed());
        }

        if !handled {
            match graph_kind {
                GraphKind::Gpu(gpu_id, _) | GraphKind::GpuVram(gpu_id) => {
                    if let Some(gpu) = graph_item_gpu(self.graph_snapshot.as_ref(), gpu_id) {
                        details.push(rule::horizontal(1).boxed());
                        details.push(
                            text(match gpu.state {
                                GpuState::Suspended => fl!("gpu-suspended-description"),
                                _ => String::new(),
                            })
                            .boxed(),
                        );
                        details.push(rule::horizontal(1).boxed());
                    }
                }
                GraphKind::NetworkTotal => {
                    if let Some((network_name, io)) =
                        top_network(self.graph_snapshot.as_ref())
                    {
                        let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(2);

                        children.push(
                            container(
                                text(network_name.to_string())
                                    .wrapping(iced::widget::text::Wrapping::None)
                    .ellipsis(iced::widget::text::Ellipsis::End),
                            )
                            .align_x(Horizontal::Left)
                            .align_y(Alignment::Center)
                            .width(Length::Fill)
                            .boxed(),
                        );
                        children.push(
                            container(text(format!(
                                "{}/s",
                                humansize::format_size(io, humansize::DECIMAL)
                            )))
                            .align_x(Horizontal::Right)
                            .align_y(Alignment::Center)
                            .width(Length::Shrink)
                            .boxed(),
                        );

                        details.push(rule::horizontal(1).boxed());
                        details.push(row(children).align_y(Alignment::Center).boxed());
                        details.push(rule::horizontal(1).boxed());
                    }
                }
                _ => {}
            }
        }

        details.push(
            button(text(fl!("details")))
                .style(flat_button)
                .on_press(message)
                .boxed(),
        );

        card(
            row![
                iced::widget::canvas(Graph::new(graph_kind, &self.graph_history).border())
                    .height(Length::Fixed(SMALL_GRAPH_HEIGHT))
                    .width(Length::Fill),
                column(details).spacing(theme::SPACE_XXS).width(Length::Fill),
            ]
            .spacing(theme::SPACE_XS)
            .width(Length::Fill)
            .boxed(),
        )
    }

    fn top_processes_by<'a>(
        &'a self,
        show_apps: bool,
        sort_category: ProcessCategory,
        sort_direction: SortDirection,
        sortable: bool,
        count: usize,
    ) -> Element<'a, Message> {
        let categories = ProcessCategory::for_top_processes(sort_category);
        let items = if show_apps {
            &self.apps
        } else {
            &self.processes
        };
        let selected = self.selected.clone();

        let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(count * 2 + 2);

        rows.push(table::header(
            &categories,
            sort_category,
            sort_direction,
            sortable,
            Message::ProcessSort,
        ));

        for item in items
            .iter()
            .filter(|x| !x.text(sort_category).is_empty())
            .k_smallest_by(count, |a, b| {
                if sort_direction.is_descending() {
                    b.compare(a, sort_category)
                } else {
                    a.compare(b, sort_category)
                }
            })
        {
            rows.push(table::divider());
            rows.push(self.view_row(item, &categories, selected.as_ref()));
        }

        rows.push(table::divider());

        column![
            text(if show_apps {
                fl!("applications")
            } else {
                fl!("processes")
            })
            .size(16),
            column(rows).width(Length::Fill),
            button(text(if show_apps {
                fl!("see-all-applications")
            } else {
                fl!("see-all-processes")
            }))
            .style(flat_button)
            .on_press(Message::SeeAllProcesses(
                show_apps,
                sort_category,
                sort_direction
            )),
        ]
        .spacing(theme::SPACE_XS)
        .width(Length::Fill)
        .boxed()
    }

    fn graph_with_top_processes<'a>(
        &'a self,
        category: ProcessCategory,
        graph: Element<'a, Message>,
    ) -> Element<'a, Message> {
        if self.content_size().width > MIN_GRAPH_WIDTH + theme::SPACE_XXL + MIN_PROCESSES_WIDTH {
            row![
                graph,
                container(self.top_processes_by(false, category, self.process_sort.1, false, 7))
                    .width(Length::Fixed(MIN_PROCESSES_WIDTH)),
            ]
            .spacing(theme::SPACE_XXL)
            .boxed()
        } else {
            column![
                graph,
                self.top_processes_by(false, category, self.process_sort.1, false, 5),
            ]
            .spacing(theme::SPACE_L)
            .boxed()
        }
    }

    fn view_cpu<'a>(&'a self, graph_item: &'a GraphItem) -> Element<'a, Message> {
        let cpu_graph = self.cpu_graph;
        let mut tags: Vec<Element<'a, Message>> = Vec::with_capacity(3);

        tags.push(tag::tag(
            column![
                text(fl!("utilization")),
                text(format!("{:.1}%", graph_item.total_cpu_usage())).size(20),
            ]
            .boxed(),
            cpu_graph == ProcGraphKind::Utilization,
            true,
            Message::CpuGraph(ProcGraphKind::Utilization),
        ));
        tags.push(tag::tag(
            column![
                text(fl!("speed")),
                text(format_frequency(graph_item.max_cpu_frequency())).size(20),
            ]
            .boxed(),
            cpu_graph == ProcGraphKind::Frequency,
            true,
            Message::CpuGraph(ProcGraphKind::Frequency),
        ));

        if let Some(temp) = graph_item.max_cpu_temp() {
            tags.push(tag::tag(
                column![
                    text(fl!("temperature")),
                    text(format!("{:.1}°C", temp)).size(20),
                ]
                .boxed(),
                cpu_graph == ProcGraphKind::Temperature,
                true,
                Message::CpuGraph(ProcGraphKind::Temperature),
            ));
        }

        let graph = column![
            text(fl!("overall-utilization")).size(16),
            row(tags).spacing(theme::SPACE_XXS),
            iced::widget::canvas(
                Graph::new(GraphKind::Cpu(cpu_graph), &self.graph_history).legend()
            )
            .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
            .width(Length::Fill),
        ]
        .spacing(theme::SPACE_XXS)
        .width(Length::Fill);

        let available = (self.content_size().width - theme::SPACE_XL * 2.0).max(CORE_WIDTH);
        let per_row = ((available / (CORE_WIDTH + theme::SPACE_M)).floor() as usize).max(1);

        let mut core_rows: Vec<Element<'a, Message>> =
            Vec::with_capacity(graph_item.cpus.len() / per_row + 1);

        for chunk in graph_item.cpus.chunks(per_row) {
            let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(chunk.len());

            for cpu in chunk {
                cells.push(
                    column![
                        row![
                            text(cpu.name.as_str())
                                .size(14)
                                .width(Length::Fixed(CORE_NAME_WIDTH)),
                            text(format_frequency(cpu.frequency))
                                .width(Length::Fixed(CORE_VALUE_WIDTH))
                                .align_x(Horizontal::Right),
                        ]
                        .width(Length::Fixed(CORE_WIDTH)),
                        row![
                            progress_bar(0.0..=1.0, cpu.usage / 100.0)
                                .girth(Length::Fixed(12.0))
                                .length(Length::Fixed(
                                    CORE_WIDTH - CORE_VALUE_WIDTH - theme::SPACE_XS
                                )),
                            text(format!("{:.1}%", cpu.usage))
                                .width(Length::Fixed(CORE_VALUE_WIDTH))
                                .align_x(Horizontal::Right),
                        ]
                        .align_y(Alignment::Center)
                        .spacing(theme::SPACE_XS)
                        .width(Length::Fixed(CORE_WIDTH)),
                    ]
                    .spacing(theme::SPACE_XXS)
                    .width(Length::Fixed(CORE_WIDTH))
                    .boxed(),
                );
            }

            core_rows.push(
                row(cells)
                    .spacing(theme::SPACE_M)
                    .width(Length::Fill)
                    .boxed(),
            );
        }

        scrollable(
            column![
                self.page_header(fl!("cpu"), true),
                self.graph_with_top_processes(ProcessCategory::CPU, graph.boxed()),
                column![
                    text(fl!("utilization-per-core")).size(16),
                    column(core_rows)
                        .spacing(theme::SPACE_S)
                        .width(Length::Fill),
                ]
                .spacing(theme::SPACE_XXS),
            ]
            .spacing(theme::SPACE_L)
            .padding(theme::PAGE_PADDING)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    fn view_memory<'a>(&'a self, graph_item: &'a GraphItem) -> Element<'a, Message> {
        let mem = &graph_item.memory;
        let total_used = mem.used + mem.cache;

        let graph = column![
            text(fl!("memory-usage")).size(16),
            row![
                labeled_value(
                    fl!("capacity"),
                    humansize::format_size(mem.total, humansize::BINARY)
                ),
                labeled_value(
                    fl!("in-use"),
                    format!(
                        "{} ({:.1}%)",
                        humansize::format_size(mem.used, humansize::BINARY),
                        100.0 * (mem.used as f64) / (mem.total as f64)
                    )
                ),
                labeled_value(
                    fl!("cache"),
                    format!(
                        "{} ({:.1}%)",
                        humansize::format_size(mem.cache, humansize::BINARY),
                        100.0 * (mem.cache as f64) / (mem.total as f64)
                    )
                ),
                labeled_value(
                    fl!("total-utilization"),
                    format!(
                        "{} ({:.1}%)",
                        humansize::format_size(total_used, humansize::BINARY),
                        100.0 * (total_used as f64) / (mem.total as f64)
                    )
                ),
            ]
            .spacing(theme::SPACE_M),
            iced::widget::canvas(Graph::new(GraphKind::Memory, &self.graph_history).legend())
                .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                .width(Length::Fill),
        ]
        .spacing(theme::SPACE_XXS)
        .width(Length::Fill);

        let swap = column![
            text(fl!("swap-usage")).size(16),
            row![
                labeled_value(
                    fl!("capacity"),
                    humansize::format_size(mem.swap_total, humansize::BINARY)
                ),
                labeled_value(
                    fl!("in-use"),
                    format!(
                        "{} ({:.1}%)",
                        humansize::format_size(mem.swap_used, humansize::BINARY),
                        100.0 * (mem.swap_used as f64) / (mem.swap_total as f64)
                    )
                ),
            ]
            .spacing(theme::SPACE_M),
            iced::widget::canvas(Graph::new(GraphKind::Swap, &self.graph_history).legend())
                .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                .width(Length::Fill),
        ]
        .spacing(theme::SPACE_XXS)
        .width(Length::Fill);

        scrollable(
            column![
                self.page_header(fl!("memory"), true),
                self.graph_with_top_processes(ProcessCategory::Memory, graph.boxed()),
                swap,
            ]
            .spacing(theme::SPACE_L)
            .padding(theme::PAGE_PADDING)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    fn view_gpu<'a>(&'a self, graph_item: &'a GraphItem) -> Element<'a, Message> {
        let Some((gpu_i, gpu)) = graph_item
            .gpus
            .iter()
            .enumerate()
            .find(|(_, gpu)| {
                self.gpu_id_opt == Some(gpu.id) || (self.gpu_id_opt.is_none() && gpu.boot_vga)
            })
            .or_else(|| graph_item.gpus.first().map(|gpu| (0, gpu)))
        else {
            return column![
                self.page_header(fl!("gpu"), true),
                text(fl!("no-gpus")).size(16),
            ]
            .spacing(theme::SPACE_L)
            .padding(theme::PAGE_PADDING)
            .boxed();
        };

        let picker = pick_list(
            Some(gpu.name.clone()),
            self.gpu_names.clone(),
            |name: &String| name.clone(),
        )
        .width(Length::Fixed(320.0))
        .on_select(Message::GpuSelect);

        let mut content: Vec<Element<'a, Message>> = vec![
            self.page_header(fl!("gpu"), true),
            container(picker).width(Length::Fill).boxed(),
        ];

        let gpu_graph = self.gpu_graphs.get(&gpu.id).copied().unwrap_or_default();

        match gpu.state {
            GpuState::Active | GpuState::Idle(_) => {
                if let Some(usage) = gpu.usage {
                    let mut tags: Vec<Element<'a, Message>> = Vec::with_capacity(4);

                    tags.push(tag::tag(
                        column![
                            text(fl!("utilization")),
                            text(format!("{:.1}%", usage)).size(20),
                        ]
                        .boxed(),
                        gpu_graph == ProcGraphKind::Utilization,
                        true,
                        Message::GpuGraph(gpu.id, ProcGraphKind::Utilization),
                    ));

                    if let Some(frequency) = gpu.frequency {
                        tags.push(tag::tag(
                            column![
                                text(fl!("speed")),
                                text(format_frequency(frequency)).size(20),
                            ]
                            .boxed(),
                            gpu_graph == ProcGraphKind::Frequency,
                            true,
                            Message::GpuGraph(gpu.id, ProcGraphKind::Frequency),
                        ));
                    }

                    if let Some(power) = gpu.power {
                        tags.push(tag::tag(
                            column![
                                text(fl!("power")),
                                text(format!("{:.1} W", power)).size(20),
                            ]
                            .boxed(),
                            gpu_graph == ProcGraphKind::Power,
                            true,
                            Message::GpuGraph(gpu.id, ProcGraphKind::Power),
                        ));
                    }

                    if let Some(temp) = gpu.temp {
                        tags.push(tag::tag(
                            column![
                                text(fl!("temperature")),
                                text(format!("{:.1}°C", temp)).size(20),
                            ]
                            .boxed(),
                            gpu_graph == ProcGraphKind::Temperature,
                            true,
                            Message::GpuGraph(gpu.id, ProcGraphKind::Temperature),
                        ));
                    }

                    let graph = column![
                        text(fl!("gpu-utilization")).size(16),
                        row(tags).spacing(theme::SPACE_XXS),
                        iced::widget::canvas(
                            Graph::new(GraphKind::Gpu(gpu.id, gpu_graph), &self.graph_history)
                                .legend()
                        )
                        .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                        .width(Length::Fill),
                    ]
                    .spacing(theme::SPACE_XXS)
                    .width(Length::Fill);

                    content.push(self.graph_with_top_processes(
                        ProcessCategory::GpuUsage(gpu.id, Some(gpu_i)),
                        graph.boxed(),
                    ));
                }

                if let (Some(vram_used), Some(vram_total)) = (gpu.vram_used, gpu.vram_total) {
                    let graph = column![
                        text(fl!("gpu-vram")).size(16),
                        row![
                            labeled_value(
                                fl!("capacity"),
                                humansize::format_size(vram_total, humansize::BINARY)
                            ),
                            labeled_value(
                                fl!("vram"),
                                format!(
                                    "{} ({:.1}%)",
                                    humansize::format_size(vram_used, humansize::BINARY),
                                    100.0 * (vram_used as f64) / (vram_total as f64)
                                )
                            ),
                        ]
                        .spacing(theme::SPACE_M),
                        iced::widget::canvas(
                            Graph::new(GraphKind::GpuVram(gpu.id), &self.graph_history).legend()
                        )
                        .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                        .width(Length::Fill),
                    ]
                    .spacing(theme::SPACE_XXS)
                    .width(Length::Fill);

                    content.push(self.graph_with_top_processes(
                        ProcessCategory::GpuVram(gpu.id, Some(gpu_i)),
                        graph.boxed(),
                    ));
                }
            }
            GpuState::Suspended => {
                content.push(
                    column![
                        text(fl!("gpu-suspended-title")).size(16),
                        text(fl!("gpu-suspended-description")),
                    ]
                    .boxed(),
                );
            }
        }

        scrollable(
            column(content)
                .spacing(theme::SPACE_L)
                .padding(theme::PAGE_PADDING)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    fn view_disk<'a>(&'a self, graph_item: &'a GraphItem) -> Element<'a, Message> {
        let all_used = graph_item.disks.iter().fold(0, |x, disk| x + disk.used);
        let all_total = graph_item.disks.iter().fold(0, |x, disk| x + disk.total);
        let all_io = graph_item.total_disk_io();

        let all_graph = column![
            text(fl!("all-disks")).size(16),
            row![
                labeled_value(
                    fl!("capacity"),
                    humansize::format_size(all_total, humansize::BINARY)
                ),
                labeled_value(
                    fl!("in-use"),
                    format!(
                        "{} ({:.1}%)",
                        humansize::format_size(all_used, humansize::BINARY),
                        100.0 * (all_used as f64) / (all_total as f64)
                    )
                ),
                labeled_value(
                    fl!("reading"),
                    format!(
                        "{}/s",
                        humansize::format_size(all_io.0 as u64, humansize::DECIMAL)
                    )
                ),
                labeled_value(
                    fl!("writing"),
                    format!(
                        "{}/s",
                        humansize::format_size(all_io.1 as u64, humansize::DECIMAL)
                    )
                ),
            ]
            .spacing(theme::SPACE_M),
            iced::widget::canvas(Graph::new(GraphKind::DiskTotal, &self.graph_history).legend())
                .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                .width(Length::Fill),
        ]
        .spacing(theme::SPACE_XXS)
        .width(Length::Fill);

        let wide = self.content_size().width > MIN_GRAPH_WIDTH * 2.0 + theme::SPACE_XXL;
        let mut content: Vec<Element<'a, Message>> = vec![
            self.page_header(fl!("disk"), true),
            self.graph_with_top_processes(ProcessCategory::DiskTotal, all_graph.boxed()),
        ];

        for disk in graph_item.disks.iter() {
            let mut info: Vec<Element<'a, Message>> = vec![
                labeled_value(fl!("mount-path"), disk.mount_path.clone()),
                labeled_value(
                    fl!("capacity"),
                    humansize::format_size(disk.total, humansize::BINARY),
                ),
                labeled_value(
                    fl!("in-use"),
                    format!(
                        "{} ({:.1}%)",
                        humansize::format_size(disk.used, humansize::BINARY),
                        100.0 * (disk.used as f64) / (disk.total as f64)
                    ),
                ),
                labeled_value(
                    fl!("reading"),
                    format!(
                        "{}/s",
                        humansize::format_size(disk.read as u64, humansize::DECIMAL)
                    ),
                ),
                labeled_value(
                    fl!("writing"),
                    format!(
                        "{}/s",
                        humansize::format_size(disk.write as u64, humansize::DECIMAL)
                    ),
                ),
            ];

            if let Some(temp) = disk.temp {
                info.push(labeled_value(fl!("temperature"), format!("{:.1}°C", temp)));
            }

            let name = disk.name.as_str();

            let reading = column![
                text(fl!("reading")).size(16),
                iced::widget::canvas(
                    Graph::new(GraphKind::DiskRead(name), &self.graph_history).legend()
                )
                .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                .width(Length::Fill),
            ]
            .spacing(theme::SPACE_XXS)
            .width(Length::Fill);

            let writing = column![
                text(fl!("writing")).size(16),
                iced::widget::canvas(
                    Graph::new(GraphKind::DiskWrite(name), &self.graph_history).legend()
                )
                .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                .width(Length::Fill),
            ]
            .spacing(theme::SPACE_XXS)
            .width(Length::Fill);

            let graphs: Element<'a, Message> = if wide {
                row![reading, writing]
                    .spacing(theme::SPACE_XXL)
                    .boxed()
            } else {
                column![reading, writing]
                    .spacing(theme::SPACE_XXS)
                    .boxed()
            };

            content.push(
                column![
                    text(disk.name.as_str()).size(16),
                    row(info).spacing(theme::SPACE_M),
                    graphs,
                ]
                .spacing(theme::SPACE_XXS)
                .width(Length::Fill)
                .boxed(),
            );
        }

        scrollable(
            column(content)
                .spacing(theme::SPACE_L)
                .padding(theme::PAGE_PADDING)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    fn view_network<'a>(&'a self, graph_item: &'a GraphItem) -> Element<'a, Message> {
        let all_io = graph_item.total_network_io();

        let all_graph = column![
            text(fl!("all-networks")).size(16),
            row![
                labeled_value(
                    fl!("receiving"),
                    format!(
                        "{}/s",
                        humansize::format_size(all_io.0 as u64, humansize::DECIMAL)
                    )
                ),
                labeled_value(
                    fl!("sending"),
                    format!(
                        "{}/s",
                        humansize::format_size(all_io.1 as u64, humansize::DECIMAL)
                    )
                ),
            ]
            .spacing(theme::SPACE_M),
            iced::widget::canvas(
                Graph::new(GraphKind::NetworkTotal, &self.graph_history).legend()
            )
            .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
            .width(Length::Fill),
        ]
        .spacing(theme::SPACE_XXS)
        .width(Length::Fill);

        let wide = self.content_size().width > 800.0;
        let mut content: Vec<Element<'a, Message>> =
            vec![self.page_header(fl!("network"), true), all_graph.boxed()];

        for net in graph_item.networks.iter() {
            let name = net.name.as_str();

            let receiving = column![
                text(fl!("receiving")).size(16),
                iced::widget::canvas(
                    Graph::new(GraphKind::NetworkRx(name), &self.graph_history).legend()
                )
                .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                .width(Length::Fill),
            ]
            .spacing(theme::SPACE_XXS)
            .width(Length::Fill);

            let sending = column![
                text(fl!("sending")).size(16),
                iced::widget::canvas(
                    Graph::new(GraphKind::NetworkTx(name), &self.graph_history).legend()
                )
                .height(Length::Fixed(LARGE_GRAPH_HEIGHT))
                .width(Length::Fill),
            ]
            .spacing(theme::SPACE_XXS)
            .width(Length::Fill);

            let graphs: Element<'a, Message> = if wide {
                row![receiving, sending].boxed()
            } else {
                column![receiving, sending].boxed()
            };

            content.push(
                column![
                    text(net.name.as_str()).size(16),
                    row![
                        labeled_value(
                            fl!("receiving"),
                            format!(
                                "{}/s",
                                humansize::format_size(net.rx as u64, humansize::DECIMAL)
                            )
                        ),
                        labeled_value(
                            fl!("sending"),
                            format!(
                                "{}/s",
                                humansize::format_size(net.tx as u64, humansize::DECIMAL)
                            )
                        ),
                    ]
                    .spacing(theme::SPACE_M),
                    graphs,
                ]
                .spacing(theme::SPACE_XXS)
                .width(Length::Fill)
                .boxed(),
            );
        }

        scrollable(
            column(content)
                .spacing(theme::SPACE_L)
                .padding(theme::PAGE_PADDING)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }
}

fn graph_item_gpu(graph_item: Option<&GraphItem>, gpu_id: GpuId) -> Option<&crate::info::GpuItem> {
    graph_item?.gpus.iter().find(|gpu| gpu.id == gpu_id)
}

fn top_network(graph_item: Option<&GraphItem>) -> Option<(&str, u64)> {
    graph_item?
        .networks
        .iter()
        .map(|x| (x.name.as_str(), (x.rx + x.tx) as u64))
        .max_by(|a, b| a.1.cmp(&b.1))
}

fn labeled_value(label: String, value: String) -> Element<'static, Message> {
    column![text(label), text(value).size(18)]
        .spacing(theme::SPACE_XXXS)
        .boxed()
}

/// Captures the raw Wayland handles of the main window, so that a native
/// `xdg_popup` can be parented to it.
fn capture_window_handle() -> Task<Message> {
    iced::window::latest().and_then(|id| {
        iced::window::run(id, |window| {
            use iced::window::raw_window_handle::{RawDisplayHandle, RawWindowHandle};

            let (Ok(display), Ok(handle)) = (window.display_handle(), window.window_handle())
            else {
                return None;
            };

            if let (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(handle)) =
                (display.as_raw(), handle.as_raw())
            {
                Some((
                    display.display.as_ptr() as usize,
                    handle.surface.as_ptr() as usize,
                ))
            } else {
                None
            }
        })
        .map(Message::WindowHandle)
    })
}

fn search_regex(search: &str) -> Option<Regex> {
    if search.is_empty() {
        return None;
    }

    RegexBuilder::new(&regex::escape(search))
        .case_insensitive(true)
        .build()
        .ok()
}

fn format_frequency(mhz: u64) -> String {
    if mhz >= 1000 {
        format!("{:.2} GHz", (mhz as f64) / 1000.0)
    } else {
        format!("{} MHz", mhz)
    }
}

fn kill(pid: Pid, force: bool) {
    #[cfg(unix)]
    {
        if let Ok(pid) = pid.as_u32().try_into() {
            let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
            unsafe {
                libc::kill(pid, signal);
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (pid, force);
    }
}

fn flat_button(iced_theme: &Theme, status: button::Status) -> button::Style {
    let palette = iced_theme.palette();

    let mut style = button::Style {
        background: None,
        text_color: palette.primary.base.color,
        border: iced::border::rounded(4),
        ..button::Style::default()
    };

    match status {
        button::Status::Hovered | button::Status::Pressed => {
            style.background = Some(palette.background.strong.color.into());
            style.text_color = palette.primary.strong.color;
        }
        _ => {}
    }

    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_regex_escapes_its_input() {
        let regex = search_regex("a.b").expect("a search term builds a pattern");

        assert!(regex.is_match("xa.by"));
        assert!(!regex.is_match("axby"));
    }

    #[test]
    fn an_empty_search_has_no_pattern() {
        assert!(search_regex("").is_none());
    }
}
