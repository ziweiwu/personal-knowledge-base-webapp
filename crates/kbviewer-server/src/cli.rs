//! Account management, on the same binary as the server.
//!
//! There is no HTTP signup route by design: this is one person's knowledge base, and an
//! account can only be created by someone with shell access to the machine.

use crate::auth::store::AuthStore;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Short passwords are the one weakness this tool can refuse at the door.
const MIN_PASSWORD_CHARS: usize = 12;

#[derive(Parser)]
#[command(
    name = "kbviewer",
    about = "Browse and edit local document folders over the web"
)]
pub struct Cli {
    /// Path to the configuration file.
    #[arg(long, env = "KBVIEWER_CONFIG", default_value = "kbviewer.config.json")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage accounts.
    User {
        #[command(subcommand)]
        action: UserAction,
    },
}

#[derive(Subcommand)]
pub enum UserAction {
    /// Create an account.
    Add {
        email: String,
        /// Read the password from stdin instead of prompting, for container
        /// provisioning and scripted setup.
        #[arg(long)]
        password_stdin: bool,
    },
    /// List accounts.
    List,
    /// Change an account's password.
    Passwd {
        email: String,
        #[arg(long)]
        password_stdin: bool,
    },
    /// Delete an account and all of its sessions.
    Rm { email: String },
    /// Sign every device out, for instance after losing one.
    Revoke,
}

pub fn run_user_command(store: &AuthStore, action: UserAction) -> Result<()> {
    match action {
        UserAction::Add {
            email,
            password_stdin,
        } => {
            let password = if password_stdin {
                read_piped_password()?
            } else {
                prompt_new_password()?
            };
            add_account(store, &email, &password)
        }
        UserAction::List => {
            print_accounts(store);
            Ok(())
        }
        UserAction::Passwd {
            email,
            password_stdin,
        } => {
            let password = if password_stdin {
                read_piped_password()?
            } else {
                prompt_new_password()?
            };
            change_password(store, &email, &password)
        }
        UserAction::Rm { email } => remove_account(store, &email),
        UserAction::Revoke => revoke_all_sessions(store),
    }
}

fn add_account(store: &AuthStore, email: &str, password: &str) -> Result<()> {
    let user = store
        .add_user(email, password)
        .context("could not create the account")?;
    println!("Created {}", user.email);
    Ok(())
}

fn print_accounts(store: &AuthStore) {
    let users = store.list_users();
    if users.is_empty() {
        println!("No accounts yet. Create one with: kbviewer user add <email>");
    }
    for user in users {
        println!("{}", user.email);
    }
}

fn change_password(store: &AuthStore, email: &str, password: &str) -> Result<()> {
    store
        .set_password(email, password)
        .context("could not change the password")?;
    // Sessions are deliberately kept: changing your own password on a device you
    // are already using should not sign you out of it. Use `revoke` for that.
    println!("Password changed for {email}");
    Ok(())
}

fn remove_account(store: &AuthStore, email: &str) -> Result<()> {
    store
        .remove_user(email)
        .context("could not remove the account")?;
    println!("Removed {email}");
    Ok(())
}

fn revoke_all_sessions(store: &AuthStore) -> Result<()> {
    let count = store.session_count();
    store.revoke_all_sessions()?;
    println!("Signed out {count} session(s)");
    Ok(())
}

/// Read the password from stdin, for container provisioning and scripted setup.
fn read_piped_password() -> Result<String> {
    let mut password = String::new();
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut password)?;
    let password = password.trim_end_matches(['\n', '\r']).to_string();
    check_password_strength(&password)?;
    Ok(password)
}

fn check_password_strength(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        bail!("password must be at least {MIN_PASSWORD_CHARS} characters");
    }
    Ok(())
}

/// Read a password twice without echoing it, and refuse obviously weak ones.
fn prompt_new_password() -> Result<String> {
    let password = rpassword::prompt_password("New password: ")?;
    check_password_strength(&password)?;
    let again = rpassword::prompt_password("Repeat password: ")?;
    if password != again {
        bail!("passwords did not match");
    }
    Ok(password)
}
