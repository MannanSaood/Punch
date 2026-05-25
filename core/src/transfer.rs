//! IDM-style chunked parallel file transfer over Iroh QUIC.
//!
//! Iroh handles hole punching, QUIC streams, and relay fallback automatically.
//! Their relay network (iroh.network) is used — not your Render instance.
//!
//! Key fixes from v1:
//! - NodeAddr from iroh::NodeAddr
//! - node_addr via endpoint.watch_addr().get()  
//! - Endpoint::builder().alpns().bind() — correct API
//! - recv.read_u32() returns Result<u32> not Result<Option<u32>>
//! - Connection type via conn.paths() not PathType (removed in 0.96)

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Context;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use iroh::endpoint::Connection;
use iroh::{Endpoint, EndpointAddr, Watcher};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

/// Concurrent bidirectional streams (each handles a slice of chunk indices).
/// More streams help fill the pipe on high-latency or relay paths; very high values
/// can increase disk contention—8 is a practical default.
pub const PARALLEL_STREAMS: u64     = 8;
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub const RETRY_ATTEMPTS: u32       = 5;
pub const RETRY_DELAY: Duration     = Duration::from_secs(3);
pub const SAVE_INTERVAL_BYTES: u64  = 512 * 1024;

/// Read/write buffer per copy loop (larger = fewer syscalls; ~256KiB is a good QUIC sweet spot).
pub const IO_COPY_BUFFER: usize = 256 * 1024;

pub const PUNCH_TRANSFER_ALPN: &[u8] = b"punch/file-transfer/1";

pub fn chunk_size_for(file_size: u64) -> u64 {
    const MB: u64 = 1024 * 1024;
    match file_size {
        0..=104_857_600                => MB,
        104_857_601..=1_073_741_824    => 4 * MB,
        1_073_741_825..=10_737_418_240 => 16 * MB,
        _                             => 64 * MB,
    }
}

