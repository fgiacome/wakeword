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
use wakew::mfcc::{FRAME_SIZE, Mfcc, NUM_MFCC, SHIFT_WIDTH};
use {defmt_rtt as _, panic_probe as _};
use embedded_wakew::utils::{RingBuffer, WakeWordError, from_i16_pcm_to_f32, prepare_mic_saadc};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});

include!("../../reference.rs");

const WINDOW_SIZE: usize = (15000 - FRAME_SIZE + SHIFT_WIDTH -1) / SHIFT_WIDTH;

static CHANNEL: StaticCell<Channel<NoopRawMutex, [f32; NUM_MFCC], 28>> = StaticCell::new();

#[embassy_executor::task]
async fn blink_led(receiver: Receiver<'static, NoopRawMutex, [f32; NUM_MFCC], 28>, mut pin: Output<'static>) {
    let mut buf = RingBuffer::<WINDOW_SIZE, 28, [f32; NUM_MFCC]>::new([0f32; NUM_MFCC]);
    loop {
        let data = receiver.receive().await;
        match buf.update(&[data]) {
            Err(WakeWordError::WouldOverflow) => error!("Error writing to mfcc ringbuffer, would overflow"),
            _ => ()
        };
        if let Some(window) = buf.frame() {
            let start = Instant::now();
            let distance = dtw(&REFERENCE, &window).await;
            let elapsed = start.elapsed().as_millis();
            info!("Duration took {} ms", elapsed);
            info!("Distance: {}", distance);
            if distance < 0.8 {
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
