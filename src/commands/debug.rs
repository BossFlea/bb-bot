use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result, anyhow, bail};
use poise::{
    ChoiceParameter, CreateReply,
    serenity_prelude::{
        AutocompleteChoice, CreateAutocompleteResponse, CreateComponent, CreateContainer,
        CreateTextDisplay, MessageFlags,
        colours::css::{DANGER, POSITIVE},
    },
};
use tracing::warn;

use crate::config::DB_SCRIPTS_DIR;
use crate::error::UserError;
use crate::shared::Context;

#[poise::command(slash_command, subcommand_required, subcommands("error", "sql"))]
pub async fn debug(_ctx: Context<'_>) -> Result<()> {
    unreachable!("This shouldn't be possible to invoke");
}

#[derive(ChoiceParameter)]
enum ErrorKind {
    User,
    Internal,
    Panic,
}

/// Intentionally trigger an error
#[poise::command(slash_command)]
async fn error(
    _ctx: Context<'_>,
    #[description = "Kind of error to return"] kind: Option<ErrorKind>,
) -> Result<()> {
    let error_type = kind.unwrap_or(ErrorKind::Internal);
    match error_type {
        ErrorKind::User => bail!(UserError(
            anyhow!("This is an example of a user error")
                .context("This is an example of extra context"),
        )),
        ErrorKind::Internal => Err(anyhow!("This is an example of an internal error")
            .context("This is an example of extra context")),
        ErrorKind::Panic => panic!("This is an example of a panic"),
    }
}

/// Execute read-only SQL on the bot's database. ('SELECT' statements)
#[poise::command(slash_command)]
async fn sql(
    ctx: Context<'_>,
    #[description = "SELECT statement to run"] sql: String,
) -> Result<()> {
    let sql_data = ctx.data().db_handle.raw_query_readonly(sql).await?;

    let response = format!("## SQL Response\n{}", sql_data.to_formatted());

    let container = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            response,
        ))])
        .accent_color(POSITIVE),
    );

    ctx.send(
        CreateReply::new()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container]), // .ephemeral(true),
    )
    .await?;

    Ok(())
}

// NOTE: currently not included anywhere, disabled
/// Run an SQL script on the bot's database. OPERATES ON THE LIVE DB!
#[poise::command(slash_command, rename = "script")]
async fn sql_script(
    ctx: Context<'_>,
    #[description = "Script path in scripts directory"]
    #[autocomplete = "autocomplete_script"]
    script: String,
) -> Result<()> {
    if script.contains("..") {
        warn!(
            "{} tried to inject '..' into sql script path",
            ctx.author().name
        );

        let container = CreateComponent::Container(
            CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                "## Invalid Script
Script path cannot contain `..`",
            ))])
            .accent_color(DANGER),
        );

        ctx.send(
            CreateReply::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let sql = fs::read_to_string(format!("{DB_SCRIPTS_DIR}/{script}"))
        .context(UserError(anyhow!("Failed to load SQL script")))?;

    ctx.data().db_handle.raw_batch(sql).await?;

    let container = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "## Script Executed Successfully
The `{script}` SQL script executed without errors."
            ),
        ))])
        .accent_color(POSITIVE),
    );

    ctx.send(
        CreateReply::new()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container]), // .ephemeral(true),
    )
    .await?;

    Ok(())
}

#[allow(dead_code)]
async fn autocomplete_script<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> CreateAutocompleteResponse<'a> {
    let mut filenames = Vec::new();
    if let Err(err) = get_filenames(Path::new(DB_SCRIPTS_DIR), &mut filenames) {
        warn!("SQL script autocompletion failed for input '{partial}': {err:#?}");
    };

    let prefix = format!("{DB_SCRIPTS_DIR}/");

    let choices: Vec<_> = filenames
        .into_iter()
        .filter_map(|f| {
            let trimmed = f
                .strip_prefix(&prefix)
                .expect("All paths should start with parent dir")
                .to_string();
            trimmed
                .starts_with(partial)
                .then_some(AutocompleteChoice::new(trimmed.clone(), trimmed))
        })
        .collect();

    CreateAutocompleteResponse::new().set_choices(choices)
}

#[allow(dead_code)]
fn get_filenames(path: &Path, files: &mut Vec<String>) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_dir() {
                get_filenames(&path, files)?;
            } else {
                files.push(path.display().to_string())
            }
        }
    }
    Ok(())
}