pub fn chunk_size_label(file_size: u64) -> &'static str {
    match file_size {
        0..=104_857_600                => "1MB",
        104_857_601..=1_073_741_824    => "4MB",
        1_073_741_825..=10_737_418_240 => "16MB",
        _                             => "64MB",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferMeta {
    pub filename: String,
    pub total_size: u64,
    pub chunk_count: u64,
    pub chunk_size: u64,
    pub file_checksum: String,
    /// JSON-serialized iroh::endpoint::EndpointAddr — contains NodeId + relay URL + direct addrs
    pub node_addr: String,
    pub parallel_streams: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkState {
    pub index: u64,
    pub offset: u64,
    pub size: u64,
    pub received: u64,
    pub checksum: String,
    pub done: bool,
}

// ─── SENDER ──────────────────────────────────────────────────────────────────

pub async fn prepare_send(path: &Path) -> anyhow::Result<(TransferMeta, Endpoint)> {
    let file = tokio::fs::File::open(path).await
        .with_context(|| format!("Could not open: {}", path.display()))?;

    let total_size = file.metadata().await?.len();
    let filename   = path.file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
        .to_string_lossy()
        .to_string();

    println!("📁 File: {} ({:.1} MB)", filename, total_size as f64 / (1024.0 * 1024.0));
    print!("🔐 Computing checksum... ");
    let file_checksum = checksum_file(path).await?;
    println!("done");

    let chunk_size  = chunk_size_for(total_size);
    let chunk_count = total_size.div_ceil(chunk_size);
    println!("📦 {} chunks × {} (dynamic sizing)", chunk_count, chunk_size_label(total_size));

    print!("🔌 Starting Iroh endpoint... ");

    // Build endpoint with our ALPN
    let endpoint = Endpoint::builder()
        .alpns(vec![PUNCH_TRANSFER_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to create Iroh endpoint")?;

    // Wait for endpoint to come online (connects to home relay)
    endpoint.online().await;

    // Get NodeAddr via watcher — correct API in iroh 0.96
    let node_addr: EndpointAddr = endpoint.watch_addr().get();
    println!("done");
    println!("🌐 Node ID: {}", node_addr.id);

    let node_addr_str = serde_json::to_string(&node_addr)
        .context("Failed to serialize NodeAddr")?;

    let meta = TransferMeta {
        filename,
        total_size,
        chunk_count,
        chunk_size,
        file_checksum,
        node_addr: node_addr_str,
        parallel_streams: PARALLEL_STREAMS,
    };

    Ok((meta, endpoint))
}

pub async fn run_sender(
    path: &Path,
    endpoint: Endpoint,
    meta: &TransferMeta,
) -> anyhow::Result<()> {
    let mp    = MultiProgress::new();
    let style = progress_style();

    let total_bar = Arc::new(mp.add(ProgressBar::new(meta.total_size)));
    total_bar.set_style(style.clone());
    total_bar.set_prefix("Total");

    let path = Arc::new(path.to_path_buf());
    let bytes_sent = Arc::new(AtomicU64::new(0));
    let total_file_size = meta.total_size;

    println!("Waiting for receiver to connect...\n");

    // Accept one QUIC connection from receiver
    let conn = tokio::time::timeout(
        Duration::from_secs(120),
        async {
            loop {
                match endpoint.accept().await {
                    Some(incoming) => {
                        match incoming.await {
                            Ok(conn) => return Ok(conn),
                            Err(e)   => tracing::warn!("Accept error: {}", e),
                        }
                    }
                    None => anyhow::bail!("Endpoint closed"),
                }
            }
        }
    ).await
    .context("Timed out waiting for receiver")??;

    tracing::debug!("Receiver connected");

    let mut handles = Vec::new();

    for i in 0..meta.parallel_streams {
        let conn      = conn.clone();
        let path      = Arc::clone(&path);
        let total_bar = Arc::clone(&total_bar);
        let bytes_sent = Arc::clone(&bytes_sent);
        let pb        = mp.add(ProgressBar::new(0));
        pb.set_style(style.clone());
        pb.set_prefix(format!("Stream {}", i));

        handles.push(tokio::spawn(async move {
            match conn.accept_bi().await {
                Ok((send, recv)) => {
                    serve_chunk_stream(send, recv, path, pb, total_bar, bytes_sent, total_file_size)
                        .await
                }
                Err(e)           => Err(anyhow::anyhow!("Stream accept: {}", e)),
            }
        }));
    }

    for handle in handles {
        handle.await.context("Task panicked")?.context("Stream error")?;
    }

    total_bar.set_position(meta.total_size);
    total_bar.finish_with_message("All chunks sent");
    println!("\n✅ Transfer complete.");
    endpoint.close().await;
    Ok(())
}

async fn serve_chunk_stream(
    mut send: iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
    path: Arc<PathBuf>,
    pb: ProgressBar,
    total_bar: Arc<ProgressBar>,
    bytes_sent: Arc<AtomicU64>,
    total_file_size: u64,
) -> anyhow::Result<()> {
    let file_size  = tokio::fs::metadata(path.as_path()).await?.len();
    let chunk_size = chunk_size_for(file_size);
    let mut file   = tokio::fs::File::open(path.as_path()).await?;

    while let Ok(chunk_index) = recv.read_u64().await {
        let resume_offset = recv.read_u64().await?;

        let chunk_start = chunk_index * chunk_size;
        let chunk_end   = ((chunk_index + 1) * chunk_size).min(file_size);
        let file_offset = chunk_start + resume_offset;
        let remaining   = chunk_end.saturating_sub(file_offset);

        pb.set_prefix(format!("Chunk {}", chunk_index));
        pb.set_length(remaining);
        pb.set_position(0);

        // Chunk hash is always over [chunk_start, chunk_end). On resume, hash the prefix from disk
        // without sending, then send and hash the rest in one pass.
        let mut hasher = Sha256::new();
        file.seek(std::io::SeekFrom::Start(chunk_start)).await?;
        if resume_offset > 0 {
            let mut prefix_left = resume_offset;
            let mut pbuf = vec![0u8; IO_COPY_BUFFER];
            while prefix_left > 0 {
                let take = (prefix_left as usize).min(pbuf.len());
                file.read_exact(&mut pbuf[..take]).await?;
                hasher.update(&pbuf[..take]);
                prefix_left -= take as u64;
            }
        } else {
            file.seek(std::io::SeekFrom::Start(file_offset)).await?;
        }

        let mut buf  = vec![0u8; IO_COPY_BUFFER];
        let mut sent = 0u64;

        while sent < remaining {
            let to_read = ((remaining - sent) as usize).min(buf.len());
            let n = file.read(&mut buf[..to_read]).await?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
            send.write_all(&buf[..n]).await?;
            sent += n as u64;
            pb.inc(n as u64);
            let total = bytes_sent.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
            total_bar.set_position(total.min(total_file_size));
        }

        let cs = format!("{:x}", hasher.finalize());
        send.write_u32(cs.len() as u32).await?;
        send.write_all(cs.as_bytes()).await?;

        pb.finish_with_message(format!("chunk {} done", chunk_index));

        let ack = recv.read_u8().await?;
        if ack != 1 {
            tracing::warn!("Chunk {} rejected", chunk_index);
        }
    }
    Ok(())
}

// ─── RECEIVER ────────────────────────────────────────────────────────────────

pub async fn run_receiver(
    meta: &TransferMeta,
    dest_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let dest    = dest_dir.join(&meta.filename);
    let partial = dest_dir.join(format!("{}.punch_partial", meta.filename));

    println!("📥 Receiving: {} ({:.1} MB)", meta.filename, meta.total_size as f64 / (1024.0 * 1024.0));
    println!("📂 Saving to: {}", dest.display());

    let node_addr: EndpointAddr = serde_json::from_str(&meta.node_addr)
        .context("Failed to parse sender NodeAddr")?;

    println!("🌐 Sender node: {}", node_addr.id);

    print!("🔌 Starting Iroh endpoint... ");
    let endpoint = Endpoint::builder()
        .alpns(vec![PUNCH_TRANSFER_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to create Iroh endpoint")?;

    endpoint.online().await;
    println!("done");

    println!("🔗 Connecting (hole punch or relay, automatic)...");
    let conn = tokio::time::timeout(
        CONNECT_TIMEOUT,
        endpoint.connect(node_addr, PUNCH_TRANSFER_ALPN)
    ).await
    .context("Connection timed out")?
    .context("Failed to connect to sender")?;

    // Show connection type — simplified, works regardless of API version
    println!("🔗 QUIC connection established via Iroh");

    let chunk_states = Arc::new(Mutex::new(
        load_or_create_chunk_states(&partial, meta).await?
    ));

    {
        let f = tokio::fs::OpenOptions::new()
            .write(true).create(true).truncate(false).open(&partial).await?;
        f.set_len(meta.total_size).await?;
    }

    let mp    = MultiProgress::new();
    let style = progress_style();

    let total_bar = Arc::new(mp.add(ProgressBar::new(meta.total_size)));
    total_bar.set_style(style.clone());
    total_bar.set_prefix("Total");

    {
        let states = chunk_states.lock().await;
        total_bar.set_position(bytes_completed(&states).min(meta.total_size));
    }

    let n_streams = meta.parallel_streams.max(1);
    let chunks_per_stream = meta.chunk_count.div_ceil(n_streams);
    let mut handles       = Vec::new();

    for stream_id in 0..n_streams {
        let start_chunk = stream_id * chunks_per_stream;
        let end_chunk   = ((stream_id + 1) * chunks_per_stream).min(meta.chunk_count);
        if start_chunk >= meta.chunk_count { break; }

        let conn         = conn.clone();
        let partial      = partial.clone();
        let chunk_states = Arc::clone(&chunk_states);
        let total_bar    = Arc::clone(&total_bar);
        let pb           = mp.add(ProgressBar::new(0));
        pb.set_style(style.clone());
        pb.set_prefix(format!("Stream {}", stream_id));
        let meta = meta.clone();

        handles.push(tokio::spawn(async move {
            receive_chunk_stream(
                conn, start_chunk, end_chunk,
                partial, chunk_states, pb, total_bar, meta,
            ).await
        }));
    }

    for handle in handles {
        handle.await.context("Task panicked")?.context("Stream error")?;
    }

    {
        let states = chunk_states.lock().await;
        let completed = bytes_completed(&states);
        total_bar.set_position(completed.min(meta.total_size));
    }
    total_bar.finish_with_message("Verifying...");

    print!("\n🔐 Verifying file integrity... ");
    let final_checksum = checksum_file(&partial).await?;
    if final_checksum != meta.file_checksum {
        anyhow::bail!(
            "File checksum mismatch.\nExpected: {}\nGot:      {}",
            meta.file_checksum, final_checksum
        );
    }
    println!("✅ Verified.");

    tokio::fs::rename(&partial, &dest).await?;
    let state_file = dest_dir.join(format!("{}.punch_state", meta.filename));
    let _ = tokio::fs::remove_file(state_file).await;
    total_bar.set_position(meta.total_size);
    total_bar.finish_with_message("Complete");
    println!("Saved to: {}", dest.display());

    endpoint.close().await;
    Ok(dest)
}

#[allow(clippy::too_many_arguments)]
async fn receive_chunk_stream(
    conn: Connection,
    start_chunk: u64,
    end_chunk: u64,
    partial: PathBuf,
    chunk_states: Arc<Mutex<Vec<ChunkState>>>,
    pb: ProgressBar,
    total_bar: Arc<ProgressBar>,
    meta: TransferMeta,
) -> anyhow::Result<()> {
    let mut attempt = 0;

    'retry: loop {
        attempt += 1;
        if attempt > RETRY_ATTEMPTS {
            anyhow::bail!("Too many retries for chunks {}-{}", start_chunk, end_chunk);
        }

        if attempt > 1 {
            println!("\n🔄 Retrying stream (attempt {}/{})...", attempt, RETRY_ATTEMPTS);
            tokio::time::sleep(RETRY_DELAY).await;
        }

        let (mut send, mut recv) = match conn.open_bi().await {
            Ok(s)  => s,
            Err(e) => {
                tracing::warn!("Failed to open QUIC stream: {}", e);
                println!("⚠️  Stream failed — retrying (data preserved)");
                continue 'retry;
            }
        };

        for chunk_index in start_chunk..end_chunk {
            let (done, resume_from) = {
                let states = chunk_states.lock().await;
                let s      = &states[chunk_index as usize];
                (s.done, s.received)
            };

            if done {
                sync_total_progress(&total_bar, &chunk_states, meta.total_size).await;
                continue;
            }

            let chunk_start = chunk_index * meta.chunk_size;
            let chunk_end   = (chunk_start + meta.chunk_size).min(meta.total_size);
            let chunk_size  = chunk_end - chunk_start;

            // Idempotent check — skip if already verified on disk
            if resume_from == chunk_size {
                if let Ok(on_disk_cs) = checksum_range(&partial, chunk_start, chunk_end).await {
                    tracing::debug!("Chunk {} fully on disk — skipping", chunk_index);
                    let mut states = chunk_states.lock().await;
                    states[chunk_index as usize].done     = true;
                    states[chunk_index as usize].received = chunk_size;
                    states[chunk_index as usize].checksum = on_disk_cs;
                    save_chunk_states(&partial, &states).await?;
                    sync_total_progress(&total_bar, &chunk_states, meta.total_size).await;
                    continue;
                }
            }

            let remaining = chunk_size - resume_from;
            pb.set_prefix(format!("Chunk {}", chunk_index));
            pb.set_length(remaining);
            pb.set_position(0);

            // Request chunk — connection drop handled by stream error
            if let Err(e) = send.write_u64(chunk_index).await {
                tracing::warn!("Write failed (connection drop): {}", e);
                save_progress(&chunk_states, chunk_index, resume_from, &partial).await?;
                println!("⚠️  Connection dropped — data preserved, retrying");
                continue 'retry;
            }
            if let Err(e) = send.write_u64(resume_from).await {
                save_progress(&chunk_states, chunk_index, resume_from, &partial).await?;
                tracing::warn!("Write failed: {}", e);
                continue 'retry;
            }

            let write_offset = chunk_start + resume_from;
            let mut received = resume_from;

            let mut out = tokio::fs::OpenOptions::new()
                .write(true).open(&partial).await?;
            out.seek(std::io::SeekFrom::Start(write_offset)).await?;

            let mut buf             = vec![0u8; IO_COPY_BUFFER];
            let mut since_last_save = 0u64;

            let mut hasher = Sha256::new();
            if resume_from > 0 {
                let mut partial_r = tokio::fs::File::open(&partial).await?;
                partial_r.seek(std::io::SeekFrom::Start(chunk_start)).await?;
                let mut left = resume_from;
                let mut hbuf = vec![0u8; IO_COPY_BUFFER];
                while left > 0 {
                    let take = (left as usize).min(hbuf.len());
                    partial_r.read_exact(&mut hbuf[..take]).await?;
                    hasher.update(&hbuf[..take]);
                    left -= take as u64;
                }
            }

            while received < chunk_size {
                let to_read = ((chunk_size - received) as usize).min(buf.len());

                // Iroh RecvStream read() returns Result<Option<usize>>
                // Ok(Some(n)) = got n bytes, Ok(None) = stream closed
                let n = match recv.read(&mut buf[..to_read]).await {
                    Ok(Some(0)) | Ok(None) => {
                        // Stream ended mid-chunk = connection drop
                        save_progress(&chunk_states, chunk_index, received, &partial).await?;
                        println!("⚠️  Stream closed at {}/{} bytes — resuming", received, chunk_size);
                        continue 'retry;
                    }
                    Ok(Some(n)) => n,
                    Err(e) => {
                        tracing::warn!("Read error (connection drop): {}", e);
                        save_progress(&chunk_states, chunk_index, received, &partial).await?;
                        println!("⚠️  Connection error — data preserved, retrying");
                        continue 'retry;
                    }
                };

                if let Err(e) = out.write_all(&buf[..n]).await {
                    anyhow::bail!("Disk write failed: {} — check available space", e);
                }

                hasher.update(&buf[..n]);
                received        += n as u64;
                since_last_save += n as u64;
                pb.inc(n as u64);
                {
                    let mut states = chunk_states.lock().await;
                    states[chunk_index as usize].received = received;
                    let pos = bytes_completed(&states);
                    total_bar.set_position(pos.min(meta.total_size));
                }

                if since_last_save >= SAVE_INTERVAL_BYTES {
                    save_progress(&chunk_states, chunk_index, received, &partial).await?;
                    since_last_save = 0;
                }
            }

            // read_u32 returns Result<u32> — NOT Result<Option<u32>>
            let cs_len = match recv.read_u32().await {
                Ok(n)  => n as usize,
                Err(e) => {
                    tracing::warn!("Lost connection reading checksum: {}", e);
                    save_progress(&chunk_states, chunk_index, received, &partial).await?;
                    println!("⚠️  Lost connection before checksum — will re-verify on retry");
                    continue 'retry;
                }
            };

            let mut cs_buf = vec![0u8; cs_len];
            if let Err(e) = recv.read_exact(&mut cs_buf).await {
                tracing::warn!("Lost connection reading checksum bytes: {}", e);
                save_progress(&chunk_states, chunk_index, received, &partial).await?;
                continue 'retry;
            }

            let expected_cs = String::from_utf8(cs_buf)?;
            let actual_cs   = format!("{:x}", hasher.finalize());

            if actual_cs != expected_cs {
                // DATA corruption — reset chunk
                tracing::warn!("Chunk {} DATA corrupt — resetting", chunk_index);
                println!("⚠️  Chunk {} data corrupt — resetting", chunk_index);
                let _ = send.write_u8(0).await; // NAK
                {
                    let mut states = chunk_states.lock().await;
                    states[chunk_index as usize].received = 0;
                    states[chunk_index as usize].done     = false;
                    save_chunk_states(&partial, &states).await?;
                }
                continue 'retry;
            }

            // ACK — connection drop here is safe, idempotent check handles it
            if let Err(e) = send.write_u8(1).await {
                tracing::warn!("Lost connection sending ACK: {}", e);
                let mut states = chunk_states.lock().await;
                states[chunk_index as usize].done     = true;
                states[chunk_index as usize].received = chunk_size;
                states[chunk_index as usize].checksum = actual_cs;
                save_chunk_states(&partial, &states).await?;
                println!("⚠️  Connection dropped after verify — chunk safe, retrying");
                continue 'retry;
            }

            {
                let mut states = chunk_states.lock().await;
                states[chunk_index as usize].done     = true;
                states[chunk_index as usize].received = chunk_size;
                states[chunk_index as usize].checksum = actual_cs;
                save_chunk_states(&partial, &states).await?;
                sync_total_progress(&total_bar, &chunk_states, meta.total_size).await;
            }

            pb.finish_with_message(format!("chunk {}", chunk_index));
        }

        break 'retry;
    }

    Ok(())
}

// ─── HELPERS ─────────────────────────────────────────────────────────────────

fn bytes_completed(states: &[ChunkState]) -> u64 {
    states
        .iter()
        .map(|c| if c.done { c.size } else { c.received.min(c.size) })
        .sum()
}

async fn sync_total_progress(
    total_bar: &ProgressBar,
    chunk_states: &Arc<Mutex<Vec<ChunkState>>>,
    total_size: u64,
) {
    let pos = {
        let states = chunk_states.lock().await;
        bytes_completed(&states)
    };
    total_bar.set_position(pos.min(total_size));
}

async fn save_progress(
    chunk_states: &Arc<Mutex<Vec<ChunkState>>>,
    chunk_index: u64,
    received: u64,
    partial: &Path,
) -> anyhow::Result<()> {
    let mut states = chunk_states.lock().await;
    states[chunk_index as usize].received = received;
    save_chunk_states(partial, &states).await
}

fn progress_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{prefix:.cyan} [{bar:40.green/dim}] {bytes}/{total_bytes} ({bytes_per_sec}, eta {eta})"
    )
    .unwrap()
    .progress_chars("█▉▊▋▌▍▎▏ ")
}

async fn load_or_create_chunk_states(
    partial: &Path,
    meta: &TransferMeta,
) -> anyhow::Result<Vec<ChunkState>> {
    let state_file = partial.with_extension("punch_state");

    if state_file.exists() {
        let content = tokio::fs::read_to_string(&state_file).await?;
        if let Ok(states) = serde_json::from_str::<Vec<ChunkState>>(&content) {
            let done_count = states.iter().filter(|c| c.done).count();
            if done_count > 0 {
                println!("↩️  Resuming: {}/{} chunks already verified", done_count, states.len());
            }
            return Ok(states);
        }
    }

    Ok((0..meta.chunk_count).map(|i| {
        let offset = i * meta.chunk_size;
        let size   = meta.chunk_size.min(meta.total_size - offset);
        ChunkState { index: i, offset, size, received: 0, checksum: String::new(), done: false }
    }).collect())
}

async fn save_chunk_states(partial: &Path, states: &[ChunkState]) -> anyhow::Result<()> {
    let state_file = partial.with_extension("punch_state");
    tokio::fs::write(state_file, serde_json::to_string(states)?).await?;
    Ok(())
}

async fn checksum_file(path: &Path) -> anyhow::Result<String> {
    let data = tokio::fs::read(path).await?;
    Ok(format!("{:x}", Sha256::digest(&data)))
}

async fn checksum_range(path: &Path, start: u64, end: u64) -> anyhow::Result<String> {
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut buf = vec![0u8; (end - start) as usize];
    file.read_exact(&mut buf).await?;
    Ok(format!("{:x}", Sha256::digest(&buf)))
}