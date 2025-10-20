use crate::actiions::install::install;
use crate::actiions::restart_explorer::restart_explorer;
use crate::actiions::uninstall::uninstall;
use dialoguer::Select;
use dialoguer::console::{Term, style};
use dialoguer::theme::ColorfulTheme;
use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Install,
    Uninstall,
    RestartExplorer,
    Exit,
}

fn theme() -> ColorfulTheme {
    // Force ASCII arrow; allow override via env MENU_ARROW if you ever need it.
    let mut t = ColorfulTheme::default();
    t.active_item_prefix = style(">".to_string());
    t.inactive_item_prefix = style(" ".to_string());
    t.picked_item_prefix = style(">".to_string());
    t.unpicked_item_prefix = style(" ".to_string());
    t.prompt_prefix = style("$".to_string());
    t.success_prefix = style(">".to_string());
    t.error_prefix = style("!".to_string());
    t
}

pub fn action_choose() -> io::Result<(Action, String)> {
    let actions = vec![
        Action::Install,
        Action::Uninstall,
        Action::RestartExplorer,
        Action::Exit,
    ];

    let labels = vec![
        "Install (requires admin)",
        "Uninstall (requires admin)",
        "Restart Explorer",
        "Exit",
    ];

    let idx = Select::with_theme(&theme())
        .with_prompt("MPQ Archive Viewer Installer")
        .items(&labels)
        .default(0)
        .interact_on(&Term::stdout())?;

    Ok((actions[idx], labels[idx].to_string()))
}

pub fn action_execute(action: Action) -> io::Result<()> {
    match action {
        Action::Install => install(),
        Action::Uninstall => uninstall(),
        Action::RestartExplorer => restart_explorer(),
        Action::Exit => Ok(()),
    }
}
