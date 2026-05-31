// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Wire protocol for the 4DA inference sidecar.
//!
//! Phase 0 of the inference-sidecar plan
//! (`.claude/plans/reranker-embedding-sidecar-design.md`): the request/response
//! types and the length-prefixed framing that the main process and the
//! `fourda-infer` sidecar will speak over stdin/stdout. Pure data + (de)framing,
//! no model code — so it compiles and tests with zero heavy dependencies and
//! adds no risk to the main app.
//!
//! Frame layout: `[u32 little-endian length][JSON payload]`. JSON is used for
//! v1 because it is debuggable and dependency-light; embedding vectors are the
//! only bulk payload and at 768 dims per item over bounded batches the overhead
//! is negligible. A later phase may switch the payload codec to bincode without
//! changing this frame layout.

use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Hard cap on a single frame's payload size (256 MiB). Guards `read_frame`
/// against a corrupt/hostile length prefix allocating unbounded memory.
pub const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;

/// A request from the main process to the inference sidecar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum InferRequest {
    /// Liveness probe.
    Ping,
    /// Embed a batch of texts. Response: `InferResponse::Embeddings`.
    Embed { texts: Vec<String> },
    /// Cross-encoder rerank `docs` against `query`. Response:
    /// `InferResponse::Rankings` (one entry per doc, original index preserved).
    Rerank { query: String, docs: Vec<String> },
    /// Ask the sidecar to drop its models and free the ONNX arena.
    Unload,
}

/// A response from the inference sidecar to the main process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum InferResponse {
    /// Reply to `Ping`.
    Pong,
    /// One embedding vector per input text, in input order.
    Embeddings { vectors: Vec<Vec<f32>> },
    /// One ranking per input doc. `index` is the doc's original position.
    Rankings { items: Vec<RerankItem> },
    /// Reply to `Unload`.
    Unloaded,
    /// The sidecar could not service the request (e.g. RAM-gated, model load
    /// failed). The caller falls back to PASIFA-only ranking — not a crash.
    Error { message: String },
}

/// A single cross-encoder ranking result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RerankItem {
    /// Original index of the doc in the request's `docs` vector.
    pub index: usize,
    /// Cross-encoder relevance score in `[0.0, 1.0]`.
    pub score: f32,
}

/// Write a length-prefixed JSON frame. Flushes so the peer can read immediately.
pub fn write_frame<W: Write, T: Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(msg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame payload exceeds MAX_FRAME_BYTES",
        ));
    }
    let len = bytes.len() as u32;
    w.write_all(&len.to_le_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}

/// Read one length-prefixed JSON frame. Returns `UnexpectedEof` cleanly when the
/// peer has closed (so a supervisor can detect a dead sidecar).
pub fn read_frame<R: Read, T: DeserializeOwned>(r: &mut R) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incoming frame length exceeds MAX_FRAME_BYTES",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn roundtrip_req(req: &InferRequest) -> InferRequest {
        let mut buf = Vec::new();
        write_frame(&mut buf, req).expect("write");
        let mut cur = Cursor::new(buf);
        read_frame(&mut cur).expect("read")
    }

    #[test]
    fn ping_roundtrips() {
        assert_eq!(roundtrip_req(&InferRequest::Ping), InferRequest::Ping);
    }

    #[test]
    fn embed_request_roundtrips() {
        let req = InferRequest::Embed {
            texts: vec!["tauri".into(), "rust async".into()],
        };
        assert_eq!(roundtrip_req(&req), req);
    }

    #[test]
    fn rerank_request_roundtrips() {
        let req = InferRequest::Rerank {
            query: "rust tauri".into(),
            docs: vec!["a".into(), "b".into(), "c".into()],
        };
        assert_eq!(roundtrip_req(&req), req);
    }

    #[test]
    fn embedding_response_preserves_dims_and_values() {
        // 768-dim is the production embedding width (SnowflakeArcticEmbedMQ).
        let v0: Vec<f32> = (0..768).map(|i| i as f32 * 0.001).collect();
        let v1: Vec<f32> = (0..768).map(|i| 1.0 - i as f32 * 0.001).collect();
        let resp = InferResponse::Embeddings {
            vectors: vec![v0.clone(), v1.clone()],
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &resp).expect("write");
        let mut cur = Cursor::new(buf);
        let got: InferResponse = read_frame(&mut cur).expect("read");
        match got {
            InferResponse::Embeddings { vectors } => {
                assert_eq!(vectors.len(), 2);
                assert_eq!(vectors[0].len(), 768);
                assert_eq!(vectors[0], v0);
                assert_eq!(vectors[1], v1);
            }
            other => panic!("expected Embeddings, got {other:?}"),
        }
    }

    #[test]
    fn rankings_response_roundtrips() {
        let resp = InferResponse::Rankings {
            items: vec![
                RerankItem { index: 2, score: 0.91 },
                RerankItem { index: 0, score: 0.40 },
            ],
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &resp).expect("write");
        let mut cur = Cursor::new(buf);
        let got: InferResponse = read_frame(&mut cur).expect("read");
        assert_eq!(got, resp);
    }

    #[test]
    fn two_frames_read_back_to_back() {
        // The transport must support a stream of frames on one pipe.
        let mut buf = Vec::new();
        write_frame(&mut buf, &InferRequest::Ping).unwrap();
        write_frame(
            &mut buf,
            &InferRequest::Embed { texts: vec!["x".into()] },
        )
        .unwrap();
        let mut cur = Cursor::new(buf);
        let a: InferRequest = read_frame(&mut cur).unwrap();
        let b: InferRequest = read_frame(&mut cur).unwrap();
        assert_eq!(a, InferRequest::Ping);
        assert_eq!(b, InferRequest::Embed { texts: vec!["x".into()] });
    }

    #[test]
    fn closed_pipe_is_clean_eof() {
        // A dead sidecar (closed pipe) must surface as EOF, not a panic — the
        // supervisor relies on this to detect a crashed worker.
        let mut cur = Cursor::new(Vec::<u8>::new());
        let err = read_frame::<_, InferRequest>(&mut cur).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn oversized_length_prefix_is_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&u32::MAX.to_le_bytes()); // 4 GiB claimed
        let mut cur = Cursor::new(buf);
        let err = read_frame::<_, InferRequest>(&mut cur).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
