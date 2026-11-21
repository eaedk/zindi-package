//! Browse open competitions and, when you are logged in, show your standing.
//!
//! ```sh
//! cargo run --example quickstart
//! ZINDI_TOKEN=... cargo run --example quickstart -- Crop-Yield-Forecast-Challenge
//! ```

use zindi::{Client, CompetitionQuery};

#[tokio::main]
async fn main() -> zindi::Result<()> {
    let client = Client::new()?;

    println!("Open competitions:");
    for comp in client
        .competitions(CompetitionQuery::new().limit(Some(10)))
        .await?
    {
        let ends = comp
            .end_time
            .map(|t| t.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        println!(
            "  {:<45} {:<20} ends {ends}",
            comp.id,
            comp.reward.as_deref().unwrap_or("-")
        );
    }

    let Some(id) = std::env::args().nth(1) else {
        println!("\nPass a competition id to see its leaderboard.");
        return Ok(());
    };

    let comp = client.competition(Some(&id)).await?;
    println!("\n{} ({})", comp.title, comp.url());
    if let Some(limit) = comp.daily_submission_limit() {
        println!("Daily submission limit: {limit}");
    }
    for file in &comp.data_files {
        println!("  {:<30} {}", file.filename, file.size_human());
    }

    println!("\nTop 5:");
    for entry in client.leaderboard(Some(&id), Some(5), false).await? {
        println!(
            "  {:>3}  {:<30} {}",
            entry.rank.unwrap_or_default(),
            entry.display_name(),
            entry.score.map(|s| s.to_string()).unwrap_or_default()
        );
    }

    if client.is_authenticated() {
        match client.rank(Some(&id)).await? {
            Some(rank) => println!("\nYour rank: {rank}"),
            None => println!("\nYou have no valid score yet."),
        }
    }
    Ok(())
}
