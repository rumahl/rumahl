use std::error::Error;
use std::fmt;

use crate::Identity;

use super::{CommandContribution, ContributionId};

#[derive(Debug, Default, Clone)]
pub struct CommandRegistry {
    commands: Vec<CommandContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRegistryError {
    AlreadyRegistered,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_register(&self, command: &CommandContribution) -> Result<(), CommandRegistryError> {
        if self
            .commands
            .iter()
            .any(|existing| existing.id() == command.id())
        {
            return Err(CommandRegistryError::AlreadyRegistered);
        }

        Ok(())
    }

    pub fn register(&mut self, command: CommandContribution) -> Result<(), CommandRegistryError> {
        self.can_register(&command)?;

        self.commands.push(command);

        Ok(())
    }

    pub fn get(&self, id: &ContributionId) -> Option<&CommandContribution> {
        self.commands.iter().find(|command| command.id() == id)
    }

    pub fn commands_for_owner(&self, owner: &Identity) -> Vec<&CommandContribution> {
        self.commands
            .iter()
            .filter(|command| command.owner() == owner)
            .collect()
    }

    pub fn commands(&self) -> &[CommandContribution] {
        &self.commands
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub(crate) fn remove_for_owner(&mut self, owner: &Identity) -> usize {
        let before = self.commands.len();

        self.commands.retain(|command| command.owner() != owner);

        before - self.commands.len()
    }
}

impl fmt::Display for CommandRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered => {
                write!(f, "command contribution is already registered")
            }
        }
    }
}

impl Error for CommandRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AppId, AppIdentity, CapabilityId, CommandAction, InstallationId, PublisherId};

    fn app(id: &str) -> AppIdentity {
        AppIdentity::new(
            AppId::parse(id).unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    fn command(id: &str, owner: AppIdentity, title: &str, capability: &str) -> CommandContribution {
        CommandContribution::new(
            ContributionId::parse(id).unwrap(),
            owner.into(),
            title,
            CommandAction::invoke_capability(CapabilityId::parse(capability).unwrap()),
        )
        .unwrap()
    }

    #[test]
    fn registers_command() {
        let mut registry = CommandRegistry::new();

        registry
            .register(command(
                "com.rumahl.notes.new-note",
                app("com.rumahl.notes"),
                "New note",
                "com.rumahl.notes.create-note",
            ))
            .unwrap();

        assert_eq!(registry.len(), 1);

        assert!(!registry.is_empty());
    }

    #[test]
    fn resolves_command_by_id() {
        let id = ContributionId::parse("com.rumahl.notes.new-note").unwrap();

        let mut registry = CommandRegistry::new();

        registry
            .register(command(
                id.as_str(),
                app("com.rumahl.notes"),
                "New note",
                "com.rumahl.notes.create-note",
            ))
            .unwrap();

        let resolved = registry.get(&id).unwrap();

        assert_eq!(resolved.id(), &id);

        assert_eq!(resolved.title(), "New note");
    }

    #[test]
    fn rejects_duplicate_command_id() {
        let notes = app("com.rumahl.notes");

        let first = command(
            "com.rumahl.notes.new-note",
            notes.clone(),
            "New note",
            "com.rumahl.notes.create-note",
        );

        let duplicate = command(
            "com.rumahl.notes.new-note",
            notes,
            "Create note",
            "com.rumahl.notes.create-note",
        );

        let mut registry = CommandRegistry::new();

        registry.register(first).unwrap();

        assert_eq!(
            registry.register(duplicate).unwrap_err(),
            CommandRegistryError::AlreadyRegistered
        );
    }

    #[test]
    fn lists_commands_for_owner() {
        let notes = app("com.rumahl.notes");

        let files = app("com.rumahl.files");

        let notes_identity = notes.clone().into();

        let mut registry = CommandRegistry::new();

        registry
            .register(command(
                "com.rumahl.notes.new-note",
                notes.clone(),
                "New note",
                "com.rumahl.notes.create-note",
            ))
            .unwrap();

        registry
            .register(command(
                "com.rumahl.notes.open",
                notes,
                "Open Notes",
                "com.rumahl.notes.open",
            ))
            .unwrap();

        registry
            .register(command(
                "com.rumahl.files.open",
                files,
                "Open Files",
                "com.rumahl.files.open",
            ))
            .unwrap();

        let commands = registry.commands_for_owner(&notes_identity);

        assert_eq!(commands.len(), 2);

        assert!(
            commands
                .iter()
                .all(|command| { command.owner() == &notes_identity })
        );
    }

    #[test]
    fn can_register_new_command_without_mutating_registry() {
        let command = command(
            "com.rumahl.notes.new-note",
            app("com.rumahl.notes"),
            "New note",
            "com.rumahl.notes.create-note",
        );

        let registry = CommandRegistry::new();

        assert!(registry.can_register(&command).is_ok());

        assert!(registry.is_empty());
    }

    #[test]
    fn cannot_register_existing_command() {
        let command = command(
            "com.rumahl.notes.new-note",
            app("com.rumahl.notes"),
            "New note",
            "com.rumahl.notes.create-note",
        );

        let mut registry = CommandRegistry::new();

        registry.register(command.clone()).unwrap();

        assert_eq!(
            registry.can_register(&command).unwrap_err(),
            CommandRegistryError::AlreadyRegistered
        );
    }

    #[test]
    fn removes_only_commands_for_owner() {
        let notes = app("com.rumahl.notes");

        let files = app("com.rumahl.files");

        let notes_identity = notes.clone().into();

        let mut registry = CommandRegistry::new();

        registry
            .register(command(
                "com.rumahl.notes.open",
                notes,
                "Open Notes",
                "com.rumahl.notes.open",
            ))
            .unwrap();

        registry
            .register(command(
                "com.rumahl.files.open",
                files,
                "Open Files",
                "com.rumahl.files.open",
            ))
            .unwrap();

        let removed = registry.remove_for_owner(&notes_identity);

        assert_eq!(removed, 1);
        assert_eq!(registry.len(), 1);

        assert!(registry.commands_for_owner(&notes_identity).is_empty());
    }
}
