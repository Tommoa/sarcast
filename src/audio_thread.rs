// use std::time::Duration;

use rodio::{OutputStream, Sink};
use std::sync;
use tokio::sync::mpsc;

use crate::decoder::SymphoniaDecoder;
use crate::{PlaybackInstructions, ReceivedData};

pub(crate) fn run(stx: mpsc::Sender<ReceivedData>, mut recv: mpsc::Receiver<PlaybackInstructions>) {
    // Get a output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let mut sink = Sink::try_new(&stream_handle).unwrap();
    let (mut tx, _) = sync::mpsc::channel();

    while let Some(instruction) = recv.blocking_recv() {
        match instruction {
            PlaybackInstructions::Play => sink.play(),
            PlaybackInstructions::Pause => sink.pause(),
            PlaybackInstructions::Speed(speed) => sink.set_speed(speed),
            instruction @ PlaybackInstructions::Seek(_) => {
                let _ = tx.send(instruction);
            }
            PlaybackInstructions::NewStream(file) => {
                sink.stop();
                sink = Sink::try_new(&stream_handle).unwrap();
                let (ntx, srx) = sync::mpsc::channel();
                tx = ntx;
                let source = SymphoniaDecoder::new(file, stx.clone(), srx).unwrap();
                // source.convert_samples();
                // Play the sound directly on the device
                sink.append(source);
            }
        }
    }
}
