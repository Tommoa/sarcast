// #![deny(missing_docs)]
#![deny(unused_results)]
#![deny(unreachable_pub)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![deny(bad_style)]
#![deny(unused)]
#![deny(clippy::pedantic)]

use symphonia::core::io::MediaSource;
use symphonia::core::meta::{MetadataRevision, TableOfContentsItem};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};

mod audio_thread;
mod decoder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing_subscriber::filter::LevelFilter::DEBUG)
        .init();

    let (send, recv) = mpsc::channel(1);
    let (metadata_send, mut metadata_recv) = mpsc::channel(1000);
    let _audio_task = tokio::task::spawn_blocking(move || audio_thread::run(metadata_send, recv));

    let feed = reqwest::get("https://biblethinker.castos.com/feed")
        .await?
        .bytes()
        .await?;
    // let feed = reqwest::get("https://atp.fm/rss").await?.bytes().await?;
    // let feed = reqwest::get("https://tilos.hu/feed/show/hi-fi-budapest")
    //     .await?
    //     .bytes()
    //     .await?;
    let mut atom = rss::Channel::read_from(&feed[..])?;
    atom.set_items(atom.items[2..3].to_owned());
    println!("{:?}", atom);
    println!("{:#?}", atom.items[0].enclosure());
    let sub_send = send.clone();
    let _ = tokio::task::spawn(async move {
        let send = sub_send;
        while let Some(metadata) = metadata_recv.recv().await {
            match metadata {
                ReceivedData::NewMetadata(metadata) => {
                    if let Some(table_of_contents) = &metadata.table_of_contents() {
                        if let TableOfContentsItem::Chapter(chapter) = &table_of_contents.items[2] {
                            send.send(PlaybackInstructions::Seek(u64::from(chapter.start_ms)))
                                .await
                                .unwrap();
                        }
                    }
                }
                ReceivedData::NewTimestamp(ts) => {
                    let _ = ts;
                    // println!("Timestamp: {}", ts);
                }
            }
        }
    });
    if let Some(enclosure) = atom.items[0].enclosure() {
        let _ = enclosure;
        stream_podcast(
            send.clone(),
            Stream::File("test.mp3".into()),
            // Stream::Url(reqwest::Url::try_from(enclosure.url())?),
        )
        .await?;
    }
    _audio_task.await?;
    Ok(())
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub enum Stream {
    File(std::path::PathBuf),
    Url(reqwest::Url),
}

#[derive(Debug)]
pub struct Downloader {
    full_size: u64,
    streamed_bytes: std::sync::Arc<tokio::sync::Mutex<bytes::BytesMut>>,
    receiver: broadcast::Receiver<bytes::Bytes>,
}

impl Downloader {
    pub async fn start(url: reqwest::Url) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, receiver) = broadcast::channel(1000);
        let mut response = reqwest::get(url).await?;
        let full_size = response.content_length().unwrap();
        let mut dup_rx = tx.subscribe();
        let _ = tokio::task::spawn(async move {
            while let Some(chunked) = response.chunk().await.unwrap() {
                let _ = tx.send(chunked);
            }
        });
        let streamed_bytes = std::sync::Arc::new(tokio::sync::Mutex::new(bytes::BytesMut::new()));
        let cloned_bytes = std::sync::Arc::clone(&streamed_bytes);
        let _ = tokio::task::spawn(async move {
            while let Ok(new_bytes) = dup_rx.recv().await {
                let mut bytes = cloned_bytes.lock().await;
                bytes.extend_from_slice(&new_bytes);
            }
        });
        Ok(Self {
            full_size,
            streamed_bytes,
            receiver,
        })
    }

    pub async fn save_file<P: AsRef<std::path::Path>>(&self, file_path: P) {
        let mut file = tokio::fs::File::create(file_path.as_ref()).await.unwrap();
        let mut new_rx = self.receiver.resubscribe();
        let bytes = self.streamed_bytes.lock().await;
        file.write_all(&bytes).await.unwrap();
        drop(bytes);
        loop {
            match new_rx.recv().await {
                Ok(data) => file.write_all(&data).await.unwrap(),
                Err(broadcast::error::RecvError::Closed) => {
                    if file.metadata().await.unwrap().len() < self.full_size {
                        unimplemented!()
                    }
                    break;
                }
                Err(_) => break,
            }
        }
    }
}

#[derive(Debug)]
pub enum PlaybackInstructions {
    NewStream(BytesWrapper),
    Pause,
    Play,
    Speed(f32),
    Seek(u64),
}

#[derive(Debug)]
pub enum ReceivedData {
    NewTimestamp(u64),
    NewMetadata(MetadataRevision),
}

async fn stream_podcast(
    send: mpsc::Sender<PlaybackInstructions>,
    stream: Stream,
) -> Result<(), Box<dyn std::error::Error>> {
    let (bytes_send, bytes_recv) = mpsc::channel(1);
    send.send(PlaybackInstructions::NewStream(BytesWrapper {
        recv: bytes_recv,
        bytes: bytes::BytesMut::new(), // response.bytes().await?,
        cursor: 0,
    }))
    .await?;
    match stream {
        Stream::Url(url) => {
            let mut download = Downloader::start(url).await?;
            while let Ok(chunk) = download.receiver.recv().await {
                bytes_send.send(chunk).await.unwrap();
            }
            download.save_file("test.mp3").await;
        }
        Stream::File(file_path) => {
            let mut file = tokio::fs::File::open(file_path).await?;
            let mut bytes = vec![];
            let maybe_chunk = file.read_to_end(&mut bytes).await;
            if let Ok(_) = maybe_chunk {
                bytes_send.send(bytes.into()).await.unwrap();
            }
        }
    }
    // send.send(PlaybackInstructions::Speed(2.0)).await?;
    // send.send(PlaybackInstructions::Play).await?;
    Ok(())
}

#[derive(Debug)]
pub struct BytesWrapper {
    recv: mpsc::Receiver<bytes::Bytes>,
    bytes: bytes::BytesMut,
    cursor: u64,
}

impl std::io::Read for BytesWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let cursor = self.cursor as usize;
        let read_length = buf.len();
        while cursor + read_length > self.bytes.len() {
            use bytes::BufMut;
            if let Some(res) = self.recv.blocking_recv() {
                self.bytes.put(res);
            }
        }
        buf[..read_length].copy_from_slice(&self.bytes[cursor..cursor + read_length]);
        self.cursor += read_length as u64;
        tracing::debug!("{} / {}", self.cursor, self.bytes.len());
        Ok(read_length)
    }
}

impl std::io::Seek for BytesWrapper {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::End(end) => {
                if -end > self.bytes.len() as i64 {
                    return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, ""));
                }
                self.cursor = (self.bytes.len() as u64).wrapping_add(end as u64);
            }
            std::io::SeekFrom::Start(start) => {
                self.cursor = (self.bytes.len() as u64).min(self.cursor + start);
            }
            std::io::SeekFrom::Current(current) => {
                if -current > self.cursor as i64 {
                    return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, ""));
                }
                self.cursor = self.cursor.wrapping_add(current as u64);
            }
        }
        Ok(self.cursor)
    }
}

impl MediaSource for BytesWrapper {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}
