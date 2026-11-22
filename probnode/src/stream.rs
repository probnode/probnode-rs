//! The real-time stream `/v2/live`: the subscribe message, and the decoder of a Probe (binary)
//! frame. Bring the WebSocket client you already use (tokio-tungstenite, ...): connect with
//! `Authorization: Bearer <key>` (or `?key=`), send [`subscribe_message`], and pass every binary
//! frame to [`decode`].
//!
//! ```
//! let msg = probnode::stream::subscribe_message(
//!     &["btc-updown-15m-1795366800"],
//!     &["book+", "trades+"],
//!     probnode::stream::StreamFormat::Probe,
//!     None,
//! );
//! assert!(msg.contains("\"format\":\"probe\""));
//! ```

use serde_json::json;

/// The wire format of a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFormat {
    /// JSON text frames.
    Json,
    /// Probe records in binary frames.
    Probe,
}

/// The subscribe message: markets, channels, format, and the probnode blocks (`None` = all).
pub fn subscribe_message(
    markets: &[&str],
    channels: &[&str],
    format: StreamFormat,
    blocks: Option<&[&str]>,
) -> String {
    let format = match format {
        StreamFormat::Json => "json",
        StreamFormat::Probe => "probe",
    };
    let mut m = json!({"op": "sub", "markets": markets, "channels": channels, "format": format});
    if let Some(b) = blocks {
        m["blocks"] = json!(b);
    }
    m.to_string()
}

/// Record types.
pub mod rtype {
    /// A book level change.
    pub const BOOK_LEVEL: u8 = 0x01;
    /// A trade.
    pub const TRADE: u8 = 0x02;
    /// The probnode object of a book event or a print.
    pub const PROBNODE: u8 = 0x14;
}

/// The metrics of a probnode record, in record order.
pub const PN_NAMES: [&str; 19] = [
    "spread",
    "depth_bid",
    "depth_ask",
    "imbalance",
    "microprice_lean",
    "updates_per_s",
    "volume_1m",
    "trades_1m",
    "taker_imbalance_1m",
    "avg_trade",
    "cost_to_trade.buy",
    "cost_to_trade.sell",
    "depth",
    "resilience",
    "volatility.10s",
    "volatility.1m",
    "adverse_selection",
    "quote_stability",
    "resolution_jump",
];
const PN_FACTORS: usize = 9;
const MISSING: i64 = i64::MIN;
const PN_NONE: i32 = i32::MIN;

/// One metric against the norms.
#[derive(Debug, Clone, PartialEq)]
pub struct Metric {
    /// The value, in the metric's units; `None` when it cannot be computed now.
    pub value: Option<f64>,
    /// value / the bucket's median; `None` for signed metrics or without norms.
    pub x_norm: Option<f64>,
    /// Percentile among similar markets.
    pub p_bucket: Option<u8>,
    /// Percentile against the market's own last 30 minutes.
    pub p_self: Option<u8>,
    /// For a risk factor: the next minute's absolute mid move (p50, p90) on similar markets.
    pub after_move_60s: Option<(f64, f64)>,
}

/// A decoded record.
#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    /// A book level change (prices 0..1, sizes in shares).
    Book {
        /// Instrument id (the symbol dictionary).
        instrument: u32,
        /// Venue time, ns.
        ts_event: u64,
        /// Our receive time, ns.
        ts_recv: u64,
        /// Price.
        price: f64,
        /// Size.
        size: f64,
        /// Per-market sequence.
        seq: u32,
        /// 0 bid, 1 ask.
        side: u8,
    },
    /// A trade.
    Trade {
        /// Instrument id.
        instrument: u32,
        /// Venue time, ns.
        ts_event: u64,
        /// Price.
        price: f64,
        /// Size.
        size: f64,
        /// Per-market sequence.
        seq: u32,
        /// 0 buy, 1 sell.
        side: u8,
    },
    /// The probnode object of the event `reference` names.
    Probnode {
        /// Instrument id.
        instrument: u32,
        /// The BookExt seq (book) or the trade seq (trade) it belongs to.
        reference: u32,
        /// "book" or "trade".
        kind: &'static str,
        /// The bucket of similar markets.
        bucket: String,
        /// The norms' version.
        stats_version: String,
        /// Security: a confirmed case names the market.
        flagged: bool,
        /// Security episodes open.
        open: u8,
        /// The metrics, in [`PN_NAMES`] order.
        metrics: Vec<Metric>,
    },
    /// A type this decoder does not open: its type and bytes.
    Other {
        /// Record type.
        rtype: u8,
        /// The record's bytes.
        raw: Vec<u8>,
    },
}

/// A frame that does not split into whole records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    /// Where the bad record starts.
    pub at: usize,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bad Probe record at byte {}", self.at)
    }
}

