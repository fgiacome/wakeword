import numpy as np
import wave

raw_text = open("recording.raw").read().strip()

# slice into 2-char hex bytes
raw_bytes = bytes(int(raw_text[i:i+2], base=16) for i in range(0, len(raw_text), 2))

samples = np.frombuffer(raw_bytes, dtype="<i2")

print(f"Got {len(samples)} samples ({len(samples)/16000:.2f}s at 16kHz)")

with wave.open("recording.wav", "w") as f:
    f.setnchannels(1)
    f.setsampwidth(2)
    f.setframerate(16000)
    f.writeframes(samples.tobytes())