use std::time::Duration;
use symphonia::{
    core::{
        audio::{AudioBufferRef, SampleBuffer, SignalSpec},
        codecs::{Decoder, DecoderOptions},
        errors::Error,
        formats::{self, FormatOptions, FormatReader, Packet},
        io::{MediaSource, MediaSourceStream},
        meta::MetadataOptions,
        probe::Hint,
        units::Time,
    },
    default::get_probe,
};

use rodio::{decoder::DecoderError, Source};

use std::sync::mpsc;
use tokio::sync::mpsc::Sender;

// Decoder errors are not considered fatal.
// The correct action is to just get a new packet and try again.
// But a decode error in more than 3 consecutive packets is fatal.
const MAX_DECODE_ERRORS: usize = 3;

pub(crate) struct SymphoniaDecoder {
    decoder: Box<dyn Decoder>,
    current_frame_offset: usize,
    packet: Packet,
    pub(crate) format: Box<dyn FormatReader>,
    buffer: SampleBuffer<i16>,
    spec: SignalSpec,
    tx: Sender<crate::ReceivedData>,
    rx: mpsc::Receiver<crate::PlaybackInstructions>,
}

impl SymphoniaDecoder {
    pub(crate) fn new<MS: MediaSource + 'static>(
        ms: MS,
        tx: Sender<crate::ReceivedData>,
        rx: mpsc::Receiver<crate::PlaybackInstructions>,
    ) -> Result<Self, DecoderError> {
        let mss = MediaSourceStream::new(Box::new(ms) as Box<dyn MediaSource>, Default::default());
        match SymphoniaDecoder::init(mss, tx, rx) {
            Err(e) => match e {
                Error::IoError(e) => Err(DecoderError::IoError(e.to_string())),
                Error::DecodeError(e) => Err(DecoderError::DecodeError(e)),
                Error::SeekError(_) => {
                    unreachable!("Seek errors should not occur during initialization")
                }
                Error::Unsupported(_) => Err(DecoderError::UnrecognizedFormat),
                Error::LimitError(e) => Err(DecoderError::LimitError(e)),
                Error::ResetRequired => Err(DecoderError::ResetRequired),
            },
            Ok(Some(decoder)) => Ok(decoder),
            Ok(None) => Err(DecoderError::NoStreams),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn into_inner(self: Box<Self>) -> MediaSourceStream {
        self.format.into_inner()
    }

    fn init(
        mss: MediaSourceStream,
        tx: Sender<crate::ReceivedData>,
        rx: mpsc::Receiver<crate::PlaybackInstructions>,
    ) -> symphonia::core::errors::Result<Option<SymphoniaDecoder>> {
        let hint = Hint::new();
        let format_opts: FormatOptions = Default::default();
        let metadata_opts: MetadataOptions = Default::default();
        let mut probed = get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;
        if let Some(mut maybe_metadata) = probed.metadata.get() {
            let metadata = maybe_metadata.skip_to_latest().cloned();
            let _ = tx.blocking_send(crate::ReceivedData::NewMetadata(metadata.unwrap()));
        }

        let stream = match probed.format.default_track() {
            Some(stream) => stream,
            None => return Ok(None),
        };

        let mut decoder = symphonia::default::get_codecs().make(
            &stream.codec_params,
            &DecoderOptions {
                verify: true,
                ..Default::default()
            },
        )?;

        let mut decode_errors: usize = 0;
        let (packet, decoded) = loop {
            let current_frame = probed.format.next_packet()?;
            match decoder.decode(&current_frame) {
                Ok(decoded) => break (current_frame, decoded),
                Err(e) => match e {
                    Error::DecodeError(_) => {
                        decode_errors += 1;
                        if decode_errors > MAX_DECODE_ERRORS {
                            return Err(e);
                        } else {
                            continue;
                        }
                    }
                    _ => return Err(e),
                },
            }
        };
        let spec = *decoded.spec();
        let buffer = SymphoniaDecoder::get_buffer(decoded, &spec);

        Ok(Some(SymphoniaDecoder {
            decoder,
            current_frame_offset: 0,
            format: probed.format,
            packet,
            buffer,
            spec,
            tx,
            rx,
        }))
    }

    #[inline]
    fn get_buffer(decoded: AudioBufferRef<'_>, spec: &SignalSpec) -> SampleBuffer<i16> {
        let duration = decoded.capacity() as u64;
        let mut buffer = SampleBuffer::<i16>::new(duration, *spec);
        buffer.copy_interleaved_ref(decoded);
        buffer
    }
}

impl Source for SymphoniaDecoder {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.buffer.samples().len())
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.spec.channels.count() as u16
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.spec.rate
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Iterator for SymphoniaDecoder {
    type Item = i16;

    #[inline]
    fn next(&mut self) -> Option<i16> {
        if self.current_frame_offset == self.buffer.len() {
            match self.rx.try_recv() {
                Ok(instruction) => {
                    if let crate::PlaybackInstructions::Seek(to) = instruction {
                        let seconds = to as f64 / 1000.0;
                        let _ = self.format.seek(
                            formats::SeekMode::Accurate,
                            formats::SeekTo::Time {
                                time: Time::from(seconds),
                                track_id: None,
                            },
                        );
                    }
                }
                Err(_) => {}
            }

            let mut decode_errors: usize = 0;
            let (packet, decoded) = loop {
                match self.format.next_packet() {
                    Ok(packet) => match self.decoder.decode(&packet) {
                        Ok(decoded) => break (packet, decoded),
                        Err(e) => match e {
                            Error::DecodeError(_) => {
                                decode_errors += 1;
                                if decode_errors > MAX_DECODE_ERRORS {
                                    return None;
                                } else {
                                    continue;
                                }
                            }
                            _ => return None,
                        },
                    },
                    Err(_) => return None,
                }
            };
            self.spec = *decoded.spec();
            self.buffer = SymphoniaDecoder::get_buffer(decoded, &self.spec);
            self.current_frame_offset = 0;
            let _ = self
                .tx
                .try_send(crate::ReceivedData::NewTimestamp(packet.ts));
            self.packet = packet;
        }

        let sample = self.buffer.samples()[self.current_frame_offset];
        self.current_frame_offset += 1;

        Some(sample)
    }
}
