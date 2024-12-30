use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia_bundle_mp3::MpaReader;
use symphonia_core::audio::{AudioBufferRef, Signal};
use symphonia_core::checksum::Md5;
use symphonia_core::formats::{FormatReader, StreamingFormatReader};
use symphonia_core::io::{MediaSource, Monitor};

use std::{fs, io};
use std::io::{Read, Seek};

struct SlowFile {
    inner: fs::File,
    counter: usize,
}

impl Read for SlowFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.counter += 1;
        if self.counter % 4 == 0 {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
        } else {
            self.inner.read(buf)
        }
    }
}

impl Seek for SlowFile {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl MediaSource for SlowFile {
    fn is_seekable(&self) -> bool {
        self.inner.is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        self.inner.byte_len()
    }
}

impl SlowFile {
    fn open(path: &str) -> io::Result<SlowFile> {
        let inner = fs::File::open(path)?;
        Ok(SlowFile {
            inner: inner,
            counter: 0,
        })
    }
}



fn main() {
    // Get the first command line argument.
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("file path not provided");

    // Open the media source.
    let src = SlowFile::open(path).expect("failed to open media");

    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Create a probe hint using the file's extension. [Optional]
    // let mut hint = Hint::new();
    // hint.with_extension("mp3");

    // Use the default options for metadata and format readers.
    let fmt_opts: FormatOptions = Default::default();

    // Probe the media source.
    // let probed = symphonia::default::get_probe()
        // .format(&hint, mss, &fmt_opts, &meta_opts)
        // .expect("unsupported format");

    // Get the instantiated format reader.
    // let mut format = probed.format;

    let mut mp3_reader = MpaReader::try_new(mss, &fmt_opts).expect("no mp3 reader");

    // Find the first audio track with a known (decodeable) codec.
    let track = mp3_reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .expect("no supported audio tracks");

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();
    let mut samples_hash = Md5::default();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .expect("unsupported codec");

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    // The decode loop.
    'decode: loop {
        // Get the next packet from the media format.
        let packet = match mp3_reader.try_next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(Error::MoreDataRequired) => {
                println!("More data required. Try again.");
                continue 'decode;
            }
            Err(err) => {
                // A unrecoverable error occurred, halt decoding.
                println!("samples hash = {:x?}", samples_hash.md5());
                panic!("{}", err);
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !mp3_reader.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            mp3_reader.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(decoded) => {
                // Consume the decoded audio samples (see below).
                match decoded {
                    AudioBufferRef::F32(buf) => {
                        for channel in 0..buf.spec().channels.count() {
                            for sample in buf.chan(channel) {
                                samples_hash.process_quad_bytes(sample.to_le_bytes());
                            }
                        }
                    },
                    _ => {
                        unimplemented!();
                    }
                }
            }
            Err(Error::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                continue;
            }
            Err(Error::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                continue;
            }
            Err(err) => {
                // An unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        }
    }
}
