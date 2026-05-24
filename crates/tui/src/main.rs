#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_tui::run().await
}
