use knuffel::{errors::DecodeError, span::Spanned};
use niri_config::WorkspaceReference;
use niri_ipc::WorkspaceReferenceArg;

pub fn config_to_ipc_action<T, S>(
    action: niri_config::Action,
    span: &Spanned<T, S>,
) -> Result<niri_ipc::Action, DecodeError<S>>
where
    S: knuffel::traits::ErrorSpan,
{
    use niri_config::Action as ConfigAction;
    use niri_ipc::Action as IpcAction;

    match action {
        ConfigAction::Quit(skip_confirmation) => {
            Ok(IpcAction::Quit { skip_confirmation })
        }
        ConfigAction::PowerOffMonitors => Ok(IpcAction::PowerOffMonitors {}),
        ConfigAction::PowerOnMonitors => Ok(IpcAction::PowerOnMonitors {}),
        ConfigAction::Spawn(command) => Ok(IpcAction::Spawn { command }),
        ConfigAction::SpawnSh(command) => Ok(IpcAction::SpawnSh { command }),
        ConfigAction::DoScreenTransition(delay_ms) => {
            Ok(IpcAction::DoScreenTransition { delay_ms })
        }
        ConfigAction::Screenshot(show_pointer, path) => {
            Ok(IpcAction::Screenshot { show_pointer, path })
        }
        ConfigAction::ScreenshotScreen(write_to_disk, show_pointer, path) => {
            Ok(IpcAction::ScreenshotScreen {
                write_to_disk,
                show_pointer,
                path,
            })
        }
        ConfigAction::ScreenshotWindow(write_to_disk, path) => {
            Ok(IpcAction::ScreenshotWindow {
                id: None,
                write_to_disk,
                path,
            })
        }
        ConfigAction::ScreenshotWindowById {
            id,
            write_to_disk,
            path,
        } => Ok(IpcAction::ScreenshotWindow {
            id: Some(id),
            write_to_disk,
            path,
        }),
        ConfigAction::ToggleKeyboardShortcutsInhibit => {
            Ok(IpcAction::ToggleKeyboardShortcutsInhibit {})
        }
        ConfigAction::CloseWindow => Ok(IpcAction::CloseWindow { id: None }),
        ConfigAction::CloseWindowById(id) => {
            Ok(IpcAction::CloseWindow { id: Some(id) })
        }
        ConfigAction::FullscreenWindow => {
            Ok(IpcAction::FullscreenWindow { id: None })
        }
        ConfigAction::FullscreenWindowById(id) => {
            Ok(IpcAction::FullscreenWindow { id: Some(id) })
        }
        ConfigAction::ToggleWindowedFullscreen => {
            Ok(IpcAction::ToggleWindowedFullscreen { id: None })
        }
        ConfigAction::ToggleWindowedFullscreenById(id) => {
            Ok(IpcAction::ToggleWindowedFullscreen { id: Some(id) })
        }
        ConfigAction::FocusWindow(id) => Ok(IpcAction::FocusWindow { id }),
        ConfigAction::FocusWindowInColumn(index) => {
            Ok(IpcAction::FocusWindowInColumn { index })
        }
        ConfigAction::FocusWindowPrevious => {
            Ok(IpcAction::FocusWindowPrevious {})
        }
        ConfigAction::FocusColumnLeft => Ok(IpcAction::FocusColumnLeft {}),
        ConfigAction::FocusColumnRight => Ok(IpcAction::FocusColumnRight {}),
        ConfigAction::FocusColumnFirst => Ok(IpcAction::FocusColumnFirst {}),
        ConfigAction::FocusColumnLast => Ok(IpcAction::FocusColumnLast {}),
        ConfigAction::FocusColumnRightOrFirst => {
            Ok(IpcAction::FocusColumnRightOrFirst {})
        }
        ConfigAction::FocusColumnLeftOrLast => {
            Ok(IpcAction::FocusColumnLeftOrLast {})
        }
        ConfigAction::FocusColumn(index) => {
            Ok(IpcAction::FocusColumn { index })
        }
        ConfigAction::FocusWindowOrMonitorUp => {
            Ok(IpcAction::FocusWindowOrMonitorUp {})
        }
        ConfigAction::FocusWindowOrMonitorDown => {
            Ok(IpcAction::FocusWindowOrMonitorDown {})
        }
        ConfigAction::FocusColumnOrMonitorLeft => {
            Ok(IpcAction::FocusColumnOrMonitorLeft {})
        }
        ConfigAction::FocusColumnOrMonitorRight => {
            Ok(IpcAction::FocusColumnOrMonitorRight {})
        }
        ConfigAction::FocusWindowDown => Ok(IpcAction::FocusWindowDown {}),
        ConfigAction::FocusWindowUp => Ok(IpcAction::FocusWindowUp {}),
        ConfigAction::FocusWindowDownOrColumnLeft => {
            Ok(IpcAction::FocusWindowDownOrColumnLeft {})
        }
        ConfigAction::FocusWindowDownOrColumnRight => {
            Ok(IpcAction::FocusWindowDownOrColumnRight {})
        }
        ConfigAction::FocusWindowUpOrColumnLeft => {
            Ok(IpcAction::FocusWindowUpOrColumnLeft {})
        }
        ConfigAction::FocusWindowUpOrColumnRight => {
            Ok(IpcAction::FocusWindowUpOrColumnRight {})
        }
        ConfigAction::FocusWindowOrWorkspaceDown => {
            Ok(IpcAction::FocusWindowOrWorkspaceDown {})
        }
        ConfigAction::FocusWindowOrWorkspaceUp => {
            Ok(IpcAction::FocusWindowOrWorkspaceUp {})
        }
        ConfigAction::FocusWindowTop => Ok(IpcAction::FocusWindowTop {}),
        ConfigAction::FocusWindowBottom => Ok(IpcAction::FocusWindowBottom {}),
        ConfigAction::FocusWindowDownOrTop => {
            Ok(IpcAction::FocusWindowDownOrTop {})
        }
        ConfigAction::FocusWindowUpOrBottom => {
            Ok(IpcAction::FocusWindowUpOrBottom {})
        }
        ConfigAction::MoveColumnLeft => Ok(IpcAction::MoveColumnLeft {}),
        ConfigAction::MoveColumnRight => Ok(IpcAction::MoveColumnRight {}),
        ConfigAction::MoveColumnToFirst => Ok(IpcAction::MoveColumnToFirst {}),
        ConfigAction::MoveColumnToLast => Ok(IpcAction::MoveColumnToLast {}),
        ConfigAction::MoveColumnToIndex(index) => {
            Ok(IpcAction::MoveColumnToIndex { index })
        }
        ConfigAction::MoveColumnLeftOrToMonitorLeft => {
            Ok(IpcAction::MoveColumnLeftOrToMonitorLeft {})
        }
        ConfigAction::MoveColumnRightOrToMonitorRight => {
            Ok(IpcAction::MoveColumnRightOrToMonitorRight {})
        }
        ConfigAction::MoveWindowDown => Ok(IpcAction::MoveWindowDown {}),
        ConfigAction::MoveWindowUp => Ok(IpcAction::MoveWindowUp {}),
        ConfigAction::MoveWindowDownOrToWorkspaceDown => {
            Ok(IpcAction::MoveWindowDownOrToWorkspaceDown {})
        }
        ConfigAction::MoveWindowUpOrToWorkspaceUp => {
            Ok(IpcAction::MoveWindowUpOrToWorkspaceUp {})
        }
        ConfigAction::ConsumeOrExpelWindowLeft => {
            Ok(IpcAction::ConsumeOrExpelWindowLeft { id: None })
        }
        ConfigAction::ConsumeOrExpelWindowLeftById(id) => {
            Ok(IpcAction::ConsumeOrExpelWindowLeft { id: Some(id) })
        }
        ConfigAction::ConsumeOrExpelWindowRight => {
            Ok(IpcAction::ConsumeOrExpelWindowRight { id: None })
        }
        ConfigAction::ConsumeOrExpelWindowRightById(id) => {
            Ok(IpcAction::ConsumeOrExpelWindowRight { id: Some(id) })
        }
        ConfigAction::ConsumeWindowIntoColumn => {
            Ok(IpcAction::ConsumeWindowIntoColumn {})
        }
        ConfigAction::ExpelWindowFromColumn => {
            Ok(IpcAction::ExpelWindowFromColumn {})
        }
        ConfigAction::SwapWindowRight => Ok(IpcAction::SwapWindowRight {}),
        ConfigAction::SwapWindowLeft => Ok(IpcAction::SwapWindowLeft {}),
        ConfigAction::ToggleColumnTabbedDisplay => {
            Ok(IpcAction::ToggleColumnTabbedDisplay {})
        }
        ConfigAction::SetColumnDisplay(display) => {
            Ok(IpcAction::SetColumnDisplay { display })
        }
        ConfigAction::CenterColumn => Ok(IpcAction::CenterColumn {}),
        ConfigAction::CenterWindow => Ok(IpcAction::CenterWindow { id: None }),
        ConfigAction::CenterWindowById(id) => {
            Ok(IpcAction::CenterWindow { id: Some(id) })
        }
        ConfigAction::CenterVisibleColumns => {
            Ok(IpcAction::CenterVisibleColumns {})
        }
        ConfigAction::FocusWorkspaceDown => {
            Ok(IpcAction::FocusWorkspaceDown {})
        }
        ConfigAction::FocusWorkspaceUp => Ok(IpcAction::FocusWorkspaceUp {}),
        ConfigAction::FocusWorkspace(reference) => {
            Ok(IpcAction::FocusWorkspace {
                reference: workspace_ref_to_arg(reference),
            })
        }
        ConfigAction::FocusWorkspacePrevious => {
            Ok(IpcAction::FocusWorkspacePrevious {})
        }
        ConfigAction::MoveWindowToWorkspaceDown(focus) => {
            Ok(IpcAction::MoveWindowToWorkspaceDown { focus })
        }
        ConfigAction::MoveWindowToWorkspaceUp(focus) => {
            Ok(IpcAction::MoveWindowToWorkspaceUp { focus })
        }
        ConfigAction::MoveWindowToWorkspace(reference, focus) => {
            Ok(IpcAction::MoveWindowToWorkspace {
                window_id: None,
                reference: workspace_ref_to_arg(reference),
                focus,
            })
        }
        ConfigAction::MoveWindowToWorkspaceById {
            window_id,
            reference,
            focus,
        } => Ok(IpcAction::MoveWindowToWorkspace {
            window_id: Some(window_id),
            reference: workspace_ref_to_arg(reference),
            focus,
        }),
        ConfigAction::MoveColumnToWorkspaceDown(focus) => {
            Ok(IpcAction::MoveColumnToWorkspaceDown { focus })
        }
        ConfigAction::MoveColumnToWorkspaceUp(focus) => {
            Ok(IpcAction::MoveColumnToWorkspaceUp { focus })
        }
        ConfigAction::MoveColumnToWorkspace(reference, focus) => {
            Ok(IpcAction::MoveColumnToWorkspace {
                reference: workspace_ref_to_arg(reference),
                focus,
            })
        }
        ConfigAction::MoveWorkspaceDown => Ok(IpcAction::MoveWorkspaceDown {}),
        ConfigAction::MoveWorkspaceUp => Ok(IpcAction::MoveWorkspaceUp {}),
        ConfigAction::SetWorkspaceName(name) => {
            Ok(IpcAction::SetWorkspaceName {
                name,
                workspace: None,
            })
        }
        ConfigAction::SetWorkspaceNameByRef { name, reference } => {
            Ok(IpcAction::SetWorkspaceName {
                name,
                workspace: Some(workspace_ref_to_arg(reference)),
            })
        }
        ConfigAction::UnsetWorkspaceName => {
            Ok(IpcAction::UnsetWorkspaceName { reference: None })
        }
        ConfigAction::UnsetWorkSpaceNameByRef(reference) => {
            Ok(IpcAction::UnsetWorkspaceName {
                reference: Some(workspace_ref_to_arg(reference)),
            })
        }
        ConfigAction::FocusMonitorLeft => Ok(IpcAction::FocusMonitorLeft {}),
        ConfigAction::FocusMonitorRight => Ok(IpcAction::FocusMonitorRight {}),
        ConfigAction::FocusMonitorDown => Ok(IpcAction::FocusMonitorDown {}),
        ConfigAction::FocusMonitorUp => Ok(IpcAction::FocusMonitorUp {}),
        ConfigAction::FocusMonitorPrevious => {
            Ok(IpcAction::FocusMonitorPrevious {})
        }
        ConfigAction::FocusMonitorNext => Ok(IpcAction::FocusMonitorNext {}),
        ConfigAction::FocusMonitor(output) => {
            Ok(IpcAction::FocusMonitor { output })
        }
        ConfigAction::MoveWindowToMonitorLeft => {
            Ok(IpcAction::MoveWindowToMonitorLeft {})
        }
        ConfigAction::MoveWindowToMonitorRight => {
            Ok(IpcAction::MoveWindowToMonitorRight {})
        }
        ConfigAction::MoveWindowToMonitorDown => {
            Ok(IpcAction::MoveWindowToMonitorDown {})
        }
        ConfigAction::MoveWindowToMonitorUp => {
            Ok(IpcAction::MoveWindowToMonitorUp {})
        }
        ConfigAction::MoveWindowToMonitorPrevious => {
            Ok(IpcAction::MoveWindowToMonitorPrevious {})
        }
        ConfigAction::MoveWindowToMonitorNext => {
            Ok(IpcAction::MoveWindowToMonitorNext {})
        }
        ConfigAction::MoveWindowToMonitor(output) => {
            Ok(IpcAction::MoveWindowToMonitor { id: None, output })
        }
        ConfigAction::MoveWindowToMonitorById { id, output } => {
            Ok(IpcAction::MoveWindowToMonitor {
                id: Some(id),
                output,
            })
        }
        ConfigAction::MoveColumnToMonitorLeft => {
            Ok(IpcAction::MoveColumnToMonitorLeft {})
        }
        ConfigAction::MoveColumnToMonitorRight => {
            Ok(IpcAction::MoveColumnToMonitorRight {})
        }
        ConfigAction::MoveColumnToMonitorDown => {
            Ok(IpcAction::MoveColumnToMonitorDown {})
        }
        ConfigAction::MoveColumnToMonitorUp => {
            Ok(IpcAction::MoveColumnToMonitorUp {})
        }
        ConfigAction::MoveColumnToMonitorPrevious => {
            Ok(IpcAction::MoveColumnToMonitorPrevious {})
        }
        ConfigAction::MoveColumnToMonitorNext => {
            Ok(IpcAction::MoveColumnToMonitorNext {})
        }
        ConfigAction::MoveColumnToMonitor(output) => {
            Ok(IpcAction::MoveColumnToMonitor { output })
        }
        ConfigAction::SetWindowWidth(change) => {
            Ok(IpcAction::SetWindowWidth { id: None, change })
        }
        ConfigAction::SetWindowWidthById { id, change } => {
            Ok(IpcAction::SetWindowWidth {
                id: Some(id),
                change,
            })
        }
        ConfigAction::SetWindowHeight(change) => {
            Ok(IpcAction::SetWindowHeight { id: None, change })
        }
        ConfigAction::SetWindowHeightById { id, change } => {
            Ok(IpcAction::SetWindowHeight {
                id: Some(id),
                change,
            })
        }
        ConfigAction::ResetWindowHeight => {
            Ok(IpcAction::ResetWindowHeight { id: None })
        }
        ConfigAction::ResetWindowHeightById(id) => {
            Ok(IpcAction::ResetWindowHeight { id: Some(id) })
        }
        ConfigAction::SwitchPresetColumnWidth => {
            Ok(IpcAction::SwitchPresetColumnWidth {})
        }
        ConfigAction::SwitchPresetColumnWidthBack => {
            Ok(IpcAction::SwitchPresetColumnWidthBack {})
        }
        ConfigAction::SwitchPresetWindowWidth => {
            Ok(IpcAction::SwitchPresetWindowWidth { id: None })
        }
        ConfigAction::SwitchPresetWindowWidthBack => {
            Ok(IpcAction::SwitchPresetWindowWidthBack { id: None })
        }
        ConfigAction::SwitchPresetWindowWidthById(id) => {
            Ok(IpcAction::SwitchPresetWindowWidth { id: Some(id) })
        }
        ConfigAction::SwitchPresetWindowWidthBackById(id) => {
            Ok(IpcAction::SwitchPresetWindowWidthBack { id: Some(id) })
        }
        ConfigAction::SwitchPresetWindowHeight => {
            Ok(IpcAction::SwitchPresetWindowHeight { id: None })
        }
        ConfigAction::SwitchPresetWindowHeightBack => {
            Ok(IpcAction::SwitchPresetWindowHeightBack { id: None })
        }
        ConfigAction::SwitchPresetWindowHeightById(id) => {
            Ok(IpcAction::SwitchPresetWindowHeight { id: Some(id) })
        }
        ConfigAction::SwitchPresetWindowHeightBackById(id) => {
            Ok(IpcAction::SwitchPresetWindowHeightBack { id: Some(id) })
        }
        ConfigAction::MaximizeColumn => Ok(IpcAction::MaximizeColumn {}),
        ConfigAction::MaximizeWindowToEdges => {
            Ok(IpcAction::MaximizeWindowToEdges { id: None })
        }
        ConfigAction::MaximizeWindowToEdgesById(id) => {
            Ok(IpcAction::MaximizeWindowToEdges { id: Some(id) })
        }
        ConfigAction::SetColumnWidth(change) => {
            Ok(IpcAction::SetColumnWidth { change })
        }
        ConfigAction::ExpandColumnToAvailableWidth => {
            Ok(IpcAction::ExpandColumnToAvailableWidth {})
        }
        ConfigAction::SwitchLayout(layout) => {
            Ok(IpcAction::SwitchLayout { layout })
        }
        ConfigAction::ShowHotkeyOverlay => Ok(IpcAction::ShowHotkeyOverlay {}),
        ConfigAction::MoveWorkspaceToMonitorLeft => {
            Ok(IpcAction::MoveWorkspaceToMonitorLeft {})
        }
        ConfigAction::MoveWorkspaceToMonitorRight => {
            Ok(IpcAction::MoveWorkspaceToMonitorRight {})
        }
        ConfigAction::MoveWorkspaceToMonitorDown => {
            Ok(IpcAction::MoveWorkspaceToMonitorDown {})
        }
        ConfigAction::MoveWorkspaceToMonitorUp => {
            Ok(IpcAction::MoveWorkspaceToMonitorUp {})
        }
        ConfigAction::MoveWorkspaceToMonitorPrevious => {
            Ok(IpcAction::MoveWorkspaceToMonitorPrevious {})
        }
        ConfigAction::MoveWorkspaceToIndex(index) => {
            Ok(IpcAction::MoveWorkspaceToIndex {
                index,
                reference: None,
            })
        }
        ConfigAction::MoveWorkspaceToIndexByRef { new_idx, reference } => {
            Ok(IpcAction::MoveWorkspaceToIndex {
                index: new_idx,
                reference: Some(workspace_ref_to_arg(reference)),
            })
        }
        ConfigAction::MoveWorkspaceToMonitor(output) => {
            Ok(IpcAction::MoveWorkspaceToMonitor {
                output,
                reference: None,
            })
        }
        ConfigAction::MoveWorkspaceToMonitorByRef {
            output_name,
            reference,
        } => Ok(IpcAction::MoveWorkspaceToMonitor {
            output: output_name,
            reference: Some(workspace_ref_to_arg(reference)),
        }),
        ConfigAction::MoveWorkspaceToMonitorNext => {
            Ok(IpcAction::MoveWorkspaceToMonitorNext {})
        }
        ConfigAction::ToggleDebugTint => Ok(IpcAction::ToggleDebugTint {}),
        ConfigAction::DebugToggleOpaqueRegions => {
            Ok(IpcAction::DebugToggleOpaqueRegions {})
        }
        ConfigAction::DebugToggleDamage => Ok(IpcAction::DebugToggleDamage {}),
        ConfigAction::ToggleWindowFloating => {
            Ok(IpcAction::ToggleWindowFloating { id: None })
        }
        ConfigAction::ToggleWindowFloatingById(id) => {
            Ok(IpcAction::ToggleWindowFloating { id: Some(id) })
        }
        ConfigAction::MoveWindowToFloating => {
            Ok(IpcAction::MoveWindowToFloating { id: None })
        }
        ConfigAction::MoveWindowToFloatingById(id) => {
            Ok(IpcAction::MoveWindowToFloating { id: Some(id) })
        }
        ConfigAction::MoveWindowToTiling => {
            Ok(IpcAction::MoveWindowToTiling { id: None })
        }
        ConfigAction::MoveWindowToTilingById(id) => {
            Ok(IpcAction::MoveWindowToTiling { id: Some(id) })
        }
        ConfigAction::FocusFloating => Ok(IpcAction::FocusFloating {}),
        ConfigAction::FocusTiling => Ok(IpcAction::FocusTiling {}),
        ConfigAction::SwitchFocusBetweenFloatingAndTiling => {
            Ok(IpcAction::SwitchFocusBetweenFloatingAndTiling {})
        }
        ConfigAction::MoveFloatingWindowById { id, x, y } => {
            Ok(IpcAction::MoveFloatingWindow { id, x, y })
        }
        ConfigAction::ToggleWindowRuleOpacity => {
            Ok(IpcAction::ToggleWindowRuleOpacity { id: None })
        }
        ConfigAction::ToggleWindowRuleOpacityById(id) => {
            Ok(IpcAction::ToggleWindowRuleOpacity { id: Some(id) })
        }
        ConfigAction::SetDynamicCastWindow => {
            Ok(IpcAction::SetDynamicCastWindow { id: None })
        }
        ConfigAction::SetDynamicCastWindowById(id) => {
            Ok(IpcAction::SetDynamicCastWindow { id: Some(id) })
        }
        ConfigAction::SetDynamicCastMonitor(output) => {
            Ok(IpcAction::SetDynamicCastMonitor { output })
        }
        ConfigAction::ClearDynamicCastTarget => {
            Ok(IpcAction::ClearDynamicCastTarget {})
        }
        ConfigAction::ToggleOverview => Ok(IpcAction::ToggleOverview {}),
        ConfigAction::OpenOverview => Ok(IpcAction::OpenOverview {}),
        ConfigAction::CloseOverview => Ok(IpcAction::CloseOverview {}),
        ConfigAction::ToggleWindowUrgent(id) => {
            Ok(IpcAction::ToggleWindowUrgent { id })
        }
        ConfigAction::SetWindowUrgent(id) => {
            Ok(IpcAction::SetWindowUrgent { id })
        }
        ConfigAction::UnsetWindowUrgent(id) => {
            Ok(IpcAction::UnsetWindowUrgent { id })
        }
        ConfigAction::LoadConfigFile => Ok(IpcAction::LoadConfigFile {}),
        other => Err(DecodeError::unsupported(
            span,
            format!("action `{:?}` not supported by niri ipc", other),
        )),
    }
}

fn workspace_ref_to_arg(
    workspace_ref: WorkspaceReference,
) -> WorkspaceReferenceArg {
    match workspace_ref {
        WorkspaceReference::Id(id) => WorkspaceReferenceArg::Id(id),
        WorkspaceReference::Index(idx) => WorkspaceReferenceArg::Index(idx),
        WorkspaceReference::Name(name) => WorkspaceReferenceArg::Name(name),
    }
}
