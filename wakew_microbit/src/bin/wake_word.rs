#![no_std]
#![no_main]

use defmt::{error, info, warn};
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::pwm::{DutyCycle, SimpleConfig, SimplePwm};
use embassy_nrf::saadc::CallbackResult;
use embassy_nrf::timer::Frequency;
use embassy_nrf::{bind_interrupts, saadc};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use static_cell::StaticCell;
use wakew::dtw::dtw;
use wakew::mfcc::{FEATURE_SIZE, FRAME_SIZE, Mfcc, NUM_MFCC, SHIFT_WIDTH, window_to_features_into};
use wakew_microbit::utils::{RingBuffer, WakeWordError, from_i16_pcm_to_f32, prepare_mic_saadc};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});

include!("../../reference.rs");

const DETECT_THRESHOLD: f32 = 14f32;
// 18000 samples gives 88 frames.
const WINDOW_SIZE: usize = (18000 - FRAME_SIZE + SHIFT_WIDTH - 1) / SHIFT_WIDTH;
const MFCC_SHIFT: usize = 15;
const CHANNEL_SIZE: usize = 300;

// Smiley face for the 5x5 LED matrix.
// Columns are [COL1/P0_28, COL2/P0_11, COL3/P0_31, COL4/P1_05, COL5/P0_30].
// Rows are active-high; columns are active-low. true = LED on.
const SMILEY: [[bool; 5]; 5] = [
    [false, false, false, false, false],
    [false, true, false, true, false], // eyes
    [false, false, false, false, false],
    [true, false, false, false, true], // mouth corners
    [false, true, true, true, false],  // mouth
];

// Jingle played on detection: (frequency Hz, duration ms).
//
// SimpleConfig default uses Prescaler::Div16 -> 1 MHz counter;
// top = 1_000_000 // freq.
const JINGLE: [(u32, u64); 4] = [
    (523, 150),  // C5
    (659, 150),  // E5
    (784, 150),  // G5
    (1047, 500), // C6
];

static CHANNEL: StaticCell<Channel<NoopRawMutex, [f32; NUM_MFCC], CHANNEL_SIZE>> =
    StaticCell::new();
static DETECTED: StaticCell<Signal<NoopRawMutex, ()>> = StaticCell::new();

#[embassy_executor::task]
async fn infer(
    receiver: Receiver<'static, NoopRawMutex, [f32; NUM_MFCC], CHANNEL_SIZE>,
    detected: &'static Signal<NoopRawMutex, ()>,
) {
    let mut buf =
        RingBuffer::<WINDOW_SIZE, WINDOW_SIZE, MFCC_SHIFT, [f32; NUM_MFCC]>::new([0f32; NUM_MFCC]);
    let mut features = [[0f32; FEATURE_SIZE]; WINDOW_SIZE];
    loop {
        let data = receiver.receive().await;
        match buf.update(&[data]) {
            Err(WakeWordError::WouldOverflow) => {
                error!("Error writing to mfcc ringbuffer, would overflow")
            }
            _ => (),
        };
        if let Some(window) = buf.frame() {
            window_to_features_into(&window, &mut features).await;
            let start = Instant::now();
            let mut min_distance = f32::INFINITY;
            for reference in REFERENCES {
                let d = dtw(reference, &features, DETECT_THRESHOLD).await;
                if d < min_distance {
                    min_distance = d;
                }
            }
            let elapsed = start.elapsed().as_millis();
            info!(
                "All DTW took {} ms, min distance: {}",
                elapsed, min_distance
            );
            if min_distance < DETECT_THRESHOLD {
                detected.signal(());
            }
        }
    }
}

async fn scan_smiley(
    rows: &mut [Output<'static>; 5],
    cols: &mut [Output<'static>; 5],
    duration: Duration,
) {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        for r in 0..5 {
            for c in 0..5 {
                if SMILEY[r][c] {
                    cols[c].set_low();
                } else {
                    cols[c].set_high();
                }
            }
            rows[r].set_high();
            Timer::after_micros(2000).await;
            rows[r].set_low();
        }
    }
    for col in cols.iter_mut() {
        col.set_high();
    }
}

#[embassy_executor::task]
async fn celebrate(
    detected: &'static Signal<NoopRawMutex, ()>,
    mut rows: [Output<'static>; 5],
    mut cols: [Output<'static>; 5],
    mut pwm: SimplePwm<'static>,
) {
    loop {
        detected.wait().await;

        pwm.enable();
        for (freq, duration_ms) in JINGLE {
            let top = (1_000_000u32 / freq) as u16;
            pwm.set_max_duty(top);
            pwm.set_duty(0, DutyCycle::normal(top / 2));
            scan_smiley(&mut rows, &mut cols, Duration::from_millis(duration_ms)).await;
        }
        pwm.disable();

        scan_smiley(&mut rows, &mut cols, Duration::from_millis(2000)).await;

        detected.reset();
    }
}

#[embassy_executor::main]
async fn main(s: Spawner) {
    let channel = CHANNEL.init(Channel::new());
    let receiver = channel.receiver();
    let sender = channel.sender();
    let detected = DETECTED.init(Signal::new());
    let p = embassy_nrf::init(Default::default());
    let (mut saadc, _mic_pwr) = prepare_mic_saadc(p.SAADC, p.P0_20, p.P0_05, Irqs);

    // LED matrix row pins (active high)
    let rows = [
        Output::new(p.P0_21, Level::Low, OutputDrive::Standard), // ROW1
        Output::new(p.P0_22, Level::Low, OutputDrive::Standard), // ROW2
        Output::new(p.P0_15, Level::Low, OutputDrive::Standard), // ROW3
        Output::new(p.P0_24, Level::Low, OutputDrive::Standard), // ROW4
        Output::new(p.P0_19, Level::Low, OutputDrive::Standard), // ROW5
    ];
    // LED matrix column pins (active low)
    let cols = [
        Output::new(p.P0_28, Level::High, OutputDrive::Standard), // COL1
        Output::new(p.P0_11, Level::High, OutputDrive::Standard), // COL2
        Output::new(p.P0_31, Level::High, OutputDrive::Standard), // COL3
        Output::new(p.P1_05, Level::High, OutputDrive::Standard), // COL4
        Output::new(p.P0_30, Level::High, OutputDrive::Standard), // COL5
    ];

    // Speaker on P0_00; SimpleConfig default uses Prescaler::Div16 (1 MHz counter)
    let pwm = SimplePwm::new_1ch(p.PWM0, p.P0_00, &SimpleConfig::default());

    // DMA buffers
    let mut bufs = [[[0; 1]; 100]; 2];
    // Buffer to copy DMA samples to and perform MFCC conversion from
    let mut audio_sample_buffer = RingBuffer::<FRAME_SIZE, FRAME_SIZE, SHIFT_WIDTH, f32>::new(0f32);
    let mfcc = Mfcc::new();
    saadc.calibrate().await;

    s.spawn(infer(receiver, detected).unwrap());
    s.spawn(celebrate(detected, rows, cols, pwm).unwrap());

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
                    Err(WakeWordError::WouldOverflow) => {
                        error!("Error writing to audio sample buffer, would overflow")
                    }
                    Ok(()) => (),
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
