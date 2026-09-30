"""同梱音源を純粋な数式で生成する(外部音源を一切使わないため著作権の問題が無い)。
生成物は本プロジェクトが CC0 1.0 (パブリックドメイン相当) として公開する。
使い方: python tools/gen_sounds.py  → app/src/main/res/raw/*.wav
"""
import math, struct, wave, os

SR = 22050
OUT = os.path.join(os.path.dirname(__file__), "..", "app", "src", "main", "res", "raw")


def write(name, samples):
    os.makedirs(OUT, exist_ok=True)
    with wave.open(os.path.join(OUT, name + ".wav"), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, s)) * 30000)) for s in samples))


def tone(freq, dur, vol=0.5, decay=6.0, harm=(1.0,)):
    n = int(SR * dur)
    return [vol * math.exp(-decay * i / n) * sum(h * math.sin(2 * math.pi * freq * (k + 1) * i / SR)
            for k, h in enumerate(harm)) / sum(harm) for i in range(n)]


def silence(dur):
    return [0.0] * int(SR * dur)


# チャイム: 3音の鐘(倍音つき、ゆっくり減衰)
chime = []
for f in (784, 988, 1319):
    chime += tone(f, 0.9, 0.6, 4.0, (1.0, 0.4, 0.2))
write("chime", chime)

# アラーム: ピピピピ を2セット
alarm = []
for _ in range(2):
    for _ in range(4):
        alarm += tone(1760, 0.12, 0.6, 1.0) + silence(0.08)
    alarm += silence(0.5)
write("alarm", alarm)

# メロディ: やさしい上昇アルペジオ(C長調)
melody = []
for f in (523.25, 659.25, 783.99, 1046.5, 783.99, 659.25, 523.25):
    melody += tone(f, 0.45, 0.5, 3.0, (1.0, 0.3))
melody += silence(0.6)
write("melody", melody)
print("ok")
