use color_eyre::eyre;

// #[async_trait::async_trait]
pub trait Scrape {
    async fn scrape(&mut self) -> eyre::Result<()>;
}
