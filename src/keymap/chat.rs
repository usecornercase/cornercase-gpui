use super::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InChat {
    Acts,
    Leaves,
    OnTop,
    Rests,
    Field,
}

pub(crate) fn swallowed_in_chat(command: Command) -> bool {
    matches!(in_chat(command), InChat::Rests | InChat::Field)
}

pub(crate) fn in_chat(command: Command) -> InChat {
    match command {
        Command::Mark
        | Command::CloseTab
        | Command::SplitRight
        | Command::SplitDown
        | Command::PreviousPane
        | Command::NextPane => InChat::Rests,
        Command::Paste => InChat::Field,
        Command::Close => InChat::OnTop,
        Command::NextWorkspace
        | Command::PreviousWorkspace
        | Command::NextProject
        | Command::PreviousProject
        | Command::AgentNeedingYou
        | Command::NewTab
        | Command::PreviousTab
        | Command::NextTab
        | Command::Tab => InChat::Leaves,
        Command::Panel(_)
        | Command::FindFile
        | Command::FindInFile
        | Command::Copy
        | Command::NewWorkspace
        | Command::StartAgent
        | Command::OpenProject
        | Command::NewBot
        | Command::Bots
        | Command::Connect
        | Command::PullRequest
        | Command::ZoomIn
        | Command::ZoomOut
        | Command::ZoomReset
        | Command::Shortcuts
        | Command::TakeToast
        | Command::Back
        | Command::Forward
        | Command::Reload
        | Command::FocusAddress
        | Command::Cut => InChat::Acts,
    }
}
