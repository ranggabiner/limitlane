use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "limitlane",
    author,
    version,
    about = "Native terminal dashboard for monitoring AI coding-provider usage limits"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Display usage status summary
    Status(StatusArgs),
    /// Account management commands
    Accounts(AccountsArgs),
    /// Display detailed usage snapshots and windows
    Usage(UsageArgs),
    /// Trigger usage limit refresh for accounts or providers
    Refresh(RefreshArgs),
    /// Run diagnostic checks on system, network, and providers
    Doctor(DoctorArgs),
    /// Configuration management
    Config(ConfigArgs),
    /// Display version information
    Version,
}

#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// Output in versioned JSON format (schema_version: "1")
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AccountsArgs {
    #[command(subcommand)]
    pub action: Option<AccountAction>,

    /// Output account list in versioned JSON format (schema_version: "1")
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AccountAction {
    /// List all configured accounts
    List {
        /// Output in versioned JSON format
        #[arg(long)]
        json: bool,
    },
    /// Add a new account (e.g. limitlane accounts add codex user@example.com)
    Add {
        /// Provider name (e.g. "claude" or "codex")
        provider: Option<String>,
        /// Identity / email address for account
        identity: Option<String>,
        /// Provider flag (--provider codex)
        #[arg(long)]
        p: Option<String>,
        /// Identity flag (--identity user@example.com)
        #[arg(long)]
        i: Option<String>,
        /// Optional alias for account
        #[arg(long)]
        alias: Option<String>,
        /// Auth method (e.g. "oauth", "api_key")
        #[arg(long, default_value = "api_key")]
        auth: String,
        /// Optional API key or secret token
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Remove an account by ID
    Remove {
        /// Account ID to remove
        #[arg(long, short)]
        id: String,
    },
    /// Trigger reauthentication for an account
    Reauth {
        /// Account ID to reauthenticate
        #[arg(long, short)]
        id: String,
    },
}

#[derive(Args, Debug, Clone)]
pub struct UsageArgs {
    /// Filter by account ID
    #[arg(long, short)]
    pub account: Option<String>,

    /// Filter by provider ID (e.g. "claude", "codex")
    #[arg(long, short)]
    pub provider: Option<String>,

    /// Output in versioned JSON format (schema_version: "1")
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RefreshArgs {
    /// Refresh specific account ID
    #[arg(long, short)]
    pub account: Option<String>,

    /// Refresh all accounts for specific provider
    #[arg(long, short)]
    pub provider: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct DoctorArgs {
    /// Output diagnostic report in versioned JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    /// Action to perform on config
    #[arg(long, default_value = "show")]
    pub action: String,
}
