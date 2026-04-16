#![no_std]
#![no_main]

use defmt::{info, warn, error};
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::saadc::CallbackResult;
use embassy_nrf::timer::Frequency;
use embassy_nrf::{bind_interrupts, saadc};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_time::Instant;
use static_cell::StaticCell;
use wakew::dtw::dtw;
use wakew::mfcc::{FEATURE_SIZE, FRAME_SIZE, Mfcc, NUM_MFCC, SHIFT_WIDTH, window_to_features_into};
use {defmt_rtt as _, panic_probe as _};
use embedded_wakew::utils::{RingBuffer, WakeWordError, from_i16_pcm_to_f32, prepare_mic_saadc};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});

include!("../../reference.rs");

const DETECT_THRESHOLD: f32 = 13f32;
// Window must exceed the reference frame count (146) to allow timing variation.
// 16500 samples @ 16kHz gives 176 frames.
const WINDOW_SIZE: usize = (18000 - FRAME_SIZE + SHIFT_WIDTH - 1) / SHIFT_WIDTH;
const MFCC_SHIFT: usize = 15;
const CHANNEL_SIZE: usize = 300;

static CHANNEL: StaticCell<Channel<NoopRawMutex, [f32; NUM_MFCC], CHANNEL_SIZE>> = StaticCell::new();

#[embassy_executor::task]
async fn blink_led(receiver: Receiver<'static, NoopRawMutex, [f32; NUM_MFCC], CHANNEL_SIZE>, mut pin: Output<'static>) {
    let mut buf = RingBuffer::<WINDOW_SIZE, MFCC_SHIFT, [f32; NUM_MFCC]>::new([0f32; NUM_MFCC]);
    let mut features = [[0f32; FEATURE_SIZE]; WINDOW_SIZE];
    loop {
        let data = receiver.receive().await;
        match buf.update(&[data]) {
            Err(WakeWordError::WouldOverflow) => error!("Error writing to mfcc ringbuffer, would overflow"),
            _ => ()
        };
        if let Some(window) = buf.frame() {
            window_to_features_into(&window, &mut features).await;
            let start = Instant::now();
            let mut min_distance = f32::INFINITY;
            for reference in REFERENCES {
                let d = dtw(reference, &features, DETECT_THRESHOLD).await;
                if d < min_distance { min_distance = d; }
            }
            let elapsed = start.elapsed().as_millis();
            info!("All DTW took {} ms, min distance: {}", elapsed, min_distance);
            if min_distance < DETECT_THRESHOLD {
                pin.toggle();
            }
        }
    }
}

#[embassy_executor::main]
async fn main(s: Spawner) {
    let channel = CHANNEL.init(Channel::new());
    let receiver = channel.receiver();
    let sender = channel.sender();
    let p = embassy_nrf::init(Default::default());
    let (mut saadc, _mic_pwr) = prepare_mic_saadc(p.SAADC, p.P0_20, p.P0_05, Irqs);
    // Setup LED pins
    let _col1 = Output::new(p.P0_28, Level::Low, OutputDrive::Standard);
    let row1 = Output::new(p.P0_21, Level::Low, OutputDrive::Standard);
    // DMA buffers
    let mut bufs = [[[0; 1]; 100]; 2];
    // Buffer to copy DMA samples to and perform MFCC conversion from
    let mut audio_sample_buffer = RingBuffer::<FRAME_SIZE, SHIFT_WIDTH, f32>::new(0f32);
    let mfcc = Mfcc::new();
    saadc.calibrate().await;

    s.spawn(blink_led(receiver, row1).unwrap());

    saadc
        .run_task_sampler(
            p.TIMER0,
            p.PPI_CH0,
            p.PPI_CH1,
            Frequency::F16MHz,
            1000,
            &mut bufs,
            |b| {
                let mut u = [0f32; SHIFT_WIDTH];
                for i in 0..b.len() {
                    u[i] = from_i16_pcm_to_f32(b[i][0]);
                }
                match audio_sample_buffer.update(&u[0..b.len()]) {
                    Err(WakeWordError::WouldOverflow) => error!("Error writing to audio sample buffer, would overflow"),
                    Ok(()) => ()
                };
                while let Some(frame) = audio_sample_buffer.frame() {
                    let mfcc_res = mfcc.mfcc(&frame);
                    let r = sender.try_send(mfcc_res);
                    if let Err(_) = r {
                        warn!("Mfccs were discarded while writing to channel")
                    }
                }
                CallbackResult::Continue
            },
        )
        .await;
}
