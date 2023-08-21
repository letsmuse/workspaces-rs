#![recursion_limit = "256"]
use serde_json::{Map, Value};
use test_log::test;

use std::fs::File;
use std::path::Path;

#[test(tokio::test)]
async fn test_subaccount_creation() -> anyhow::Result<()> {
    let worker = workspaces::sandbox().await?;
    let account = worker.dev_create_account().await?;

    let sub = account
        .create_subaccount("subaccount")
        .transact()
        .await?
        .into_result()?;

    let expect_id = format!("subaccount.{}", account.id());
    let actual_id = sub.id().to_string();

    assert_eq!(actual_id, expect_id);

    // Check if the stored credentials match with the subaccount created.
    let savedir = Path::new("../target/credentials");
    sub.store_credentials(savedir).await?;
    let creds = File::open(savedir.join(format!("{}.json", sub.id())))?;
    let contents: Map<String, Value> = serde_json::from_reader(creds)?;
    assert_eq!(
        contents.get("account_id"),
        Some(&Value::String(sub.id().to_string()))
    );

    Ok(())
}

#[test(tokio::test)]
async fn test_transfer_near() -> anyhow::Result<()> {
    let worker = workspaces::sandbox().await?;
    let (alice, bob) = (
        worker.dev_create_account().await?,
        worker.dev_create_account().await?,
    );

    // transfer 500_000_000 token from alice to bob
    _ = alice.transfer_near(bob.id(), 500_000_000).await?;

    // All sandbox accounts start with a balance of `100 * 1_000_000_000_000_000_000_000_000` tokens.
    // Assert the the tokens have been transferred.
    assert_eq!(
        bob.view_account().await?.balance,
        100 * 1_000_000_000_000_000_000_000_000 + 500_000_000,
    );

    Ok(())
}