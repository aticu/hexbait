//! Implements the different modules that are shown in the GUI.

use egui_dock::{DockState, NodeIndex};
use hexbait_common::Input;

use crate::state::State;

pub mod content;
pub mod hex;
pub mod inspector;
pub mod parsed_value;
pub mod scrollbars;
pub mod search;
pub mod settings;
pub mod statistics_display;

/// The different tab types in the hexbait application.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TabType {
    /// Shows the content of the input.
    ///
    /// This includes the scrollbar and either a hexview or a statistics view depending on the zoom
    /// level.
    Content,
    /// Shows detailed views of the currently selected data.
    Inspector,
    /// Shows the parsed value.
    ParsedValue,
    /// Shows settings.
    Settings,
    /// Shows search controls.
    Search,
}

/// The context for the hexbait application.
pub struct Context {
    /// The state of the application.
    pub state: State,
    /// The input of the application.
    pub input: Input,
}

impl egui_dock::TabViewer for Context {
    type Tab = TabType;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        format!("{tab:?}").into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        self.state.settings.apply_settings_to_ui(ui);

        let show_fn = match tab {
            TabType::Content => content::show,
            TabType::Inspector => inspector::show,
            TabType::ParsedValue => parsed_value::show,
            TabType::Settings => settings::show,
            TabType::Search => search::show,
        };

        show_fn(ui, &mut self.state, &self.input);
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        matches!(tab, TabType::Settings | TabType::Search)
    }

    fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
        match tab {
            TabType::Content => [true, false],
            _ => [true, true],
        }
    }
}

/// The dock state for a hex view.
pub fn hex_dock_state() -> DockState<TabType> {
    let mut dock_state = DockState::new(vec![TabType::Content]);

    let surface = dock_state.main_surface_mut();

    let [_, inspector_node] =
        surface.split_right(NodeIndex::root(), 0.75, vec![TabType::Inspector]);

    surface.split_below(inspector_node, 0.5, vec![TabType::ParsedValue]);

    dock_state
}
