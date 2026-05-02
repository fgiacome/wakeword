# Embedded Wakeword Recognition

An embedded wakeword recognition application written in Rust + Embassy, targeting
the micro:bit v2 board. The `wakew` crate is a platform-independent `no_std`
wakeword recognition library based on MFCCs and DTW template matching. The
`wakew_microbit` crate contains micro:bit-specific I/O and the two binary
targets: `record_audio` and `wake_word`.

Before compiling and flashing `wake_word`, you need to record reference clips and
store their MFCCs in `wakew_microbit/reference.rs`. The setup workflow is:

1. Flash `record_audio` onto your micro:bit. It streams raw audio as hex-encoded
   bytes over RTT to the host terminal.
2. Save that output to `to_wav/recording.raw`, then run the Python script in
   `to_wav/` to produce a WAV file.
3. Repeat for 3–4 utterances of your wakeword.
4. Pass the WAV files to the `dump_wav` example in the `wakew` crate to generate
   the `reference.rs` content, then place it at `wakew_microbit/reference.rs`.

You should also tune the constants in `dump_wav.rs` and `wake_word.rs` to suit
your recordings. At a minimum: `SIZE` (reference length in samples), `WINDOW_SIZE`
(live detection window length in MFCC frames), `MFCC_SHIFT` (window slide step),
and `DETECT_THRESHOLD` (DTW distance threshold for triggering a detection).