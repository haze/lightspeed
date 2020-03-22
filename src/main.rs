use async_std::sync::{Arc, Mutex};
use dawn::{
    gateway::{shard::Event, Cluster, ClusterConfig},
    http::Client as HttpClient,
};
use futures::StreamExt;
use std::{env, error::Error, fmt};

#[derive(Debug, Clone, Copy)]
enum LightspeedError {
    TessInit,
    InvalidPath,
    ProducedNonUTF8Text,
    MutexError,
}
impl fmt::Display for LightspeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for LightspeedError {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let token = env::var("DISCORD_TOKEN")?;
    let http = HttpClient::new(&token);

    let cluster_config = ClusterConfig::builder(&token).build();
    let cluster = Cluster::new(cluster_config);
    cluster.up().await?;

    let mut events = cluster.events().await;

    while let Some(event) = events.next().await {
        tokio::spawn(handle_event(event, http.clone()));
    }

    Ok(())
}

async fn handle_event(
    event: (u64, Event),
    http: HttpClient,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use std::io::Write;
    use tempfile::NamedTempFile;
    match event {
        (id, Event::Ready(_)) => {
            println!("Shard {} connected & listening…", id);
        }
        (_, Event::MessageCreate(msg)) => {
            if let Some(image) = msg.attachments.get(0) {
                if msg.author.id.0 != 418412306981191680 {
                    return Ok(());
                }
                // 0. Get a temp file to store image in
                let mut file = NamedTempFile::new()?;
                // 1. Download file to temp file
                let image_bytes = surf::get(&*image.url).recv_bytes().await?;
                file.write_all(image_bytes.as_slice())?;
                let text = {
                    let mut handle = leptess::LepTess::new(None, "eng")
                        .map_err(|_| LightspeedError::TessInit)?;
                    handle.set_image(
                        file.path()
                            .to_str()
                            .ok_or_else(|| LightspeedError::InvalidPath)?,
                    );
                    handle
                        .get_utf8_text()
                        .map(|text| text.replace("\n", " "))
                        .map_err(|_| LightspeedError::ProducedNonUTF8Text)?
                };
                http.create_message(msg.channel_id).content(text).await?;
            }
        }
        _ => {}
    }

    Ok(())
}
