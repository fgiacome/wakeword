use embassy_nrf::{
    Peri, gpio, interrupt, peripherals::{P0_05, P0_20, SAADC}, saadc
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WakeWordError {
    #[error("Update would overflow ringbuffer")]
    WouldOverflow,
}

/// A RingBuffer that returns a copy of the entire buffer starting from the most
/// recently read position.
///
/// ## Generic params
/// B is the size of the buffer, S is the amount of new samples that must be
/// written to the buffer before getting a new read, and T is the underlying
/// data type.
///
/// `U` must be >= `S` and not more than `U` samples at a time should be written
/// to the buffer.
pub struct RingBuffer<const B: usize, const S: usize, T> {
    b: [T; B],
    start: usize,
    last_read: usize,
    free_space: usize,
}

impl<const B: usize, const S: usize, T: Copy> RingBuffer<B, S, T> {
    pub fn new(init: T) -> Self {
        let b = [init; B];
        let start = 0;
        let last_read = 0;
        let free_space = B;
        RingBuffer {
            b,
            start,
            last_read,
            free_space,
        }
    }

    /// This function writes the samples contained in `u` to the buffer.
    pub fn update(&mut self, u: &[T]) -> Result<(), WakeWordError> {
        if u.len() > self.free_space {
            return Err(WakeWordError::WouldOverflow);
        }
        for i in 0..u.len() {
            self.b[(self.start + i) % B] = u[i];
        }
        self.start = (self.start + u.len()) % B;
        self.free_space -= u.len();
        Ok(())
    }

    /// This function returns the data contained in the buffer from least to
    /// most recent. If fewer than `S` unread samples are present in the buffer,
    /// returns None.
    pub fn frame(&mut self) -> Option<[T; B]> {
        let virtual_start = if self.start >= self.last_read {
            self.start
        } else {
            self.start + B
        };
        let to_read = virtual_start - self.last_read;
        if to_read > S {
            self.free_space += to_read;
            let buf = core::array::from_fn(|i| self.b[(self.last_read + i) % B]);
            self.last_read = (self.last_read + S) % B;
            Some(buf)
        } else {
            None
        }
    }
}

pub fn prepare_mic_saadc<'b>(
    saadc_peri: Peri<'b, SAADC>,
    p0_20_peri: Peri<'b, P0_20>,
    p0_05_peri: Peri<'b, P0_05>,
    irq: impl interrupt::typelevel::Binding<interrupt::typelevel::SAADC, saadc::InterruptHandler> + 'b,
) -> (saadc::Saadc<'b, 1>, gpio::Output<'b>) {
    let mic_pwr = gpio::Output::new(
        p0_20_peri,
        gpio::Level::High,
        gpio::OutputDrive::Standard,
    );
    let mut config = saadc::Config::default();
    config.resolution = saadc::Resolution::_12BIT;
    let mut channel_config = saadc::ChannelConfig::single_ended(p0_05_peri);
    channel_config.gain = saadc::Gain::GAIN1_4;
    let saadc = saadc::Saadc::new(saadc_peri, irq, config, [channel_config]);
    (saadc, mic_pwr)
}

pub fn from_i16_pcm_to_f32(i: i16) -> f32 {
    i as f32 / 32768f32
}