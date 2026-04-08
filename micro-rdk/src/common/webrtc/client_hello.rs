//! DTLS ClientHello reassembly buffer.
//!
//! mbedtls rejects fragmented DTLS ClientHello messages with `-28800`.
//! This module provides a self-contained reassembly mechanism that
//! collects fragments and produces a single unfragmented record once
//! all bytes arrive.
//!
//! The buffer is designed for zero-copy operation: UDP datagrams are read
//! directly into the buffer via `recv_buf()`, then processed in-place
//! via `process_recv()`. Each datagram overwrites the header area and
//! lands its payload at `buf[25..]`; `process_recv` relocates the payload
//! to its correct offset within the reassembly region.
//!
//! Fragment overlap is not tracked — per RFC 6347, overlapping fragments
//! must carry identical content, so duplicate writes are harmless.

use std::io;

const RECORD_HDR_LEN: usize = 13;
const HANDSHAKE_HDR_LEN: usize = 12;
const TOTAL_HDR_LEN: usize = RECORD_HDR_LEN + HANDSHAKE_HDR_LEN; // 25
const MAX_PAYLOAD: usize = 4096;
/// Maximum single UDP datagram size we expect to receive.
const MAX_RECV: usize = 2048;
/// Total buffer: reassembled record area + space for one incoming datagram.
const BUF_SIZE: usize = TOTAL_HDR_LEN + MAX_PAYLOAD + MAX_RECV;

/// Parse a 24-bit big-endian integer from a 3-byte slice.
#[inline]
fn u24(b: &[u8]) -> usize {
    ((b[0] as usize) << 16) | ((b[1] as usize) << 8) | (b[2] as usize)
}

/// Encode a `usize` as 3 big-endian bytes (u24).
#[inline]
fn put_u24(buf: &mut [u8], val: usize) {
    buf[0] = (val >> 16) as u8;
    buf[1] = (val >> 8) as u8;
    buf[2] = val as u8;
}

pub(crate) struct ClientHelloBuffer {
    /// Buffer layout: `[0..25]` headers, `[25..25+MAX_PAYLOAD]` payload.
    /// Every UDP recv writes the datagram starting at `buf[0]`, overwriting
    /// the headers each time. `process_recv` then relocates the payload
    /// from `buf[25..]` to `buf[25+frag_off..]` via `copy_within`.
    buf: Box<[u8; BUF_SIZE]>,
    /// Sum of `frag_len` across all received fragments.
    received: usize,
    /// Total payload length from the handshake `length` field. 0 until the first fragment arrives.
    total_len: usize,
    /// Whether we have received any fragment yet. Controls where `recv_buf()` places data.
    has_fragments: bool,
    /// Drain cursor for mbedtls reads.
    read_offset: usize,
}

impl ClientHelloBuffer {
    pub(crate) fn new() -> Self {
        Self {
            buf: Box::new([0u8; BUF_SIZE]),
            received: 0,
            total_len: 0,
            has_fragments: false,
            read_offset: 0,
        }
    }

    /// Total reassembled record length (record header + handshake header + payload).
    fn record_len(&self) -> usize {
        TOTAL_HDR_LEN + self.total_len
    }

    /// Returns a mutable slice where the next UDP datagram should be read into.
    ///
    /// - First call (no data yet): returns `buf[0..]` so the datagram lands
    ///   directly in the reassembly area (zero-copy for unfragmented case and
    ///   first-fragment-at-offset-0).
    /// - Subsequent calls: returns `buf[25+total_len..]` so the recv doesn't
    ///   clobber previously assembled payload bytes.
    pub(crate) fn recv_buf(&mut self) -> &mut [u8] {
        if !self.has_fragments {
            // First recv: datagram lands at start for zero-copy unfragmented case
            &mut self.buf[..MAX_RECV]
        } else {
            // Subsequent recvs: land past the payload area to avoid clobbering
            let off = TOTAL_HDR_LEN + self.total_len;
            &mut self.buf[off..off + MAX_RECV]
        }
    }

