//! The certificate compressors handed to wreq (RFC 8879).
//!
//! wreq registers whatever codecs the caller gives it. These are the three that wreq
//! `v6.0.0-rc.28` shipped built in, with the same settings. A client only ever runs the
//! decompress side: the server compresses its certificate chain, the client inflates it.

use std::io::{self, Write};

use brotli::{CompressorWriter as BrotliEncoder, Decompressor as BrotliDecoder};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use wreq::tls::compress::{CertificateCompressionAlgorithm, CertificateCompressor, Codec};
use zstd::stream::{Decoder as ZstdDecoder, Encoder as ZstdEncoder};

#[derive(Debug)]
struct Zlib;

#[derive(Debug)]
struct Brotli;

#[derive(Debug)]
struct Zstd;

static ZLIB: Zlib = Zlib;
static BROTLI: Brotli = Brotli;
static ZSTD: Zstd = Zstd;

/// The compressor wreq registers for `algorithm`.
pub(crate) fn compressor(
    algorithm: CertificateCompressionAlgorithm,
) -> &'static dyn CertificateCompressor {
    match algorithm {
        CertificateCompressionAlgorithm::BROTLI => &BROTLI,
        CertificateCompressionAlgorithm::ZSTD => &ZSTD,
        _ => &ZLIB,
    }
}

impl CertificateCompressor for Zlib {
    fn compress(&self) -> Codec {
        Codec::Pointer(|input, output| {
            let mut encoder = ZlibEncoder::new(output, Compression::default());
            encoder.write_all(input)?;
            encoder.finish()?;
            Ok(())
        })
    }

    fn decompress(&self) -> Codec {
        Codec::Pointer(|input, output| {
            let mut decoder = ZlibDecoder::new(input);
            io::copy(&mut decoder, output)?;
            Ok(())
        })
    }

    fn algorithm(&self) -> CertificateCompressionAlgorithm {
        CertificateCompressionAlgorithm::ZLIB
    }
}

impl CertificateCompressor for Brotli {
    fn compress(&self) -> Codec {
        Codec::Pointer(|input, output| {
            // Quality 11 and a 22-bit window, as rc.28 wreq used.
            let mut encoder = BrotliEncoder::new(output, input.len(), 11, 22);
            encoder.write_all(input)?;
            encoder.flush()?;
            Ok(())
        })
    }

    fn decompress(&self) -> Codec {
        Codec::Pointer(|input, output| {
            let mut decoder = BrotliDecoder::new(input, 4096);
            io::copy(&mut decoder, output)?;
            Ok(())
        })
    }

    fn algorithm(&self) -> CertificateCompressionAlgorithm {
        CertificateCompressionAlgorithm::BROTLI
    }
}

impl CertificateCompressor for Zstd {
    fn compress(&self) -> Codec {
        Codec::Pointer(|input, output| {
            let mut encoder = ZstdEncoder::new(output, 0)?;
            encoder.write_all(input)?;
            encoder.finish()?;
            Ok(())
        })
    }

    fn decompress(&self) -> Codec {
        Codec::Pointer(|input, output| {
            let mut decoder = ZstdDecoder::new(input)?;
            io::copy(&mut decoder, output)?;
            Ok(())
        })
    }

    fn algorithm(&self) -> CertificateCompressionAlgorithm {
        CertificateCompressionAlgorithm::ZSTD
    }
}

#[cfg(test)]
mod tests {
    use wreq::tls::compress::{CertificateCompressionAlgorithm, Codec};

    use super::compressor;

    fn run(codec: &Codec, input: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        match codec {
            Codec::Pointer(func) => func(input, &mut output).unwrap(),
            Codec::Dynamic(func) => func(input, &mut output).unwrap(),
        }
        output
    }

    #[test]
    fn each_codec_round_trips_and_reports_its_algorithm() {
        // Long enough that every codec shrinks it, so the compressed bytes are not the input.
        let chain = b"-----BEGIN CERTIFICATE-----\n".repeat(64);
        for algorithm in [
            CertificateCompressionAlgorithm::ZLIB,
            CertificateCompressionAlgorithm::BROTLI,
            CertificateCompressionAlgorithm::ZSTD,
        ] {
            let compressor = compressor(algorithm);
            assert_eq!(compressor.algorithm(), algorithm);
            let compressed = run(&compressor.compress(), &chain);
            assert!(compressed.len() < chain.len(), "{algorithm:?}");
            assert_eq!(run(&compressor.decompress(), &compressed), chain);
        }
    }
}
