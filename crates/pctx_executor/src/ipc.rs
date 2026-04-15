/// Async JSON-lines IPC helpers shared by the pool manager and worker binary.
///
/// Each message is a single line of JSON terminated by `\n`. The writer flushes
/// must be called by the caller after writing (to avoid buffering surprises).
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// Serialize `msg` as a single JSON line and write it to `writer`.
///
/// Does **not** flush; the caller must flush when appropriate.
pub async fn write_msg<W, T>(writer: &mut W, msg: &T) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let mut line = serde_json::to_string(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await
}

/// Read one `\n`-terminated JSON line from `reader` and deserialize it.
///
/// Returns `Err` with `UnexpectedEof` if the stream ends before a complete line.
pub async fn read_msg<R, T>(reader: &mut R) -> std::io::Result<T>
where
    R: AsyncBufRead + Unpin,
    T: DeserializeOwned,
{
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "IPC channel closed (EOF)",
        ));
    }
    serde_json::from_str(line.trim_end())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
