// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::{Command, CommandFactory, Parser, ValueEnum};
use clap_complete::{Generator, Shell, generate, generate_to};
use strum::{EnumIter, IntoEnumIterator};

use crate::iota_commands::IotaCommand;

#[derive(Debug, Clone, ValueEnum, EnumIter)]
#[value(rename_all = "lower")]
pub enum GenShell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
}

impl Generator for GenShell {
    fn file_name(&self, name: &str) -> String {
        match self {
            Self::Bash => Shell::Bash.file_name(name),
            Self::Elvish => Shell::Elvish.file_name(name),
            Self::Fish => Shell::Fish.file_name(name),
            Self::PowerShell => Shell::PowerShell.file_name(name),
            Self::Zsh => Shell::Zsh.file_name(name),
        }
    }

    fn generate(&self, cmd: &clap::Command, buf: &mut dyn std::io::prelude::Write) {
        match self {
            Self::Bash => Shell::Bash.generate(cmd, buf),
            Self::Elvish => Shell::Elvish.generate(cmd, buf),
            Self::Fish => Shell::Fish.generate(cmd, buf),
            Self::PowerShell => Shell::PowerShell.generate(cmd, buf),
            Self::Zsh => Shell::Zsh.generate(cmd, buf),
        }
    }
}

#[derive(Debug, Parser)]
pub struct GenerateCompletionsCommand {
    /// The shell for which completions will be generated. By default all will
    /// be generated.
    #[arg(long, short)]
    shell: Option<GenShell>,
    /// The output directory for the completions file.
    /// Prints to stdout by default.
    #[arg(long, short)]
    out_dir: Option<String>,
}

impl GenerateCompletionsCommand {
    pub fn run(self) -> anyhow::Result<()> {
        let GenerateCompletionsCommand { shell, out_dir } = self;

        let mut cli = IotaCommand::command();

        if let Some(out_dir) = &out_dir {
            std::fs::create_dir(out_dir).ok();
        }

        fn gen(shell: GenShell, out_dir: &Option<String>, cli: &mut Command) -> anyhow::Result<()> {
            match out_dir {
                Some(out_dir) => {
                    generate_to(shell, cli, env!("CARGO_PKG_NAME"), out_dir)?;
                }
                None => {
                    generate(shell, cli, env!("CARGO_PKG_NAME"), &mut std::io::stdout());
                }
            }
            Ok(())
        }

        if let Some(shell) = shell {
            gen(shell, &out_dir, &mut cli)?;
        } else {
            for shell in GenShell::iter() {
                gen(shell, &out_dir, &mut cli)?;
            }
        }

        Ok(())
    }
}