impl std::error::Error for DecodeError {}

fn u32_at(p: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]])
}

fn u64_at(p: &[u8], o: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&p[o..o + 8]);
    u64::from_le_bytes(b)
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn pct(v: u8) -> Option<u8> {
    (v != 255).then_some(v)
}

/// The records of one binary frame.
pub fn decode(frame: &[u8]) -> Result<Vec<Record>, DecodeError> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 16 <= frame.len() {
        let n = frame[i] as usize * 4;
        if n < 16 || i + n > frame.len() {
            return Err(DecodeError { at: i });
        }
        let p = &frame[i..i + n];
        let instrument = u32_at(p, 4);
        let ts_event = u64_at(p, 8);
        let rec = match p[1] {
            rtype::BOOK_LEVEL | rtype::TRADE if n >= 48 => {
                let price = u64_at(p, 24) as i64 as f64 / 1e9;
                let size = u64_at(p, 32) as i64 as f64 / 1e6;
                let seq = u32_at(p, 40);
                if p[1] == rtype::BOOK_LEVEL {
                    Record::Book {
                        instrument,
                        ts_event,
                        ts_recv: u64_at(p, 16),
                        price,
                        size,
                        seq,
                        side: p[44],
                    }
                } else {
                    Record::Trade {
                        instrument,
                        ts_event,
                        price,
                        size,
                        seq,
                        side: p[44],
                    }
                }
            }
            rtype::PROBNODE if n >= 80 + PN_NAMES.len() * 16 + PN_FACTORS * 8 => {
                let mut metrics = Vec::with_capacity(PN_NAMES.len());
                let mut o = 80;
                for _ in PN_NAMES {
                    let v = u64_at(p, o) as i64;
                    let x = u32_at(p, o + 8) as i32;
                    metrics.push(Metric {
                        value: (v != MISSING).then(|| v as f64 / 1e6),
                        x_norm: (x != PN_NONE).then(|| f64::from(x) / 1e4),
                        p_bucket: pct(p[o + 12]),
                        p_self: pct(p[o + 13]),
                        after_move_60s: None,
                    });
                    o += 16;
                }
                let first = PN_NAMES.len() - PN_FACTORS;
                for m in metrics.iter_mut().skip(first) {
                    let a50 = u32_at(p, o) as i32;
                    let a90 = u32_at(p, o + 4) as i32;
                    if a50 != PN_NONE {
                        m.after_move_60s = Some((f64::from(a50) / 1e6, f64::from(a90) / 1e6));
                    }
                    o += 8;
                }
                Record::Probnode {
                    instrument,
                    reference: u32_at(p, 24),
                    kind: if p[28] == 1 { "trade" } else { "book" },
                    bucket: cstr(&p[32..64]),
                    stats_version: cstr(&p[64..80]),
                    flagged: p[29] == 1,
                    open: p[30],
                    metrics,
                }
            }
            t => Record::Other {
                rtype: t,
                raw: p.to_vec(),
            },
        };
        out.push(rec);
        i += n;
    }
    if i != frame.len() {
        return Err(DecodeError { at: i });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_HEX: &str = include_str!("../tests/live_frame.hex");

    fn frame() -> Vec<u8> {
        let h = FRAME_HEX.trim();
        (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0))
            .collect()
    }

    #[test]
    fn decodes_the_engines_frame() {
        let recs = decode(&frame()).expect("decode");
        assert_eq!(recs.len(), 2);
        match &recs[0] {
            Record::Book {
                price, seq, size, ..
            } => {
                assert!((price - 0.49).abs() < 1e-12);
                assert_eq!(*seq, 42);
                assert!((size - 1000.0).abs() < 1e-9);
            }
            other => panic!("first record {other:?}"),
        }
        match &recs[1] {
            Record::Probnode {
                reference,
                bucket,
                stats_version,
                metrics,
                ..
            } => {
                assert_eq!(*reference, 42);
                assert_eq!(bucket, "updown-15m|>3m|.3-.7");
                assert_eq!(stats_version, "v0-fixture");
                assert_eq!(metrics[0].value, Some(0.02));
                assert_eq!(metrics[0].p_bucket, Some(50));
                assert_eq!(metrics[0].x_norm, Some(1.0));
                assert_eq!(metrics[12].value, Some(1000.0));
            }
            other => panic!("second record {other:?}"),
        }
    }

    #[test]
    fn refuses_a_truncated_frame() {
        let f = frame();
        assert!(decode(&f[..f.len() - 8]).is_err());
    }

    #[test]
    fn subscribe_with_blocks() {
        let m = subscribe_message(&["m"], &["book+"], StreamFormat::Json, Some(&["risk"]));
        assert!(m.contains("\"blocks\":[\"risk\"]") && m.contains("\"format\":\"json\""));
    }
}