    /// Process `n` bytes that were read into the slice returned by `recv_buf()`.
    ///
    /// Returns `Ok(true)` when the ClientHello is complete and ready to be drained.
    /// Returns `Ok(false)` when more fragments are needed.
    /// Returns `Err` if the datagram is not a DTLS handshake ClientHello.
    pub(crate) fn process_recv(&mut self, n: usize) -> io::Result<bool> {
        // Datagram offset matches what recv_buf() returned
        let dat_off = if !self.has_fragments {
            0
        } else {
            TOTAL_HDR_LEN + self.total_len
        };

        if n < TOTAL_HDR_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "datagram too short for DTLS handshake",
            ));
        }
        if self.buf[dat_off] != 22 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a DTLS handshake record",
            ));
        }
        if self.buf[dat_off + RECORD_HDR_LEN] != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a ClientHello message",
            ));
        }

        let msg_len = u24(&self.buf[dat_off + 14..dat_off + 17]);
        let frag_off = u24(&self.buf[dat_off + 19..dat_off + 22]);
        let frag_len = u24(&self.buf[dat_off + 22..dat_off + 25]);

        if msg_len == 0 || msg_len > MAX_PAYLOAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ClientHello payload exceeds buffer capacity",
            ));
        }
        if frag_off + frag_len > msg_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "fragment extends past message length",
            ));
        }
        if n < TOTAL_HDR_LEN + frag_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "datagram shorter than declared fragment length",
            ));
        }

        // Unfragmented: first recv, datagram at buf[0..], already in place
        if frag_off == 0 && frag_len == msg_len {
            self.received = msg_len;
            self.total_len = msg_len;
            self.has_fragments = true;
            self.read_offset = 0;
            log::debug!("DTLS ClientHello: unfragmented, {msg_len} bytes");
            return Ok(true);
        }

        log::debug!(
            "DTLS ClientHello fragment: offset={frag_off}, frag_len={frag_len}, total={msg_len}"
        );

        let first_fragment = !self.has_fragments;
        self.has_fragments = true;
        if first_fragment {
            // First fragment: datagram at buf[0..]. Headers are already in place.
            self.total_len = msg_len;
            self.read_offset = 0;
        }

        // Payload source: buf[dat_off + 25 .. dat_off + 25 + frag_len]
        // Payload dest:   buf[25 + frag_off .. 25 + frag_off + frag_len]
        let src_start = dat_off + TOTAL_HDR_LEN;
        let dst_start = TOTAL_HDR_LEN + frag_off;

        if src_start != dst_start {
            self.buf.copy_within(src_start..src_start + frag_len, dst_start);
        }

        self.received += frag_len;

        if self.received >= self.total_len {
            self.finalize_headers();
            return Ok(true);
        }

        Ok(false)
    }

    /// Patch the record and handshake headers for the reassembled (unfragmented) record.
    fn finalize_headers(&mut self) {
        let record_payload_len = (HANDSHAKE_HDR_LEN + self.total_len) as u16;
        self.buf[11] = (record_payload_len >> 8) as u8;
        self.buf[12] = record_payload_len as u8;
        put_u24(&mut self.buf[19..22], 0);
        put_u24(&mut self.buf[22..25], self.total_len);
        log::debug!(
            "DTLS ClientHello reassembled: {} bytes",
            self.record_len()
        );
    }

    /// Read reassembled data into `dest`. Returns number of bytes copied.
    /// Returns 0 when all data has been consumed.
    pub(crate) fn read(&mut self, dest: &mut [u8]) -> usize {
        let total = self.record_len();
        if self.read_offset >= total {
            return 0;
        }
        let remaining = total - self.read_offset;
        let n = dest.len().min(remaining);
        dest[..n].copy_from_slice(&self.buf[self.read_offset..self.read_offset + n]);
        self.read_offset += n;
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a complete DTLS record with handshake header for a ClientHello fragment.
    fn make_fragment(msg_len: usize, frag_off: usize, frag_len: usize, payload: &[u8]) -> Vec<u8> {
        assert_eq!(payload.len(), frag_len);
        let record_payload_len = HANDSHAKE_HDR_LEN + frag_len;
        let mut buf = Vec::with_capacity(TOTAL_HDR_LEN + frag_len);

        // Record header (13 bytes)
        buf.push(22); // content type: handshake
        buf.extend_from_slice(&[0xfe, 0xfd]); // DTLS 1.2 version
        buf.extend_from_slice(&[0x00, 0x00]); // epoch
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x01]); // sequence number
        buf.push((record_payload_len >> 8) as u8);
        buf.push(record_payload_len as u8);

        // Handshake header (12 bytes)
        buf.push(1); // msg_type: ClientHello
        buf.push((msg_len >> 16) as u8);
        buf.push((msg_len >> 8) as u8);
        buf.push(msg_len as u8);
        buf.extend_from_slice(&[0x00, 0x00]); // message_seq
        buf.push((frag_off >> 16) as u8);
        buf.push((frag_off >> 8) as u8);
        buf.push(frag_off as u8);
        buf.push((frag_len >> 16) as u8);
        buf.push((frag_len >> 8) as u8);
        buf.push(frag_len as u8);

        buf.extend_from_slice(payload);
        buf
    }

    /// Write a datagram into the buffer's recv_buf and process it.
    fn feed(buffer: &mut ClientHelloBuffer, datagram: &[u8]) -> io::Result<bool> {
        let recv = buffer.recv_buf();
        assert!(recv.len() >= datagram.len());
        recv[..datagram.len()].copy_from_slice(datagram);
        buffer.process_recv(datagram.len())
    }

    #[test]
    fn test_unfragmented_client_hello() {
        let mut buffer = ClientHelloBuffer::new();
        let payload = vec![0xCC; 200];
        let datagram = make_fragment(200, 0, 200, &payload);

        assert!(feed(&mut buffer, &datagram).unwrap());

        let mut out = vec![0u8; 4096];
        let n = buffer.read(&mut out);
        assert_eq!(n, datagram.len());
        assert_eq!(&out[..n], &datagram[..]);

        assert_eq!(buffer.read(&mut out), 0);
    }

    #[test]
    fn test_fragmented_in_order() {
        let mut buffer = ClientHelloBuffer::new();
        let payload1 = vec![0xAA; 1000];
        let payload2 = vec![0xBB; 461];
        let frag1 = make_fragment(1461, 0, 1000, &payload1);
        let frag2 = make_fragment(1461, 1000, 461, &payload2);

        assert!(!feed(&mut buffer, &frag1).unwrap());
        assert!(feed(&mut buffer, &frag2).unwrap());

        let mut out = vec![0u8; 4096];
        let n = buffer.read(&mut out);
        assert_eq!(n, TOTAL_HDR_LEN + 1461);

        let record_payload_len = ((out[11] as usize) << 8) | (out[12] as usize);
        assert_eq!(record_payload_len, HANDSHAKE_HDR_LEN + 1461);
        assert_eq!(u24(&out[19..22]), 0);
        assert_eq!(u24(&out[22..25]), 1461);

        assert!(out[TOTAL_HDR_LEN..TOTAL_HDR_LEN + 1000]
            .iter()
            .all(|&b| b == 0xAA));
        assert!(out[TOTAL_HDR_LEN + 1000..TOTAL_HDR_LEN + 1461]
            .iter()
            .all(|&b| b == 0xBB));
    }

    #[test]
    fn test_fragmented_out_of_order() {
        let mut buffer = ClientHelloBuffer::new();
        let payload1 = vec![0xAA; 1000];
        let payload2 = vec![0xBB; 461];
        let frag1 = make_fragment(1461, 0, 1000, &payload1);
        let frag2 = make_fragment(1461, 1000, 461, &payload2);

        // Receive fragment 2 first
        assert!(!feed(&mut buffer, &frag2).unwrap());
        assert!(feed(&mut buffer, &frag1).unwrap());

        let mut out = vec![0u8; 4096];
        let n = buffer.read(&mut out);
        assert_eq!(n, TOTAL_HDR_LEN + 1461);

        assert_eq!(u24(&out[19..22]), 0);
        assert_eq!(u24(&out[22..25]), 1461);

        assert!(out[TOTAL_HDR_LEN..TOTAL_HDR_LEN + 1000]
            .iter()
            .all(|&b| b == 0xAA));
        assert!(out[TOTAL_HDR_LEN + 1000..TOTAL_HDR_LEN + 1461]
            .iter()
            .all(|&b| b == 0xBB));
    }

    #[test]
    fn test_fragmented_with_overlap() {
        let mut buffer = ClientHelloBuffer::new();
        // Use the same byte value in the overlap region (as required by RFC 6347)
        let mut payload1 = vec![0xAA; 1000];
        let mut payload2 = vec![0xBB; 561];
        // Overlap region [900..1000) — must match in both fragments
        payload1[900..1000].fill(0xDD);
        payload2[..100].fill(0xDD);
        let frag1 = make_fragment(1461, 0, 1000, &payload1);
        let frag2 = make_fragment(1461, 900, 561, &payload2);

        assert!(!feed(&mut buffer, &frag1).unwrap());
        assert!(feed(&mut buffer, &frag2).unwrap());

        let mut out = vec![0u8; 4096];
        let n = buffer.read(&mut out);
        assert_eq!(n, TOTAL_HDR_LEN + 1461);

        // Bytes 0-899: 0xAA (from frag1 only)
        assert!(out[TOTAL_HDR_LEN..TOTAL_HDR_LEN + 900]
            .iter()
            .all(|&b| b == 0xAA));
        // Bytes 900-999: 0xDD (overlap region, same content in both)
        assert!(out[TOTAL_HDR_LEN + 900..TOTAL_HDR_LEN + 1000]
            .iter()
            .all(|&b| b == 0xDD));
        // Bytes 1000-1460: 0xBB (from frag2 only)
        assert!(out[TOTAL_HDR_LEN + 1000..TOTAL_HDR_LEN + 1461]
            .iter()
            .all(|&b| b == 0xBB));
    }

    #[test]
    fn test_not_handshake_returns_error() {
        let mut buffer = ClientHelloBuffer::new();
        let mut datagram = make_fragment(200, 0, 200, &[0xCC; 200]);
        datagram[0] = 23; // application data, not handshake

        let result = feed(&mut buffer, &datagram);
        assert!(result.is_err());
    }

    #[test]
    fn test_not_client_hello_returns_error() {
        let mut buffer = ClientHelloBuffer::new();
        let mut datagram = make_fragment(200, 0, 200, &[0xCC; 200]);
        datagram[RECORD_HDR_LEN] = 2; // ServerHello, not ClientHello

        let result = feed(&mut buffer, &datagram);
        assert!(result.is_err());
    }
}
